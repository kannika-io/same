use std::sync::Arc;

use same::context::{Authentication, DownloadAllSchemaFilesOpts, EmptyDownloadProbe};
use same::mapping::{MapSchemasOpts, map_schemas};

use crate::common::TestEnv;

mod common;

/// A registry with Avro, JSON Schema and Protobuf subjects: Avro and JSON are mapped,
/// Protobuf is ignored.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mixed_schema_types_should_map_avro_and_json() -> anyhow::Result<()> {
    common::setup_logs();

    let env = TestEnv::new_containerized_cluster().await?;

    let _ = env
        .register_avro_schema("user", include_str!("assets/avro/user/v1.avsc"))
        .await?;
    let _ = env
        .register_json_schema("event-value", include_str!("assets/json/event/v1.json"))
        .await?;
    let _ = env
        .register_protobuf_schema("image", include_str!("assets/proto/image/v1.proto"))
        .await?;

    let from = env.mk_context("from", Authentication::None)?;
    let to = env.mk_context("to", Authentication::None)?;

    from.download_all_schema_files(DownloadAllSchemaFilesOpts::<EmptyDownloadProbe>::default())
        .await?;
    to.download_all_schema_files(DownloadAllSchemaFilesOpts::<EmptyDownloadProbe>::default())
        .await?;

    let mapping = map_schemas(Arc::new(from), Arc::new(to), MapSchemasOpts::default()).await?;

    assert!(
        mapping.missed().is_empty(),
        "expected no missed schemas, got: {:?}",
        mapping.missed()
    );
    assert_eq!(
        mapping.matched().len(),
        2,
        "expected the Avro and JSON schemas to be matched, protobuf ignored"
    );

    Ok(())
}
