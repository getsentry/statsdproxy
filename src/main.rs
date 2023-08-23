use anyhow::Error;
use clap::Parser;

mod config;
mod middleware;
mod types;

#[cfg(test)]
mod testutils;

use middleware::{Server, Upstream};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    listen: String,

    /// Specify an address to an upstream statsd server in 'host:port' format.
    #[arg(short, long)]
    upstream: String,

    /// Specify a configuration file to add middlewares. See example.yaml for which middlewares are
    /// supported.
    #[arg(short, long)]
    config_path: Option<String>,
}

fn main() -> Result<(), Error> {
    let args = Args::parse();

    let config = args
        .config_path
        .as_deref()
        .map(config::Config::new)
        .transpose()?
        .unwrap_or_default();

    let mut client: Box<dyn middleware::Middleware> = Box::new(Upstream::new(args.upstream)?);
    for middleware_config in config.middlewares.into_iter().rev() {
        match middleware_config {
            config::MiddlewareConfig::AllowTag(config) => {
                client = Box::new(middleware::allow_tag::AllowTag::new(config, client));
            }
            config::MiddlewareConfig::DenyTag(config) => {
                client = Box::new(middleware::deny_tag::DenyTag::new(config, client));
            }
            config::MiddlewareConfig::CardinalityLimit(config) => {
                client = Box::new(middleware::cardinality_limit::CardinalityLimit::new(
                    config, client,
                ));
            }
            config::MiddlewareConfig::AggregateMetrics(config) => {
                client = Box::new(middleware::aggregate::AggregateMetrics::new(config, client));
            }
            config::MiddlewareConfig::AddTag(config) => {
                client = Box::new(middleware::add_tag::AddTag::new(config, client));
            }
        }
    }

    let server = Server::new(args.listen, client)?;

    server.run()?;

    Ok(())
}
