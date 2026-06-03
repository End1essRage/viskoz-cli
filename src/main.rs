mod cli;
mod config;
mod grpc;
use tracing;
mod platform;
mod runner;
mod tailscale;
 
use anyhow::Result;
use cli::{Cli, Commands};
use clap::Parser;
use tracing::info;
 
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("runner_cli=debug".parse()?),
        )
        .init();
 
    let cli = Cli::parse();
 
    match cli.command {
        Commands::Start(args) => {
            info!("Starting runner-cli...");
            cli::start::handle(args).await?; 
        }
        Commands::Stop => {
            cli::stop::handle().await?;
        }
        Commands::Status => {
            cli::status::handle().await?;
        }
    }
 
    Ok(())
}
 
