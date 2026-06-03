use anyhow::{Result, bail, Context};

// Remove the mod process block and make the helper functions accessible
async fn run_tailscale(args: &[&str]) -> Result<String> {
    let binary = if cfg!(windows) { "tailscale.exe" } else { "tailscale" };
    
    let output = tokio::process::Command::new(binary)
        .args(args)
        .output()
        .await
        .context("tailscale not found. Install: https://tailscale.com/download")?;

    if !output.status.success() {
        bail!(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// This function signature matches what start.rs expects: &str parameters
pub async fn up(login_server: &str, auth_key: &str) -> Result<String> {
    run_tailscale(&[
        "up",
        &format!("--login-server={}", login_server),
        &format!("--authkey={}", auth_key),
        "--accept-routes",
    ]).await?;
    
    // Get the mesh IP after successful connection
    get_ip().await
}

pub async fn down() -> Result<()> {
    run_tailscale(&["down"]).await?;
    Ok(())
}

pub async fn get_ip() -> Result<String> {
    run_tailscale(&["ip", "--4"]).await
}

pub async fn is_running() -> bool {
    run_tailscale(&["status"]).await.is_ok()
}