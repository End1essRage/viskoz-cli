use anyhow::Result;
 
pub async fn handle() -> Result<()> {
    let config = crate::config::Config::load()?;
    println!("CP Address: {}", config.cp_address);
    println!("Mesh IP:    {}", config.mesh_ip);
    // TODO: дёрнуть CP через gRPC GetRunnerStatus
    Ok(())
}