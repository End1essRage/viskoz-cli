use anyhow::{Result};
use bollard::Docker;
use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::CreateContainerOptions;

use crate::grpc::proto::RegisterRunnerResponse;

use crate::cli::StartArgs;

pub async fn start(reg: &RegisterRunnerResponse, mesh_ip: &str, args: &StartArgs) -> Result<()> {
    let docker = Docker::connect_with_local_defaults()?;

    let env = vec![
        format!("CONNECTOR_CP_ADDRESS={}", args.cp_address),
        //format!("CONNECTOR_RUNNER_ID={}", reg.runner_id),
        format!("CONNECTOR_TOKEN={}", reg.runner_token),
        format!("MESH_IP={}", mesh_ip),
        format!("REGISTRY_ENDPOINT={}", reg.registry_endpoint),
        format!("REGISTRY_USERNAME={}", reg.registry_username),
        format!("REGISTRY_PASSWORD={}", reg.registry_password),
    ];

    let host_config = HostConfig {
        binds: Some(vec!["/var/run/docker.sock:/var/run/docker.sock".to_string()]),
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

    docker.create_container(
        None::<CreateContainerOptions>,
        config
    ).await?;

    Ok(())
}