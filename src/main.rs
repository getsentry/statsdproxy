use anyhow::Error;
use clap::Parser;

mod config;
mod middleware;
mod types;
use middleware::{Server, Upstream};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    listen: String,

    /// Specify an address to an upstream statsd server in 'host:port' format.
    #[arg(short, long)]
    upstream: String,
    // TODO: implement a middleware, a way of nesting them and a configuration file
    #[arg(short, long)]
    config_path: String,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let client = Upstream::new(args.upstream)?;
    let server = Server::new(args.listen, client)?;
    let _config = config::Config::new(&args.config_path)?;
    server.run()?;

    Ok(())
}
