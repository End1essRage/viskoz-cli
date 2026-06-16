use anyhow::{bail,Result, Context};
use std::process::Command;
use bollard::plugin::DeviceMapping;
use tracing::{info, error};
use bollard::Docker;
use bollard::auth::DockerCredentials;
use bollard::models::{ContainerCreateBody, HostConfig, NetworkCreateRequest};
use bollard::query_parameters::{CreateContainerOptions, LogsOptions};
use futures_util::StreamExt;
use crate::grpc::proto::RegisterRunnerResponse;
use crate::cli::RunnerStartArgs;

use base64::{Engine as _, engine::general_purpose::URL_SAFE};
use serde_json::json;
use uuid::Uuid;

pub async fn start(reg: &RegisterRunnerResponse, args: &RunnerStartArgs) -> Result<()> {
  let docker = Docker::connect_with_local_defaults()?;

    info!("Runner image from CP: '{}'", reg.runner_image);
    info!("Registry endpoint: '{}'", reg.registry_endpoint);
    info!("mesh cp endpoint: '{}'", reg.cp_mesh_address);

    // TODO pull sidecar image
    pull_image(
        &docker,
        &reg.runner_image,
        &reg.registry_endpoint,
        &reg.registry_username,
        &reg.registry_password,
    ).await?;

    // 1. Создаём изолированную сеть
    let net_name = format!("runner-net-{}", Uuid::new_v4().simple());
    info!("Creating network: {}", net_name);
    docker.create_network(NetworkCreateRequest {
        name: net_name.clone(),
        driver: Some("bridge".to_string()),
        ..Default::default()
    }).await?;

    // 2. Запускаем sidecar Tailscale (userspace)
    let sidecar_name = format!("runner-ts-{}", Uuid::new_v4().simple());

    let sidecar_env = vec![
        format!("TS_AUTHKEY={}", reg.headscale_auth_key.clone()),
        format!("TS_LOGIN_SERVER={}",reg.headscale_url.clone()),
        "TS_USERSPACE=false".to_string(),
        format!("TS_STATE_DIR=/var/lib/tailscale/{}", sidecar_name),
        format!("TS_HOSTNAME=runner-{}", Uuid::new_v4().simple()),
        format!("TS_EXTRA_ARGS=--accept-dns=false --login-server={} --accept-routes=false",reg.headscale_url.clone()),
        "TS_AUTH_ONCE=true".to_string(),
        "TS_ACCEPT_DNS=false".to_string(),
        "TS_ENABLE_HEALTH_CHECK=true".to_string(),
        "TS_LOCAL_ADDR_PORT=127.0.0.1:41234".to_string(),
    ];

    let sidecar_config = ContainerCreateBody {
        image: Some("tailscale/tailscale:latest".to_string()),
        env: Some(sidecar_env),
        volumes: Some(vec![format!("{}:/var/lib/tailscale", "ts-run-volume")]),
        host_config: Some(HostConfig {
            network_mode: Some(net_name.clone()),
            cap_add: Some(vec!["NET_ADMIN".to_string()]),
            devices: Some(vec![DeviceMapping{
                path_on_host: Some("/dev/net/tun".to_string()),
                path_in_container: Some("/dev/net/tun".to_string()),
                cgroup_permissions:Some("rwm".to_string())
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };

    let sidecar = docker.create_container(
        Some(CreateContainerOptions { platform: "linux".to_string() ,name: Some(sidecar_name.clone()) }),
        sidecar_config,
    ).await?;
    let sidecar_id = sidecar.id;
    info!("Tailscale sidecar created: {} (id: {})", sidecar_name, sidecar_id);

    docker.start_container(&sidecar_id, None).await?;
    info!("Tailscale sidecar started");

    // Даём sidecar время на регистрацию в Headscale
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // 3. Запускаем раннер в той же сети, НО с network_mode = "container:<sidecar_id>"
    //    Это даёт раннеру тот же сетевой стек, что и sidecar (и Tailscale IP)
    let runner_name = format!("runner-{}", Uuid::new_v4().simple());
    let env = vec![
        format!("CONNECTOR_CP_ADDRESS={}", reg.cp_mesh_address),
        "CONNECTOR_INSECURE=true".to_string(),
        format!("CONNECTOR_TOKEN={}", reg.runner_token),
        "CONNECTOR_SEND_BUF=50".to_string(),
        "CONNECTOR_RECV_BUF=20".to_string(),

        format!("REGISTRY_ENDPOINT={}", reg.registry_endpoint),
        format!("REGISTRY_USERNAME={}", reg.registry_username),
        format!("REGISTRY_PASSWORD={}", reg.registry_password),
        "REGISTRY_INSECURE=true".to_string(),
        "REGISTRY_TOKEN=".to_string(),

        format!("HOST_DATA_PATH={}", args.host_data_path),
        "RUNNER_MOUNT_DIR=/data".to_string(),
        "STEAM_CMD_IMAGE=steamcmd:latest".to_string(),
        "HEARTBEATS_INTERVAL_SEC=5".to_string(),
        "METRICS_INTERVAL_SEC=30".to_string(),

        // Передаём раннеру ID sidecar, чтобы он знал, в какой сети запускать дочерние контейнеры
        format!("CHILD_NETWORK_MODE=container:{}", sidecar_id),
    ];

    let full_image = build_full_image_name(&reg.registry_endpoint, &reg.runner_image);
    let docker_gid = get_docker_gid()?;
    let runner_config: ContainerCreateBody = ContainerCreateBody {
        image: Some(full_image.clone()),
        env: Some(env),
        host_config: Some(HostConfig {
            network_mode: Some(format!("container:{}", sidecar_id)),  // разделяем сеть sidecar
            group_add: Some(vec![docker_gid.to_string()]),
            binds: Some(vec![
                "/var/run/docker.sock:/var/run/docker.sock".to_string(),
                format!("{}:/data", args.host_data_bind),
            ]),
            nano_cpus: Some((args.cpu_cores as i64) * 1_000_000_000),
            memory: Some((args.memory_mb as i64) * 1024 * 1024),
            ..Default::default()
        }),
        ..Default::default()
    };

    let runner = docker.create_container(
        Some(CreateContainerOptions { platform: "Linux".to_string() , name: Some(runner_name.clone()) }),
        runner_config,
    ).await?;
    let runner_id = runner.id;
    info!("Runner container created: {} (id: {})", runner_name, runner_id);

    docker.start_container(&runner_id, None).await?;
    info!("Runner container started");

    // Даём раннеру время на запуск
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Проверяем статус раннера
    let inspect = docker.inspect_container(&runner_id, None).await?;
    let state = inspect.state.context("no container state")?;
    let running = state.running.unwrap_or(false);
    let exit_code = state.exit_code.unwrap_or(-1);

    if !running {
        error!("Runner container exited with code {}", exit_code);
        let logs = docker.logs(&runner_id, Some(LogsOptions {
            stdout: true,
            stderr: true,
            tail: "50".to_string(),
            ..Default::default()
        }));
        let logs: Vec<_> = logs.collect().await;
        for log in logs {
            if let Ok(output) = log {
                error!("Container log: {}", output);
            }
        }
        anyhow::bail!("Runner container exited with code {}", exit_code);
    }

    // Опционально: можно также проверить health sidecar
    info!("Runner is running. Sidecar ID: {}", sidecar_id);
    Ok(())
}

fn get_docker_gid() -> Result<u32> {
    let output = Command::new("getent")
        .args(["group", "docker"])
        .output()?;
    let line = String::from_utf8(output.stdout)?;
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() >= 3 {
        let gid = parts[2].parse::<u32>()?;
        Ok(gid)
    } else {
        bail!("docker group not found")
    }
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