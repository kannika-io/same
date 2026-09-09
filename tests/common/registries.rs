use testcontainers::ContainerAsync;
use testcontainers::GenericImage;
use testcontainers::ImageExt;
use testcontainers::core::IntoContainerPort;
use testcontainers::core::WaitFor;
use testcontainers::runners::AsyncRunner;

#[allow(clippy::large_enum_variant)] // test helper, size is irrelevant
pub enum TestSchemaRegistry {
    Remote(RemoteSchemaRegistry),
    Containerized(ContainerizedSchemaRegistry),
}

pub struct RemoteSchemaRegistry {
    url: String,
}

pub struct ContainerizedSchemaRegistry {
    _container: ContainerAsync<GenericImage>,
    schema_registry_port: u16,
}

impl RemoteSchemaRegistry {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    pub fn get_schema_registry_url(&self) -> String {
        self.url.to_string()
    }
}

impl ContainerizedSchemaRegistry {
    const IMAGE: &'static str = "docker.redpanda.com/redpandadata/redpanda";
    const TAG: &'static str = "latest";

    pub async fn start() -> anyhow::Result<Self> {
        let container = GenericImage::new(Self::IMAGE, Self::TAG)
            .with_exposed_port(8081.tcp())
            .with_exposed_port(9092.tcp())
            .with_wait_for(WaitFor::message_on_stderr("Successfully started Redpanda"))
            .with_cmd(vec![
                "redpanda",
                "start",
                "--smp",
                "1",
                "--memory",
                "1G",
                "--mode",
                "dev-container",
            ])
            .start()
            .await?;

        let schema_registry_port = container.get_host_port_ipv4(8081).await?;

        Ok(Self {
            _container: container,
            schema_registry_port,
        })
    }

    pub fn get_schema_registry_url(&self) -> String {
        format!("http://localhost:{}", self.schema_registry_port)
    }
}
