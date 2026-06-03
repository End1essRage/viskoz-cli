use anyhow::{bail, Context, Result};
use std::process::Command;
use tracing::{info, warn};

pub async fn check_tailscaled() -> Result<()> {
    // 1. Проверка наличия бинарного файла
    check_tailscaled_binary()?;
    
    // 2. Проверка запущенного процесса
    check_tailscaled_process()?;
    
    // 3. Проверка соединения с tailscale
    check_tailscale_connection().await?;
    
    info!("tailscaled: OK (binary found, service running, connected)");
    Ok(())
}

fn check_tailscaled_binary() -> Result<()> {
    let output = Command::new("which")
        .arg("tailscaled")
        .output()
        .or_else(|_| Command::new("command").args(["-v", "tailscaled"]).output());

    match output {
        Ok(o) if o.status.success() => {
            info!("✓ tailscaled binary found");
            Ok(())
        }
        _ => bail!("tailscaled binary not found. Install tailscale: https://tailscale.com/download"),
    }
}

fn check_tailscaled_process() -> Result<()> {
    // Проверка через pidof
    let output = Command::new("pidof")
        .arg("tailscaled")
        .output();
    
    if let Ok(o) = output {
        if o.status.success() && !o.stdout.is_empty() {
            let pids = String::from_utf8_lossy(&o.stdout);
            info!("✓ tailscaled process running (PIDs: {})", pids.trim());
            return Ok(());
        }
    }
    
    // Альтернативная проверка через pgrep
    let output = Command::new("pgrep")
        .args(["-x", "tailscaled"])
        .output();
    
    if let Ok(o) = output {
        if o.status.success() && !o.stdout.is_empty() {
            info!("✓ tailscaled process running");
            return Ok(());
        }
    }
    
    // Проверка через systemd
    #[cfg(target_os = "linux")]
    {
        let status = Command::new("systemctl")
            .args(["is-active", "--quiet", "tailscaled"])
            .status();
        
        if let Ok(status) = status {
            if status.success() {
                info!("✓ tailscaled service active");
                return Ok(());
            }
        }
    }
    
    bail!(
        "tailscaled is not running.\n\n\
        To start tailscaled:\n\
        • Linux (systemd): sudo systemctl start tailscaled\n\
        • Linux (manual): sudo tailscaled\n\
        • macOS: sudo tailscaled\n\
        • After starting, run: sudo tailscale up\n\n\
        For more info: https://tailscale.com/download"
    )
}

async fn check_tailscale_connection() -> Result<()> {
    // Проверка, что tailscale настроен и подключен
    let output = Command::new("tailscale")
        .arg("status")
        .output();
    
    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.contains("stopped") || stdout.contains("not connected") {
                warn!("tailscale is installed but not connected");
                info!("Run 'sudo tailscale up' to connect to a tailnet");
                // Не возвращаем ошибку, так как это может быть нормально
                // для некоторых сценариев
            } else {
                // Извлекаем IP адрес из вывода
                if let Some(ip) = extract_tailscale_ip(&stdout) {
                    info!("✓ tailscale connected (IP: {})", ip);
                } else {
                    info!("✓ tailscale connected");
                }
            }
            Ok(())
        }
        Ok(o) => {
            // Команда выполнилась с ошибкой
            let stderr = String::from_utf8_lossy(&o.stderr);
            warn!("tailscale status check failed: {}", stderr);
            Ok(()) // Не фатально
        }
        Err(_) => {
            // tailscale command failed или не установлен
            warn!("Unable to check tailscale connection status");
            Ok(()) // Не фатально, так как процесс tailscaled может быть запущен
        }
    }
}

fn extract_tailscale_ip(status_output: &str) -> Option<String> {
    // Пример вывода tailscale status:
    // 100.64.0.1     my-machine    my-machine      active  -
    for line in status_output.lines() {
        if line.contains("active") && !line.contains("offline") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if !parts.is_empty() && parts[0].contains('.') {
                return Some(parts[0].to_string());
            }
        }
    }
    None
}