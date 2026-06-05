mod cli;
mod config;
mod grpc;
use tracing;
mod platform;
mod runner;
mod tailscale;

use anyhow::Result;
use cli::{Cli, Commands, UserAction, RunnerAction};
use clap::Parser;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("mgs_cli=debug".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::User(user_cmd) => {
            match user_cmd.command {
                UserAction::Connect(args) => {
                    info!("Starting mesh...");
                    cli::connect::handle(args).await?;
                }
            }
        }
        Commands::Runner(runner_cmd) => {
            match runner_cmd.command {
                RunnerAction::Start(args) => {
                    info!("Starting runner...");
                    cli::start::handle(args).await?;
                }
                RunnerAction::Stop => {
                    cli::stop::handle().await?;
                }
                RunnerAction::Status => {
                    cli::status::handle().await?;
                }
            }
        }
    }

    Ok(())
}