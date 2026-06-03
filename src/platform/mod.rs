use anyhow::{bail, Result};
use tracing::info;
 
#[cfg(target_os = "windows")]
mod windows;
#[cfg(not(target_os = "windows"))]
mod linux;
 
/// Проверяем что docker daemon запущен и доступен
pub async fn check_docker() -> Result<()> {
    let client = bollard::Docker::connect_with_local_defaults()?;
    match client.ping().await {
        Ok(_) => {
            info!("Docker daemon: OK");
            Ok(())
        }
        Err(e) => bail!("Docker daemon not available: {}. Please start Docker.", e),
    }
}

/// Проверяем что tailscaled установлен и запущен
pub async fn check_tailscaled() -> Result<()> {
    #[cfg(target_os = "windows")]
    return windows::check_tailscaled().await;
 
    #[cfg(not(target_os = "windows"))]
    return linux::check_tailscaled().await;
}
 
