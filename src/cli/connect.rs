use anyhow::Result;
use tracing::info;

use crate::cli::UserConnectArgs;
use crate::config::Config;
use crate::grpc::CpClient;
use crate::platform;
use crate::tailscale;
 
pub async fn handle(args: UserConnectArgs) -> Result<()> {
    // 1. Проверяем платформенные зависимости
    info!("Checking platform dependencies...");
    platform::check_tailscaled().await?;
 
    // 2. Регистрируемся в CP, как юзер
    info!("Registering with control-plane at {}...", args.cp_address);
    let mut cp = CpClient::connect(&args.cp_address).await?;
 

    Ok(())
}