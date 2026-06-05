use anyhow::{Result,Context};
use tracing::{info,error};

use crate::cli::UserConnectArgs;
use crate::config::Config;
use crate::grpc::CpPlayerClient;
use crate::platform;
use crate::tailscale;
 
pub async fn handle(args: UserConnectArgs) -> Result<()> {
    // 1. Проверяем платформенные зависимости
    info!("Checking platform dependencies...");
    platform::check_tailscaled().await?;
 
    // 2. Регистрируемся в CP, как юзер
    info!("Registering with control-plane at {}...", args.cp_address_user);
    let mut cp = CpPlayerClient::connect(&args.cp_address_user).await?;
    let reg = cp.player_connect(&args).await?;
 
    // 3. Поднимаем tailscale
    info!("Bringing up tailscale mesh...");
    let mesh_ip = tailscale::up(&reg.headscale_url, &reg.headscale_auth_key).await?;
    info!("Mesh IP: {}", mesh_ip);

    // ? 5. Передаем полученный меш айпи в контрол плейн
  
    Ok(())
}