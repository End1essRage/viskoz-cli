use anyhow::Result;
use tracing::info;

use crate::cli::StartArgs;
use crate::config::Config;
use crate::grpc::CpClient;
use crate::platform;
use crate::tailscale;
use crate::runner;
 
pub async fn handle(args: StartArgs) -> Result<()> {
    // 1. Проверяем платформенные зависимости
    info!("Checking platform dependencies...");
    platform::check_docker().await?;
    platform::check_tailscaled().await?;
 
    // 2. Регистрируемся в CP, получаем всё необходимое
    info!("Registering with control-plane at {}...", args.cp_address);
    let mut cp = CpClient::connect(&args.cp_address).await?;
    let reg = cp.register(&args).await?;
 
    // 3. Поднимаем tailscale
    info!("Bringing up tailscale mesh...");
    let mesh_ip = tailscale::up(&reg.headscale_url, &reg.headscale_auth_key).await?;
    info!("Mesh IP: {}", mesh_ip);
 
    // 4. Сохраняем состояние локально
    let config = Config {
        cp_address: args.cp_address.clone(),
        mesh_ip: mesh_ip.clone(),
        runner_token: reg.runner_token.clone(),
    };
    config.save()?;
 
    // 5. Запускаем runner контейнер
    info!("Starting runner container...");
    runner::start(&reg, &mesh_ip, &args).await?;
 
    info!("Runner started successfully");
    Ok(())
}