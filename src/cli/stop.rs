use anyhow::Result;
use tracing::info;
 
pub async fn handle() -> Result<()> {
    let config = crate::config::Config::load()?;
    
    // TODO: остановить контейнер через bollard, деregistрация в CP
    Ok(())
}
 
