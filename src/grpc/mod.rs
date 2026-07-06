use anyhow::{Result, Context};
use std::time::Duration;
use tonic::transport::{Channel, Endpoint};
use crate::cli::{UserConnectArgs, RunnerStartArgs};

pub mod proto {
    tonic::include_proto!("cli.runner.v1");
    tonic::include_proto!("cli.user.v1");
}
use proto::runner_cli_service_client::RunnerCliServiceClient;
use proto::{RegisterRunnerRequest, RegisterRunnerResponse, RunnerResources, UpdateMeshIpRequest};
use proto::user_cli_service_client::UserCliServiceClient;
use proto::{PlayerConnectRequest, PlayerConnectResponse};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub struct CpRunnerClient {
    inner: RunnerCliServiceClient<Channel>,
}

pub struct CpPlayerClient {
    inner: UserCliServiceClient<Channel>,
}

impl CpPlayerClient {
    pub async fn connect(addr: &str) -> Result<Self> {
        let endpoint = Endpoint::from_shared(format!("https://{}", addr))?
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT); // таймаут на КАЖДЫЙ запрос через этот канал по умолчанию

        let channel = endpoint
            .connect()
            .await
            .with_context(|| format!("не удалось подключиться к control-plane по адресу {addr} за {CONNECT_TIMEOUT:?}"))?;

        Ok(Self {
            inner: UserCliServiceClient::new(channel),
        })
    }

    pub async fn player_connect(&mut self, args: &UserConnectArgs) -> Result<PlayerConnectResponse> {
        let request = tonic::Request::new(PlayerConnectRequest {
            cli_version: env!("CARGO_PKG_VERSION").to_string(),
            join_secret: args.join_secret.clone(),
            platform: std::env::consts::OS.to_string(),
        });

        let response = self
            .inner
            .player_connect(request)
            .await
            .context("player_connect: control-plane не ответил вовремя или вернул ошибку")?;

        Ok(response.into_inner())
    }
}

impl CpRunnerClient {
    pub async fn connect(addr: &str) -> Result<Self> {
        let endpoint = Endpoint::from_shared(format!("https://{}", addr))?
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT);

        let channel = endpoint
            .connect()
            .await
            .with_context(|| format!("не удалось подключиться к control-plane по адресу {addr} за {CONNECT_TIMEOUT:?}"))?;

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

        let response = self
            .inner
            .register_runner(request)
            .await
            .context("register_runner: control-plane не ответил вовремя или вернул ошибку")?;

        Ok(response.into_inner())
    }
}