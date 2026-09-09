use crate::common::TestEnv;
use same::context::{Authentication, DownloadAllSchemaFilesOpts, EmptyDownloadProbe};
use same::mapping::{MapSchemasOpts, map_schemas};
use std::sync::Arc;

mod common;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_map_schemas_ignores_protobuf() -> anyhow::Result<()> {
    common::setup_logs();

    let env = TestEnv::new_containerized_cluster().await?;

    let _ = env
        .register_protobuf_schema("image", include_str!("assets/proto/image/v1.proto"))
        .await?;
    let _ = env
        .register_protobuf_schema("image", include_str!("assets/proto/image/v2.proto"))
        .await?;

    let from = env.mk_context("from", Authentication::None)?;
    let to = env.mk_context("to", Authentication::None)?;

    from.download_all_schema_files(DownloadAllSchemaFilesOpts::<EmptyDownloadProbe>::default())
        .await?;
    to.download_all_schema_files(DownloadAllSchemaFilesOpts::<EmptyDownloadProbe>::default())
        .await?;

    let mapping = map_schemas(Arc::new(from), Arc::new(to), MapSchemasOpts::default()).await?;

    assert!(
        mapping.matched().is_empty(),
        "protobuf schemas should not be matched"
    );
    assert!(
        mapping.missed().is_empty(),
        "protobuf schemas are not indexed"
    );

    Ok(())
}
