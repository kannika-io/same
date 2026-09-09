use std::sync::Arc;

use same::context::{Authentication, DownloadAllSchemaFilesOpts, EmptyDownloadProbe};
use same::mapping::{MapSchemasOpts, map_schemas};

use crate::common::TestEnv;

mod common;

/// Structural differences must never be mapped:
/// - `eventId` is a string in the source and an integer in the target
/// - a *property* named `title` (not the annotation) differs in type
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn structurally_different_json_schemas_should_not_map() -> anyhow::Result<()> {
    common::setup_logs();

    let source_env = TestEnv::new_containerized_cluster().await?;
    let target_env = TestEnv::new_containerized_cluster().await?;

    let _ = source_env
        .register_json_schema(
            "event-value",
            include_str!("assets/json/mismatch/event-string-id.json"),
        )
        .await?;
    let _ = source_env
        .register_json_schema(
            "document-value",
            include_str!("assets/json/mismatch/title-property-string.json"),
        )
        .await?;

    let _ = target_env
        .register_json_schema(
            "event-value",
            include_str!("assets/json/mismatch/event-integer-id.json"),
        )
        .await?;
    let _ = target_env
        .register_json_schema(
            "document-value",
            include_str!("assets/json/mismatch/title-property-integer.json"),
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
        mapping.matched().is_empty(),
        "expected no matched schemas, got: {:?}",
        mapping.matched()
    );
    assert_eq!(
        mapping.missed().len(),
        2,
        "expected both source schemas to be missed"
    );

    Ok(())
}
