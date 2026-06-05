use clap::{Parser, Subcommand, Args};

pub mod start;
pub mod stop;
pub mod status;
pub mod connect;

#[derive(Parser)]
#[command(name = "mgs-cli", about = "GameHost runner manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Команды для управления пользователями
    User(UserCommands),
    /// Команды для управления раннером
    Runner(RunnerCommands),
}

// Убираем #[derive(Subcommand)] - теперь это обычное перечисление
// которое будет использоваться как аргументы для команды User
#[derive(clap::Args)]
pub struct UserCommands {
    #[command(subcommand)]
    pub command: UserAction,
}

// Убираем #[derive(Subcommand)] - теперь это обычное перечисление
// которое будет использоваться как аргументы для команды Runner
#[derive(clap::Args)]
pub struct RunnerCommands {
    #[command(subcommand)]
    pub command: RunnerAction,
}

#[derive(Subcommand)]
pub enum UserAction {
    /// Подключение пользователя (mesh)
    Connect(UserConnectArgs),
    // Здесь можно добавить другие действия для пользователя
    // List,
    // Remove,
}

#[derive(Subcommand)]
pub enum RunnerAction {
    /// Запустить runner + mesh
    Start(RunnerStartArgs),
    /// Остановить runner
    Stop,
    /// Статус runner'а
    Status,
}

#[derive(Args)]
pub struct UserConnectArgs {
    /// Адрес control-plane
    #[arg(long, env = "CP_ADDRESS_USER")]
    pub cp_address_user: String,

    #[arg(long, env = "JOIN_SECRET")]
    pub join_secret: String,
}

#[derive(Args)]
pub struct RunnerStartArgs {
    /// Адрес control-plane
    #[arg(long, env = "CP_ADDRESS_RUNNER")]
    pub cp_address_runner: String,

    #[arg(long, env = "JOIN_SECRET")]
    pub join_secret: String,

    #[arg(long, env = "HOST_DATA_PATH")]
    pub host_data_path: String,

    #[arg(long, env = "HOST_DATA_BIND")]
    pub host_data_bind: String,

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