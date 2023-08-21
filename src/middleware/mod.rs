use std::net::UdpSocket;
use std::sync::Arc;

use anyhow::Error;

use crate::types::Metric;

pub mod allow_tag;
pub mod deny_tag;
pub mod cardinality_limit;

pub struct Overloaded {
    pub metric: Metric,
}

impl Middleware for Box<dyn Middleware> {
    fn join(&mut self) -> Result<(), Error> {
        self.as_mut().join()
    }
    fn poll(&mut self) -> Result<(), Error> {
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
    fn poll(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded>;
}

pub struct Upstream {
    socket: Arc<UdpSocket>,
    buffer: [u8; 4096],
    buf_used: usize,
}

impl Upstream {
    pub fn new(upstream: String) -> Result<Self, Error> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        // cloudflare says connect() allows some kernel-internal optimizations on Linux
        // https://blog.cloudflare.com/everything-you-ever-wanted-to-know-about-udp-sockets-but-were-afraid-to-ask-part-1/
        socket.connect(upstream)?;
        Ok(Upstream {
            socket: Arc::new(socket),
            buffer: [0; 4096],
            buf_used: 0,
        })
    }
}

impl Middleware for Upstream {
    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded> {
        let metric_len = metric.raw.len();
        if metric_len + 1 > 4096 - self.buf_used {
            // Message bigger than space left in buffer. Flush the buffer.
            self.socket
                .send(&self.buffer[..self.buf_used])
                .expect("failed to send to upstream");
            self.buf_used = 0;
        }
        if metric_len > 4096 {
            // Message too big for the entire buffer, send it and pray.
            self.socket
                .send(&metric.raw)
                .expect("failed to send to upstream");
        }
        else {
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
                let metric = Metric { raw };

                let mut carryover_metric = Some(metric);
                while let Some(metric) = carryover_metric.take() {
                    self.middleware.poll()?;
                    match self.middleware.submit(metric) {
                        Ok(()) => {}
                        Err(Overloaded { metric }) => carryover_metric = Some(metric),
                    }
                }
            }
        }
    }
}
