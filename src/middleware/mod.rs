use std::net::UdpSocket;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Error;

use crate::types::Metric;

pub mod aggregate;
pub mod allow_tag;
pub mod cardinality_limit;
pub mod deny_tag;

const BUFSIZE: usize = 8192;

#[derive(Debug)]
pub struct Overloaded {
    pub metric: Option<Metric>,
}

impl Middleware for Box<dyn Middleware> {
    fn join(&mut self) -> Result<(), Error> {
        self.as_mut().join()
    }
    fn poll(&mut self) -> Result<(), Overloaded> {
        self.as_mut().poll()
    }
    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded> {
        self.as_mut().submit(metric)
    }
}

pub trait Middleware {
    fn join(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn poll(&mut self) -> Result<(), Overloaded> {
        Ok(())
    }
    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded>;
}

pub struct Upstream {
    socket: Arc<UdpSocket>,
    buffer: [u8; BUFSIZE],
    buf_used: usize,
    last_sent_at: SystemTime,
}

impl Upstream {
    pub fn new(upstream: String) -> Result<Self, Error> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        // cloudflare says connect() allows some kernel-internal optimizations on Linux
        // https://blog.cloudflare.com/everything-you-ever-wanted-to-know-about-udp-sockets-but-were-afraid-to-ask-part-1/
        socket.connect(upstream)?;
        Ok(Upstream {
            socket: Arc::new(socket),
            buffer: [0; BUFSIZE],
            buf_used: 0,
            last_sent_at: UNIX_EPOCH,
        })
    }
}

impl Middleware for Upstream {
    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded> {
        let now = SystemTime::now();
        let metric_len = metric.raw.len();
        if metric_len + 1 > BUFSIZE - self.buf_used
            || now
                .duration_since(self.last_sent_at)
                .map_or(true, |x| x > Duration::from_secs(1))
        {
            if self.buf_used > 0 {
                // Message bigger than space left in buffer, or we have not sent any metrics in a
                // while. Flush the buffer.
                let bytes = self
                    .socket
                    .send(&self.buffer[..self.buf_used])
                    .expect("failed to send to upstream");
                if bytes != self.buf_used {
                    // UDP, so this should never happen, but...
                    panic!(
                        "tried to send {} bytes but only sent {}.",
                        self.buf_used, bytes
                    );
                }
                self.buf_used = 0;
            }
            self.last_sent_at = now;
        }
        if metric_len > BUFSIZE {
            // Message too big for the entire buffer, send it and pray.
            let bytes = self
                .socket
                .send(&metric.raw)
                .expect("failed to send to upstream");
            if bytes != metric_len {
                // UDP, so this should never happen, but...
                panic!(
                    "tried to send {} bytes but only sent {}.",
                    metric_len, bytes
                );
            }
        } else {
            // Put the message in the buffer, separating it from the previous message if any.
            if self.buf_used > 0 {
                self.buffer[self.buf_used] = b'\n';
                self.buf_used += 1;
            }
            self.buffer[self.buf_used..self.buf_used + metric_len].copy_from_slice(&metric.raw);
            self.buf_used += metric_len;
        }
        Ok(())
    }
}

pub struct Server<M> {
    socket: UdpSocket,
    middleware: M,
}

impl<M> Server<M>
where
    M: Middleware,
{
    pub fn new(listen: String, middleware: M) -> Result<Self, Error> {
        let socket = UdpSocket::bind(listen)?;
        Ok(Server { socket, middleware })
    }

    pub fn run(mut self) -> Result<(), Error> {
        // if sending this large udp dataframes happens to work randomly, we should not be the
        // one that breaks that setup.
        let mut buf = [0; 65535];

        loop {
            let (num_bytes, _app_socket) = self.socket.recv_from(buf.as_mut_slice())?;
            for raw in buf[..num_bytes].split(|&x| x == b'\n') {
                if raw.is_empty() {
                    continue;
                }

                let raw = raw.to_owned();
                let metric = Metric::new(raw);

                let mut carryover_metric = Some(metric);
                while let Some(metric) = carryover_metric.take() {
                    if let Err(Overloaded { metric }) = self
                        .middleware
                        .poll()
                        .and_then(|()| self.middleware.submit(metric))
                    {
                        carryover_metric = metric;
                    }
                }
            }
        }
    }
}
