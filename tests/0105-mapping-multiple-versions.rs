use std::sync::Arc;

use same::context::{Authentication, DownloadAllSchemaFilesOpts, EmptyDownloadProbe};
use same::mapping::conflict::ConflictResolutionStrategy;
use same::mapping::{MapSchemasOpts, map_schemas};

use crate::common::TestEnv;

mod common;

/// CYM-1200: When a subject has multiple schema versions that share the same Rabin fingerprint
/// (e.g. schemas that differ only in custom properties), all versions must appear in the mapping
/// output. Previously, MultiMap::iter() only yielded one value per fingerprint key, silently
/// dropping other versions.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn all_source_versions_with_same_fingerprint_should_be_mapped() -> anyhow::Result<()> {
    common::setup_logs();

    let source_env = TestEnv::new_containerized_cluster().await?;
    let target_env = TestEnv::new_containerized_cluster().await?;

    // Two schemas that differ only in custom root properties — same Avro canonical form,
    // same Rabin fingerprint, but the registry assigns different version numbers and IDs.
    let without_custom = r#"{
        "type": "record",
        "name": "User",
        "namespace": "io.kannika.test",
        "fields": [
            {"name": "name", "type": "string"},
            {"name": "age", "type": "int"}
        ]
    }"#;

    let with_custom = r#"{
        "type": "record",
        "name": "User",
        "namespace": "io.kannika.test",
        "fields": [
            {"name": "name", "type": "string"},
            {"name": "age", "type": "int"}
        ],
        "dataOwner": "IoT Platform",
        "sourceApplication": "SensorHub"
    }"#;

    // Source: register v1 (plain) then v2 (with custom props) — same fingerprint, different IDs
    let _ = source_env
        .register_avro_schema("user-value", without_custom)
        .await?;
    let _ = source_env
        .register_avro_schema("user-value", with_custom)
        .await?;

    // Target: register both too so each source version has a match
    let _ = target_env
        .register_avro_schema("user-value", without_custom)
        .await?;
    let _ = target_env
        .register_avro_schema("user-value", with_custom)
        .await?;

    let from = source_env.mk_context("from", Authentication::None)?;
    let to = target_env.mk_context("to", Authentication::None)?;

    from.download_all_schema_files(DownloadAllSchemaFilesOpts::<EmptyDownloadProbe>::default())
        .await?;
    to.download_all_schema_files(DownloadAllSchemaFilesOpts::<EmptyDownloadProbe>::default())
        .await?;

    let opts = MapSchemasOpts {
        on_conflict: ConflictResolutionStrategy::PickHighestId,
        ..Default::default()
    };
    let mapping = map_schemas(Arc::new(from), Arc::new(to), opts).await?;

    assert_eq!(
        mapping.matched().len(),
        2,
        "Expected both source versions to be mapped, but only {} were: {:?}",
        mapping.matched().len(),
        mapping.matched()
    );

    Ok(())
}
