use std::sync::Arc;

use same::context::{Authentication, DownloadAllSchemaFilesOpts, EmptyDownloadProbe};
use same::mapping::{MapSchemasOpts, map_schemas};

use crate::common::TestEnv;

mod common;

/// Schemas that are structurally identical but differ only in custom root-level properties
/// (e.g. dataOwnerEmail, dataOwner, sourceApplication) should produce the same Rabin fingerprint.
/// Custom properties are not part of the Avro Parsing Canonical Form and must be ignored.
///
/// This reproduces the real-world scenario where two separate registries hold the same schema
/// but one has additional custom metadata properties at the root level.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn schemas_with_custom_properties_should_map() -> anyhow::Result<()> {
    common::setup_logs();

    // Two separate registries — source has the schema without custom props,
    // target has the same schema with custom props added.
    let source_env = TestEnv::new_containerized_cluster().await?;
    let target_env = TestEnv::new_containerized_cluster().await?;

    // Register schema without custom properties in source registry
    let _ = source_env
        .register_avro_schema(
            "sensor-reading-value",
            include_str!("assets/avro/custom-properties/without-custom-props.avsc"),
        )
        .await?;

    // Register schema with custom properties in target registry
    let _ = target_env
        .register_avro_schema(
            "sensor-reading-value",
            include_str!("assets/avro/custom-properties/with-custom-props.avsc"),
        )
        .await?;

    let from = source_env.mk_context("from", Authentication::None)?;
    let to = target_env.mk_context("to", Authentication::None)?;

    from.download_all_schema_files(DownloadAllSchemaFilesOpts::<EmptyDownloadProbe>::default())
        .await?;
    to.download_all_schema_files(DownloadAllSchemaFilesOpts::<EmptyDownloadProbe>::default())
        .await?;

    let mapping = map_schemas(Arc::new(from), Arc::new(to), MapSchemasOpts::default()).await?;

    assert!(
        mapping.missed().is_empty(),
        "Expected no missed schemas, but got: {:?}",
        mapping.missed()
    );
    assert!(
        !mapping.matched().is_empty(),
        "Expected at least one matched schema"
    );

    Ok(())
}
