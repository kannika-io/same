use crate::common::registries::{RemoteSchemaRegistry, TestSchemaRegistry};
use registries::ContainerizedSchemaRegistry;
use same::context::{Authentication, Context, SchemaRegistryConfig};
use same::registry::{
    DeleteVersionOptions, ListSubjectsOptions, NewVersionOptions, RegisterSchema, SchemaId,
    SchemaReference, SchemaRegistryClient, SchemaRegistryClientError, SchemaType, Subject,
    SubjectName,
};
#[allow(unused)]
use std::sync::Once;

pub mod macros;
pub mod registries;

#[allow(unused)]
pub struct TestEnv {
    pub registry: TestSchemaRegistry,
    pub client: SchemaRegistryClient,
    cache_dir: tempfile::TempDir,
}

#[allow(unused)]
impl TestEnv {
    pub async fn new_containerized_cluster() -> anyhow::Result<Self> {
        let container = ContainerizedSchemaRegistry::start().await?;
        let client = SchemaRegistryClient::new(&container.get_schema_registry_url())?;
        let registry = TestSchemaRegistry::Containerized(container);
        let cache_dir = tempfile::tempdir()?;
        Ok(Self {
            registry,
            client,
            cache_dir,
        })
    }

    pub fn new_remote(url: &str) -> anyhow::Result<Self> {
        let remote = RemoteSchemaRegistry::new(url);
        let registry = TestSchemaRegistry::Remote(remote);
        let client = SchemaRegistryClient::new(url)?;
        let cache_dir = tempfile::tempdir()?;
        Ok(Self {
            registry,
            client,
            cache_dir,
        })
    }

    pub async fn delete_all_subjects(&self) -> anyhow::Result<()> {
        if let TestSchemaRegistry::Remote(remote) = &self.registry
            && remote.get_schema_registry_url().contains("confluent.cloud")
        {
            return Err(anyhow::anyhow!(
                "Cannot delete all subjects in Confluent Schema Registry for safety reasons"
            ));
        }

        let subjects = self
            .client
            .subject()
            .list(ListSubjectsOptions::default())
            .await?;
        for subject in subjects {
            for version in self.client.subject().versions(&subject).await? {
                let _ = self
                    .client
                    .subject()
                    .delete_version(&subject, version, DeleteVersionOptions::default())
                    .await?;
            }
        }
        Ok(())
    }

    pub fn get_client(&self) -> &SchemaRegistryClient {
        &self.client
    }

    pub async fn register_protobuf_schema(
        &self,
        subject_name: &str,
        schema: &str,
    ) -> anyhow::Result<Subject> {
        let request = RegisterSchema {
            schema: schema.to_string(),
            schema_type: Some(SchemaType::Protobuf),
            ..Default::default()
        };
        self.register_schema(subject_name, request).await
    }

    pub async fn register_avro_schema(
        &self,
        subject_name: &str,
        schema: &str,
    ) -> anyhow::Result<Subject> {
        let request = RegisterSchema {
            schema: schema.to_string(),
            schema_type: Some(SchemaType::Avro),
            ..Default::default()
        };
        self.register_schema(subject_name, request).await
    }

    pub async fn register_avro_schema_with_references(
        &self,
        subject_name: &str,
        schema: &str,
        references: Vec<SchemaReference>,
    ) -> anyhow::Result<Subject> {
        let request = RegisterSchema {
            schema: schema.to_string(),
            schema_type: Some(SchemaType::Avro),
            references,
        };
        self.register_schema(subject_name, request).await
    }

    pub async fn register_json_schema(
        &self,
        subject_name: &str,
        schema: &str,
    ) -> anyhow::Result<Subject> {
        let request = RegisterSchema {
            schema: schema.to_string(),
            schema_type: Some(SchemaType::Json),
            ..Default::default()
        };
        self.register_schema(subject_name, request).await
    }

    pub async fn register_json_schema_with_references(
        &self,
        subject_name: &str,
        schema: &str,
        references: Vec<SchemaReference>,
    ) -> anyhow::Result<Subject> {
        let request = RegisterSchema {
            schema: schema.to_string(),
            schema_type: Some(SchemaType::Json),
            references,
        };
        self.register_schema(subject_name, request).await
    }

    async fn register_schema(
        &self,
        subject_name: &str,
        register_schema_request: RegisterSchema,
    ) -> anyhow::Result<Subject> {
        let subject_name: SubjectName = subject_name.parse()?;

        let registered_schema = self
            .client
            .subject()
            .new_version(
                &subject_name,
                &register_schema_request,
                NewVersionOptions::default(),
            )
            .await?;

        match self
            .find_version_of_schema_id(&subject_name, &registered_schema.id)
            .await?
        {
            Some(subject) => Ok(subject),
            None => Err(anyhow::anyhow!(
                "Failed to find version for subject {} with id {} after registering it",
                subject_name,
                registered_schema.id
            )),
        }
    }

    async fn find_version_of_schema_id(
        &self,
        subject: &SubjectName,
        schema_id: &SchemaId,
    ) -> Result<Option<Subject>, SchemaRegistryClientError> {
        let subject_versions = self.client.subject().versions(subject).await?;

        for subject_version in subject_versions {
            let schema = self
                .client
                .subject()
                .version(subject, subject_version)
                .await?;

            if let Some(schema) = schema
                && schema.id == *schema_id
            {
                return Ok(Some(schema));
            }
        }

        Ok(None)
    }

    pub fn mk_context(&self, name: &str, auth: Authentication) -> anyhow::Result<Context> {
        Ok(Context::new(
            name.parse()?,
            SchemaRegistryConfig {
                url: self.get_schema_registry_url(),
                auth,
            },
        )
        .with_cache_dir(self.cache_dir.path().to_path_buf()))
    }

    fn get_schema_registry_url(&self) -> String {
        match &self.registry {
            TestSchemaRegistry::Containerized(container) => container.get_schema_registry_url(),
            TestSchemaRegistry::Remote(remote) => remote.get_schema_registry_url().to_string(),
        }
    }
}

pub fn setup_logs() {
    use tracing_subscriber::prelude::*;

    static INIT: Once = Once::new();

    INIT.call_once(|| {
        // Enable logs
        let format = tracing_subscriber::fmt::layer()
            .with_thread_names(true)
            .with_line_number(true);
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .or_else(|_| {
                tracing_subscriber::EnvFilter::try_new("debug,hyper=warn,bollard::docker=warn")
            })
            .unwrap();
        tracing_subscriber::registry()
            .with(filter)
            .with(format)
            .init();
    })
}
