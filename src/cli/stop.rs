use anyhow::Result;
use tracing::info;
 
pub async fn handle() -> Result<()> {
    let config = crate::config::Config::load()?;
    
    info!("Stopping runner ... {}",config.runner_token);
    // TODO: остановить контейнер через bollard, деregistрация в CP
    Ok(())
}
 
