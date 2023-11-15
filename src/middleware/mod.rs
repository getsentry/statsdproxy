use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Error;

use crate::types::Metric;

pub mod add_tag;
pub mod aggregate;
pub mod allow_tag;
pub mod cardinality_limit;
pub mod deny_tag;
pub mod tag_cardinality_limit;

#[cfg(feature = "cli")]
pub mod server;

const BUFSIZE: usize = 8192;

impl Middleware for Box<dyn Middleware> {
    fn join(&mut self) -> Result<(), Error> {
        self.as_mut().join()
    }
    fn poll(&mut self) {
        self.as_mut().poll()
    }
    fn submit(&mut self, metric: &mut Metric) {
        self.as_mut().submit(metric)
    }
}

pub trait Middleware {
    fn join(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn poll(&mut self) {}
    fn submit(&mut self, metric: &mut Metric);
}

pub struct Upstream {
    socket: Arc<UdpSocket>,
    upstream: SocketAddr,
    buffer: [u8; BUFSIZE],
    buf_used: usize,
    last_sent_at: SystemTime,
}

impl Upstream {
    pub fn new<A>(upstream: A) -> Result<Self, Error>
    where
        A: ToSocketAddrs,
    {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        Ok(Upstream {
            socket: Arc::new(socket),
            upstream: upstream.to_socket_addrs()?.next().unwrap(),
            buffer: [0; BUFSIZE],
            buf_used: 0,
            last_sent_at: UNIX_EPOCH,
        })
    }

    fn send_buffer(&self, buf: &[u8]) {
        match self.socket.send_to(buf, self.upstream) {
            Ok(bytes) => {
                if bytes != buf.len() {
                    // UDP, so this should never happen, but...
                    log::error!("tried to send {} bytes but only sent {}.", buf.len(), bytes);
                }
            }
            Err(e) => {
                log::error!("failed to send to UDP upstream: {}", e);
            }
        }
    }

    fn flush(&mut self) {
        if self.buf_used > 0 {
            self.send_buffer(&self.buffer[..self.buf_used]);
            self.buf_used = 0;
        }
        self.last_sent_at = SystemTime::now(); // Annoyingly superfluous call to now().
    }

    fn timed_flush(&mut self) {
        let now = SystemTime::now();
        if now
            .duration_since(self.last_sent_at)
            .map_or(true, |x| x > Duration::from_secs(1))
        {
            // We have not sent any metrics in a while. Flush the buffer.
            self.flush();
        }
    }
}

impl Drop for Upstream {
    fn drop(&mut self) {
        self.flush();
    }
}

impl Middleware for Upstream {
    fn submit(&mut self, metric: &mut Metric) {
        let metric_len = metric.raw.len();
        if metric_len + 1 > BUFSIZE - self.buf_used {
            // Message bigger than space left in buffer. Flush the buffer.
            self.flush();
        }
        if metric_len > BUFSIZE {
            // Message too big for the entire buffer, send it and pray.
            self.send_buffer(&metric.raw);
        } else {
            // Put the message in the buffer, separating it from the previous message if any.
            if self.buf_used > 0 {
                self.buffer[self.buf_used] = b'\n';
                self.buf_used += 1;
            }
            self.buffer[self.buf_used..self.buf_used + metric_len].copy_from_slice(&metric.raw);
            self.buf_used += metric_len;
        }
        // poll gets called before submit, so if the buffer needed to be flushed for time reasons,
        // it already was.
    }

    fn poll(&mut self) {
        self.timed_flush();
    }
}
