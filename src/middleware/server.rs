use std::io::ErrorKind;
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Error;

use crate::middleware::Middleware;
use crate::types::Metric;

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
        // An acceptable balance between busyloop and responsiveness to signals.
        socket.set_read_timeout(Some(Duration::from_secs(1)))?;
        Ok(Server { socket, middleware })
    }

    pub fn run(mut self) -> Result<(), Error> {
        // if sending this large udp dataframes happens to work randomly, we should not be the
        // one that breaks that setup.
        let mut buf = [0; 65535];

        let stop = Arc::new(AtomicBool::new(false));

        // This block is basically useless on windows. Would need to implement as a full fledged
        // service.
        #[cfg(not(windows))] // No SIGHUP on windows.
        signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&stop))?;
        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&stop))?;
        signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&stop))?;

        while !stop.load(Ordering::Relaxed) {
            let (num_bytes, _app_socket) = match self.socket.recv_from(buf.as_mut_slice()) {
                Err(err) => match err.kind() {
                    // Different timeout errors might be raised depending on platform.
                    ErrorKind::WouldBlock | ErrorKind::TimedOut => {
                        // Allow the middlewares to do any needed bookkeeping.
                        self.middleware.poll();
                        continue;
                    }
                    _ => return Err(Error::from(err)),
                },
                Ok(s) => s,
            };
            for raw in buf[..num_bytes].split(|&x| x == b'\n') {
                if raw.is_empty() {
                    continue;
                }

                let raw = raw.to_owned();
                let metric = Metric::new(raw);

                self.middleware.poll();
                self.middleware.submit(metric);
            }
        }
        Ok(())
    }
}
