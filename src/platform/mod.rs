use anyhow::{bail, Result};
use bollard::plugin::{DeviceMapping};
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

pub fn get_host_ids() -> (u32, u32){
    #[cfg(target_os = "windows")]
    return windows::get_host_ids();
 
    #[cfg(not(target_os = "windows"))]
    return linux::get_host_ids();
}

pub fn get_docker_gid() -> Result<u32>{
    #[cfg(target_os = "windows")]
    return windows::get_docker_gid();
 
    #[cfg(not(target_os = "windows"))]
    return linux::get_docker_gid();
}

pub fn unix_only_devices() -> Option<Vec<DeviceMapping>>{
    #[cfg(target_os = "windows")]
    return windows::unix_only_devices();
 
    #[cfg(not(target_os = "windows"))]
    return linux::unix_only_devices();
}

pub fn docker_sock_bind() -> String{
    #[cfg(target_os = "windows")]
    return windows::docker_sock_bind();
 
    #[cfg(not(target_os = "windows"))]
    return linux::docker_sock_bind();
}

pub fn docker_group_add() ->  Result<Option<Vec<String>>>{
    #[cfg(target_os = "windows")]
    return windows::docker_group_add();
 
    #[cfg(not(target_os = "windows"))]
    return linux::docker_group_add();
}

pub fn unix_only_cap_add() -> Option<Vec<String>>{
    #[cfg(target_os = "windows")]
    return windows::unix_only_cap_add();
 
    #[cfg(not(target_os = "windows"))]
    return linux::unix_only_cap_add();
}