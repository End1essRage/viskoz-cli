use anyhow::{Result,Context};
use tracing::{info,error};

use crate::cli::RunnerStartArgs;
use crate::config::Config;
use crate::grpc::CpRunnerClient;
use crate::platform;
use crate::tailscale;
use crate::runner;
 
pub async fn handle(args: RunnerStartArgs) -> Result<()> {
    // Проверяем платформенные зависимости
    info!("Checking platform dependencies...");
    platform::check_docker().await?;
 
    // Регистрируемся в CP, получаем всё необходимое
    info!("Registering with control-plane at {}...", args.cp_address_runner);
    let mut cp = CpRunnerClient::connect(&args.cp_address_runner).await?;
    let reg = cp.register_runner(&args).await?;

    info!("CP MESH IP: {}", reg.cp_mesh_address);

    // Запускаем runner контейнер
    info!("Starting runner container...");
    runner::start(&reg, &args).await?;
 
    info!("Runner started successfully");
    Ok(())
}