use anyhow::{Result, Context};
use tracing::{info, error};
use bollard::Docker;
use bollard::auth::DockerCredentials;
use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::{CreateContainerOptions, LogsOptions};
use futures_util::StreamExt;
use crate::grpc::proto::RegisterRunnerResponse;
use crate::cli::RunnerStartArgs;

use base64::{Engine as _, engine::general_purpose::URL_SAFE};
use serde_json::json;

pub async fn start(reg: &RegisterRunnerResponse, args: &RunnerStartArgs) -> Result<()> {
    let docker = Docker::connect_with_local_defaults()?;

    info!("Runner image from CP: '{}'", reg.runner_image);
    info!("Registry endpoint: '{}'", reg.registry_endpoint);
    info!("mesh cp endpoint: '{}'", reg.cp_mesh_address);
    pull_image(
        &docker,
        &reg.runner_image,
        &reg.registry_endpoint,
        &reg.registry_username,
        &reg.registry_password,
    ).await?;

    let env = vec![
        format!("CONNECTOR_CP_ADDRESS={}", reg.cp_mesh_address),
        format!("CONNECTOR_INSECURE=true"),
        format!("CONNECTOR_TOKEN={}", reg.runner_token),
        format!("CONNECTOR_SEND_BUF=50"),
        format!("CONNECTOR_RECV_BUF=20"),

        format!("REGISTRY_ENDPOINT={}", reg.registry_endpoint),
        format!("REGISTRY_USERNAME={}", reg.registry_username),
        format!("REGISTRY_PASSWORD={}", reg.registry_password),
        format!("REGISTRY_INSECURE=true"),
        format!("REGISTRY_TOKEN="),

        format!("HOST_DATA_PATH={}", args.host_data_path),  // нужно добавить в RunnerStartArgs
        format!("RUNNER_MOUNT_DIR=/data"),
        format!("STEAM_CMD_IMAGE=steamcmd:latest"),
        format!("HEARTBEATS_INTERVAL_SEC=5"),
        format!("METRICS_INTERVAL_SEC=30"),
    ];

    let host_config = HostConfig {
        binds: Some(vec![
            "/var/run/docker.sock:/var/run/docker.sock".to_string(),
            format!("{}:/data", args.host_data_bind),
        ]),
        network_mode: Some("host".to_string()),  // ← контейнер видит сеть хоста включая tailscale0
        dns: Some(vec!["100.100.100.100".to_string()]),
        nano_cpus: Some((args.cpu_cores as i64) * 1_000_000_000),
        memory: Some((args.memory_mb as i64) * 1024 * 1024),
        ..Default::default()
    };

    let config = ContainerCreateBody {
        image: Some(reg.runner_image.clone()),
        env: Some(env),
        host_config: Some(host_config),
        ..Default::default()
    };

    let container = docker.create_container(
        None::<CreateContainerOptions>,
        config
    ).await?;

    let container_id = container.id;
    info!("Container created: {}", container_id);

    // Запускаем
    docker.start_container(&container_id, None).await?;
    info!("Container started: {}", container_id);

    // Проверяем статус через секунду
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let inspect = docker.inspect_container(&container_id, None).await?;
    let state = inspect.state.context("no container state")?;
    
    let running = state.running.unwrap_or(false);
    let exit_code = state.exit_code.unwrap_or(-1);
    let status = state.status.map(|s| format!("{:?}", s)).unwrap_or_default();

    info!("Container status: running={} exit_code={} status={}", running, exit_code, status);

    if !running {
        // Достаём логи если упал
        let logs = docker.logs(
            &container_id,
            Some(LogsOptions {
                stdout: true,
                stderr: true,
                tail: "50".to_string(),
                ..Default::default()
            }),
        );
        use futures_util::StreamExt;
        let logs: Vec<_> = logs.collect().await;
        for log in logs {
            if let Ok(output) = log {
                error!("Container log: {}", output);
            }
        }
        anyhow::bail!("Container exited with code {}", exit_code);
    }

    Ok(())
}

fn build_registry_auth(username: &str, password: &str, server_address: &str) -> DockerCredentials {
    DockerCredentials {
        username: Some(username.to_string()),
        password: Some(password.to_string()),
        serveraddress: Some(server_address.to_string()),
        ..Default::default()
    }
}

fn build_full_image_name(registry_endpoint: &str, image: &str) -> String {
    if registry_endpoint.is_empty() {
        return image.to_string();
    }
    let host = registry_endpoint
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/');
    format!("{}/{}", host, image)
}

pub async fn pull_image(
    docker: &Docker,
    image: &str,
    registry_endpoint: &str,
    username: &str,
    password: &str,
) -> Result<()> {
    let full_image = build_full_image_name(registry_endpoint, image);
    info!("Pulling image: {}", full_image);

    let credentials = if !username.is_empty() && !password.is_empty() {
        Some(build_registry_auth(username, password, registry_endpoint))
    } else {
        None
    };

    let options = bollard::query_parameters::CreateImageOptions {
        from_image: Some(full_image.clone()),
        ..Default::default()
    };

    let mut stream = docker.create_image(Some(options), None, credentials);

    while let Some(msg) = stream.next().await {
        match msg {
            Ok(info) => {
                if let Some(status) = &info.status {
                    if let Some(detail) = &info.progress_detail {
                        info!("Pull progress: {} {:?}", status, detail);
                    }
                    if status.contains("Downloaded newer image") || status.contains("Image is up to date") {
                        info!("Pull completed: {}", status);
                    }
                }
                if let Some(err) = &info.error_detail {
                    anyhow::bail!("Pull error: {:?}", err);
                }
            }
            Err(e) => anyhow::bail!("Pull stream error: {}", e),
        }
    }

    Ok(())
}