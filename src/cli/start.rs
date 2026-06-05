use anyhow::{Result,Context};
use tracing::{info,error};

use crate::cli::RunnerStartArgs;
use crate::config::Config;
use crate::grpc::CpRunnerClient;
use crate::platform;
use crate::tailscale;
use crate::runner;
 
pub async fn handle(args: RunnerStartArgs) -> Result<()> {
    // 1. Проверяем платформенные зависимости
    info!("Checking platform dependencies...");
    platform::check_docker().await?;
    platform::check_tailscaled().await?;
 
    // 2. Регистрируемся в CP, получаем всё необходимое
    info!("Registering with control-plane at {}...", args.cp_address_runner);
    let mut cp = CpRunnerClient::connect(&args.cp_address_runner).await?;
    let reg = cp.register_runner(&args).await?;

    info!("CP MESH IP: {}", reg.cp_mesh_address);
 
    // 3. Поднимаем tailscale
    info!("Bringing up tailscale mesh...");
    let mesh_ip = tailscale::up(&reg.headscale_url, &reg.headscale_auth_key).await?;
    info!("Mesh IP: {}", mesh_ip);

    // 4. Сохраняем состояние локально
    info!("Saving config...");
    let config = Config {
        //TODO
        cp_runner_addr: reg.cp_mesh_address.clone(),
        mesh_ip: mesh_ip.clone(),
    };
    
    // Добавьте отладку
    let config_path = std::env::current_dir()
        .context("Failed to get current directory")?
        .join("mgs-config.toml");
    info!("Will save config to: {:?}", config_path);
    
    match config.save() {
        Ok(_) => info!("Config saved successfully"),
        Err(e) => {
            error!("Failed to save config: {}", e);
            // Не фатально, продолжаем
        }
    }

    // 5. Передаем полученный меш айпи в контрол плейн
    let upd = cp.update_mesh_ip(reg.runner_token.clone(),mesh_ip.clone()).await?;
    // 6. Запускаем runner контейнер
    info!("Starting runner container...");
    runner::start(&reg, &args).await?;
 
    info!("Runner started successfully");
    Ok(())
}