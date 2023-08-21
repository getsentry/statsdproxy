use std::{io, sync::Arc};
use tokio::net::UdpSocket;

use anyhow::Error;

use crate::types::Metric;

pub struct Overloaded {
    pub metric: Metric,
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
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Upstream {
    pub async fn new(upstream: String) -> Result<Self, Error> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        // cloudflare says connect() allows some kernel-internal optimizations on Linux
        // https://blog.cloudflare.com/everything-you-ever-wanted-to-know-about-udp-sockets-but-were-afraid-to-ask-part-1/
        socket.connect(upstream).await?;
        Ok(Upstream {
            socket: Arc::new(socket),
            handle: None,
        })
    }
}

impl Middleware for Upstream {
    fn submit(&mut self, metric: Metric) -> Result<(), Overloaded> {
        if let Some(ref mut handle) = self.handle {
            if !handle.is_finished() {
                return Err(Overloaded { metric });
            } else {
                self.handle = None;
            }
        }

        // TODO: we should attempt to buffer up and send multiple metrics within a single udp
        // dataframe. this comes with the risk of creating too large udp payloads (causing data
        // loss), and it's not clear to me how one is supposed to detect this case beforehand.

        // this "fast path" doubles the throughput on this test:
        //
        // nc -ul 8081 | pv > /dev/null
        // cargo run --release -- --listen 127.0.0.1:8080 --upstream 127.0.0.1:8081
        // cat /dev/zero | nc -u 127.0.0.1 8080
        //
        // ...since on loopback there is no blocking, we actually never spawn a task.
        //
        // with fast path: 170MiB/s
        // without fast path: 60MiB/s
        // bare netcat: 330MiB/s
        //
        // tested on MacOS Ventura 13.4, Apple M2
        match self.socket.try_send(&metric.raw) {
            // unclear whether we would be doing partial writes, but cadence does it the same way
            Ok(_) => return Ok(()),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("[upstream submit fast] I/O error: {}", e),
        };

        // if the socket buffer is full, let's just spawn a task. this path is pretty slow but
        // (from what we know) the important usecase is running this proxy on localhost

        let socket = self.socket.clone();

        self.handle = Some(tokio::task::spawn(async move {
            socket
                .send(&metric.raw)
                .await
                .expect("[upstream submit slow] I/O error");
        }));

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
    pub async fn new(listen: String, middleware: M) -> Result<Self, Error> {
        let socket = UdpSocket::bind(&listen).await?;
        Ok(Server { socket, middleware })
    }

    pub async fn run(mut self) -> Result<(), Error> {
        // if sending this large udp dataframes happens to work randomly, we should not be the
        // one that breaks that setup.
        let mut buf = [0; 65535];

        loop {
            let (num_bytes, _app_socket) = self.socket.recv_from(buf.as_mut_slice()).await?;
            for raw in buf[..num_bytes].split(|&x| x == b'\n') {
                if raw.is_empty() {
                    continue;
                }

                let raw = raw.to_owned();
                let metric = Metric::new(raw);

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
