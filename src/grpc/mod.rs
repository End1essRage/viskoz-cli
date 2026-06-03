use anyhow::{Result,Context};
use tonic::transport::Channel;
 
use crate::cli::RunnerStartArgs;
 
// Сгенерированный код из proto
pub mod proto {
    tonic::include_proto!("cli.runner.v1");
}
 
use proto::runner_cli_service_client::RunnerCliServiceClient;
use proto::{RegisterRunnerRequest, RegisterRunnerResponse, RunnerResources,UpdateMeshIpRequest};
 
pub struct CpClient {
    inner: RunnerCliServiceClient<Channel>,
}
 
impl CpClient {
    pub async fn connect(addr: &str) -> Result<Self> {
        let channel = Channel::from_shared(format!("http://{}", addr))?
            .connect()
            .await?;
        Ok(Self {
            inner: RunnerCliServiceClient::new(channel),
        })
    }
 
    pub async fn register_runner(&mut self, args: &RunnerStartArgs) -> Result<RegisterRunnerResponse> {
        let request = tonic::Request::new(RegisterRunnerRequest {
            cli_version: env!("CARGO_PKG_VERSION").to_string(),
            join_secret: args.join_secret.clone(),
            platform: std::env::consts::OS.to_string(),
            resources: Some(RunnerResources {
                cpu_cores: args.cpu_cores,
                memory_mb: args.memory_mb,
                disk_mb: args.disk_mb,
            }),
        });
 
        let response = self.inner.register_runner(request).await?;
        Ok(response.into_inner())
    }

    /// Обновление mesh IP после поднятия tailscale
    pub async fn update_mesh_ip(&mut self, runner_token: String, mesh_ip: String) -> Result<bool> {
        let request = tonic::Request::new(UpdateMeshIpRequest {
            runner_token: runner_token.to_string(),
            mesh_ip: mesh_ip.to_string(),
        });

        let response = self.inner.update_mesh_ip(request)
            .await
            .context("Failed to call UpdateMeshIP RPC")?;
        
        let inner = response.into_inner();
        
        Ok(inner.ok)
    }
}