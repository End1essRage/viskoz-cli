use anyhow::{bail, Result, Context};
use std::process::Command;
use std::os::unix::fs::MetadataExt;
use bollard::plugin::{DeviceMapping, VolumeCreateRequest};
use tracing::{info, error, warn};
use bollard::Docker;
use bollard::auth::DockerCredentials;
use bollard::models::{ContainerCreateBody, HealthConfig, HostConfig, NetworkCreateRequest};
use bollard::query_parameters::{CreateContainerOptions, LogsOptions};
use futures_util::StreamExt;
use crate::grpc::proto::RegisterRunnerResponse;
use crate::cli::RunnerStartArgs;

const CONNECTOR_VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn start(reg: &RegisterRunnerResponse, args: &RunnerStartArgs) -> Result<()> {
    let docker = Docker::connect_with_local_defaults()?;

    info!("Runner image from CP: '{}'", reg.runner_image);
    info!("Registry endpoint: '{}'", reg.registry_endpoint);
    info!("mesh cp endpoint: '{}'", reg.cp_mesh_address);
    info!("runner_token aka runner_id: '{}'", reg.runner_token);

    //TODO
    let runner_id = reg.runner_token.clone();

    pull_image(
        &docker,
        &reg.runner_image,
        &reg.registry_endpoint,
        &reg.registry_username,
        &reg.registry_password,
    ).await?;

    //TODO
    // 1. Создаём изолированную сеть
    let net_name = format!("runner-net-{}", runner_id);

    match docker.create_network(NetworkCreateRequest {
        name: net_name.clone(),
        driver: Some("bridge".to_string()),
        ..Default::default()
    }).await {
    Ok(_) => info!("Сеть '{}' создана", net_name),
    Err(e) if e.to_string().contains("Conflict") || e.to_string().contains("already exists") => {
        warn!("Сеть '{}' уже существует, используем существующую", net_name);
        // Ничего не делаем, продолжаем
    }
    Err(e) => {
        error!("Не удалось создать сеть '{}': {}", net_name, e);
        return Err(e.into()); // пробрасываем другие ошибки
    }
}

    // 2. Запускаем sidecar Tailscale (userspace=false, как в compose)
    let sidecar_name = format!("runner-ts-{}", runner_id);

    let sidecar_env = vec![
        format!("TS_AUTHKEY={}", reg.headscale_auth_key.clone()),
        format!("TS_LOGIN_SERVER={}", reg.headscale_url.clone()),
        "TS_USERSPACE=false".to_string(),
        "TS_STATE_DIR=/var/lib/tailscale".to_string(),
        format!("TS_HOSTNAME=runner-{}", runner_id),
        format!(
            "TS_EXTRA_ARGS=--accept-dns=false --login-server={} --accept-routes=false",
            reg.headscale_url.clone()
        ),
        "TS_AUTH_ONCE=true".to_string(),
        "TS_ACCEPT_DNS=false".to_string(),
        "TS_ENABLE_HEALTH_CHECK=true".to_string(),
        "TS_LOCAL_ADDR_PORT=127.0.0.1:41234".to_string(),
        // добавлено для соответствия compose — сокет тейлскейлда,
        // на случай если другим процессам в сети понадобится дергать tailscaled напрямую
        "TS_SOCKET=/var/run/tailscale-run/tailscaled.sock".to_string(),
    ];

    let volume_name = format!("ts-run-volume-{}", runner_id);
    let request = VolumeCreateRequest {
        name: Some(volume_name.to_string()),
        driver: Some("local".to_string()),       // драйвер по умолчанию
        driver_opts: None,                 // можно задать опции (например, размер)
        labels: None,                      // метки для тома
        cluster_volume_spec: None,         // для Swarm (не нужно)
    };

    docker.create_volume(request).await?;

    let sidecar_config = ContainerCreateBody {
        image: Some("tailscale/tailscale:latest".to_string()),
        env: Some(sidecar_env),
        //volumes: Some(vec![format!("{}:/var/lib/tailscale", "ts-run-volume")]),
        // healthcheck — 1-в-1 как в вашем compose, чтобы ждать реальной готовности,
        // а не гадать со sleep()
        healthcheck: Some(HealthConfig {
            test: Some(vec![
                "CMD".to_string(),
                "wget".to_string(),
                "--spider".to_string(),
                "-q".to_string(),
                "http://127.0.0.1:41234/healthz".to_string(),
            ]),
            interval: Some(5_000_000_000),       // 5s между попытками (быстрее, чем в compose — там 30s, но при старте раннера ждать полминуты дорого)
            timeout: Some(10_000_000_000),        // 10s
            retries: Some(3),
            start_period: Some(5_000_000_000),    // 5s
            start_interval: Some(5_000_000_000),
        }),
        host_config: Some(HostConfig {
            network_mode: Some(net_name.clone()),
            cap_add: unix_only_cap_add(),
            devices: unix_only_devices(),
            binds: Some(vec![
                format!("{}:/var/lib/tailscale", "ts-run-volume"),
            ]),
            ..Default::default()
        }),
        ..Default::default()
    };

    let sidecar = docker.create_container(
        Some(CreateContainerOptions { platform: "linux".to_string(), name: Some(sidecar_name.clone()) }),
        sidecar_config,
    ).await?;
    let sidecar_id = sidecar.id;
    info!("Tailscale sidecar created: {} (id: {})", sidecar_name, sidecar_id);

    docker.start_container(&sidecar_id, None).await?;
    info!("Tailscale sidecar started, waiting for healthcheck...");

    wait_for_healthy(&docker, &sidecar_id, 60).await
        .context("sidecar не прошёл healthcheck вовремя")?;
    info!("Tailscale sidecar healthy");

    // 3. Запускаем раннер в сети sidecar
    let (local_uid, local_gid) = get_host_ids();

    let runner_name = format!("runner-{}", runner_id);
    let mut env = vec![
        format!("LOCAL_UID={}", local_uid),
        format!("LOCAL_GID={}", local_gid),
        format!("CONNECTOR_CP_ADDRESS={}", reg.cp_mesh_address),
        "CONNECTOR_INSECURE=true".to_string(),
        format!("CONNECTOR_TOKEN={}", reg.runner_token),
        format!("CONNECTOR_VERSION={}", CONNECTOR_VERSION),
        "CONNECTOR_SEND_BUF=50".to_string(),
        "CONNECTOR_RECV_BUF=20".to_string(),

        format!("REGISTRY_ENDPOINT={}", reg.registry_endpoint),
        format!("REGISTRY_USERNAME={}", reg.registry_username),
        format!("REGISTRY_PASSWORD={}", reg.registry_password),
        "REGISTRY_INSECURE=true".to_string(),
        "REGISTRY_TOKEN=".to_string(),

        format!("HOST_DATA_PATH={}", args.host_data_path),
        "HEARTBEATS_INTERVAL_SEC=5".to_string(),
        "METRICS_INTERVAL_SEC=30".to_string(),

        format!("CHILD_NETWORK_MODE=container:{}", sidecar_id),
    ];

    match get_docker_gid() {
        Ok(docker_gid) => {
            env.push(format!("DOCKER_GID={}", docker_gid));
        }
        Err(e)=> {
             info!("DOCKER_GID не определен");
        }
    }

    let full_image = build_full_image_name(&reg.registry_endpoint, &reg.runner_image);
    let runner_config: ContainerCreateBody = ContainerCreateBody {
        image: Some(full_image.clone()),
        env: Some(env),
        host_config: Some(HostConfig {
            network_mode: Some(format!("container:{}", sidecar_id)),
            //group_add: docker_group_add()?,
            binds: Some(vec![
                docker_sock_bind(),
                format!("{}:/data", args.host_data_path),
            ]),
            nano_cpus: Some((args.cpu_cores as i64) * 1_000_000_000),
            memory: Some((args.memory_mb as i64) * 1024 * 1024),
            ..Default::default()
        }),
        ..Default::default()
    };

    let runner = docker.create_container(
        Some(CreateContainerOptions { platform: "linux".to_string(), name: Some(runner_name.clone()) }),
        runner_config,
    ).await?;
    let runner_id = runner.id;
    info!("Runner container created: {} (id: {})", runner_name, runner_id);

    docker.start_container(&runner_id, None).await?;
    info!("Runner container started");

    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

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

    info!("Runner is running. Sidecar ID: {}", sidecar_id);
    Ok(())
}

/// Ждёт, пока контейнер получит health-статус "healthy", как `condition: service_healthy` в compose.
async fn wait_for_healthy(docker: &Docker, container_id: &str, timeout_secs: u64) -> Result<()> {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
    loop {
        let inspect = docker.inspect_container(container_id, None).await?;
        if let Some(state) = &inspect.state {
            if let Some(health) = &state.health {
                match health.status.as_ref().map(|s| s.to_string()).as_deref() {
                    Some("healthy") => return Ok(()),
                    Some("unhealthy") => bail!("контейнер {container_id} стал unhealthy"),
                    _ => {}
                }
            }
            if !state.running.unwrap_or(false) {
                bail!("контейнер {container_id} остановился до прохождения healthcheck");
            }
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("таймаут {timeout_secs}s ожидания healthcheck для {container_id}");
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

// ---------- платформенно-зависимые куски ----------
#[cfg(unix)]
fn get_host_ids() -> (u32, u32) {
    unsafe {
        (libc::getuid(), libc::getgid())
    }
}

#[cfg(windows)]
fn get_host_ids() -> (u32, u32) {
    // На Windows/Docker Desktop нет родного понятия UID/GID —
    // юзер внутри Linux VM Docker Desktop всегда мапится в 1000:1000
    // при обычном bind-mount, так что это безопасный дефолт
    (1000, 1000)
}

#[cfg(unix)]
fn docker_sock_bind() -> String {
    "/var/run/docker.sock:/var/run/docker.sock".to_string()
}

#[cfg(windows)]
fn docker_sock_bind() -> String {
    // TODO(windows): для Docker Desktop это именованный пайп, а не unix-сокет.
    // Формат монтирования отличается и обычно требует --isolation=process
    // либо Linux-контейнеров через WSL2-backend, где /var/run/docker.sock
    // может быть доступен как обычно. Пока — заглушка под WSL2-режим.
    "//./pipe/docker_engine://./pipe/docker_engine".to_string()
}

#[cfg(unix)]
fn docker_group_add() -> Result<Option<Vec<String>>> {
    Ok(Some(vec![get_docker_gid()?.to_string()]))
}

#[cfg(windows)]
fn docker_group_add() -> Result<Option<Vec<String>>> {
    // На Windows нет концепции unix-групп для сокета докера — не требуется.
    Ok(None)
}

#[cfg(unix)]
fn unix_only_cap_add() -> Option<Vec<String>> {
    Some(vec!["NET_ADMIN".to_string()])
}

#[cfg(windows)]
fn unix_only_cap_add() -> Option<Vec<String>> {
    // TODO(windows): NET_ADMIN — linux capability, у Windows-контейнеров нет прямого аналога.
    None
}

#[cfg(unix)]
fn unix_only_devices() -> Option<Vec<DeviceMapping>> {
    Some(vec![DeviceMapping {
        path_on_host: Some("/dev/net/tun".to_string()),
        path_in_container: Some("/dev/net/tun".to_string()),
        cgroup_permissions: Some("rwm".to_string()),
    }])
}

#[cfg(windows)]
fn unix_only_devices() -> Option<Vec<DeviceMapping>> {
    // TODO(windows): /dev/net/tun не существует в Windows-контейнерах.
    // Практический путь для Windows-хостов — гонять tailscaled как Windows-сервис
    // на хосте, а не как sidecar-контейнер, и подключать раннер к его localhost API.
    None
}

#[cfg(unix)]
fn get_docker_gid() -> Result<u32> {
    let metadata = std::fs::metadata("/var/run/docker.sock")
        .context("Не удалось получить метаданные /var/run/docker.sock. Убедитесь, что сокет смонтирован")?;
    let gid = metadata.gid();
    if gid == 0 {
        bail!("GID сокета равен 0 (root). Возможно, сокет не принадлежит группе docker.");
    }
    Ok(gid)
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