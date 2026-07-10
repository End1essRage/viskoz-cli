use anyhow::{bail, Result};
use bollard::plugin::{DeviceMapping};
use std::process::Command;

pub fn get_host_ids() -> (u32, u32) {
    // На Windows/Docker Desktop нет родного понятия UID/GID —
    // юзер внутри Linux VM Docker Desktop всегда мапится в 1000:1000
    // при обычном bind-mount, так что это безопасный дефолт
    (1000, 1000)
}

pub fn get_docker_gid() -> Result<u32> {
    Ok(0)
}

pub fn unix_only_devices() -> Option<Vec<DeviceMapping>> {
    // TODO(windows): /dev/net/tun не существует в Windows-контейнерах.
    // Практический путь для Windows-хостов — гонять tailscaled как Windows-сервис
    // на хосте, а не как sidecar-контейнер, и подключать раннер к его localhost API.
    None
}

pub fn docker_sock_bind() -> String {
    // TODO(windows): для Docker Desktop это именованный пайп, а не unix-сокет.
    // Формат монтирования отличается и обычно требует --isolation=process
    // либо Linux-контейнеров через WSL2-backend, где /var/run/docker.sock
    // может быть доступен как обычно. Пока — заглушка под WSL2-режим.
    "//./pipe/docker_engine://./pipe/docker_engine".to_string()
}

pub fn docker_group_add() -> Result<Option<Vec<String>>> {
    // На Windows нет концепции unix-групп для сокета докера — не требуется.
    Ok(None)
}

pub fn unix_only_cap_add() -> Option<Vec<String>> {
    // TODO(windows): NET_ADMIN — linux capability, у Windows-контейнеров нет прямого аналога.
    None
}

pub async fn check_tailscaled() -> Result<()> {
    // Проверяем через systemctl или просто наличие процесса
    let output = Command::new("systemctl")
        .args(["is-active", "tailscaled"])
        .output();
 
    match output {
        Ok(o) if o.status.success() => {
            tracing::info!("tailscaled: OK (systemd)");
            Ok(())
        }
        _ => {
            // Fallback — проверяем просто наличие процесса
            let output = Command::new("pgrep").arg("tailscaled").output();
            match output {
                Ok(o) if o.status.success() => {
                    tracing::info!("tailscaled: OK (process)");
                    Ok(())
                }
                _ => bail!(
                    "tailscaled not running. Install tailscale: https://tailscale.com/download"
                ),
            }
        }
    }
}