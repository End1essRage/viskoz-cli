use clap::{Parser, Subcommand, Args};

pub mod start;
pub mod stop;
pub mod status;
 
#[derive(Parser)]
#[command(name = "runner-cli", about = "GameHost runner manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}
 
#[derive(Subcommand)]
pub enum Commands {
    /// Запустить runner
    Start(StartArgs),
    /// Остановить runner
    Stop,
    /// Статус runner'а
    Status,
}
 
#[derive(Args)]
pub struct StartArgs {
    /// Адрес control-plane
    #[arg(long, env = "CP_ADDRESS")]
    pub cp_address: String,

    #[arg(long, env = "JOIN_SECRET")]
    pub join_secret: String,
 
    /// CPU cores доступные для runner'а
    #[arg(long, default_value = "2")]
    pub cpu_cores: u32,
 
    /// RAM в MB
    #[arg(long, default_value = "4096")]
    pub memory_mb: u64,
 
    /// Disk в MB
    #[arg(long, default_value = "20480")]
    pub disk_mb: u64,
}
 
