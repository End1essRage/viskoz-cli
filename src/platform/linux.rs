use anyhow::{bail, Result};
use std::process::Command;
use tracing::info;
 
pub async fn check_tailscaled() -> Result<()> {
    // Проверяем через systemctl или просто наличие процесса
    let output = Command::new("systemctl")
        .args(["is-active", "tailscaled"])
        .output();
 
    match output {
        Ok(o) if o.status.success() => {
            info!("tailscaled: OK (systemd)");
            Ok(())
        }
        _ => {
            // Fallback — проверяем просто наличие процесса
            let output = Command::new("pgrep").arg("tailscaled").output();
            match output {
                Ok(o) if o.status.success() => {
                    info!("tailscaled: OK (process)");
                    Ok(())
                }
                _ => bail!(
                    "tailscaled not running. Install tailscale: https://tailscale.com/download"
                ),
            }
        }
    }
}