use anyhow::{bail, Result};
use std::process::Command;
use tracing::info;
 
pub async fn check_tailscaled() -> Result<()> {
    let output = Command::new("which")
        .arg("tailscaled")
        .output()
        .or_else(|_| Command::new("command").args(["-v", "tailscaled"]).output());

    match output {
        Ok(o) if o.status.success() => {
            info!("tailscaled: OK (binary found)");
            Ok(())
        }
        _ => bail!("tailscaled binary not found. Install tailscale: https://tailscale.com/download"),
    }
}