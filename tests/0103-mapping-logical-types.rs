use std::sync::Arc;

use same::context::{Authentication, DownloadAllSchemaFilesOpts, EmptyDownloadProbe};
use same::mapping::{MapSchemasOpts, map_schemas};

use crate::common::TestEnv;

mod common;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn logical_type_uuid() -> anyhow::Result<()> {
    common::setup_logs();

    let env = TestEnv::new_containerized_cluster().await?;
    env.register_avro_schema(
        "uuid",
        include_str!("assets/avro/logicaltypes/uuid/v1.avsc"),
    )
    .await?;
    env.register_avro_schema(
        "uuid",
        include_str!("assets/avro/logicaltypes/uuid/v2.avsc"),
    )
    .await?;
    env.register_avro_schema(
        "hvac",
        include_str!("assets/avro/logicaltypes/uuid/hvac.avsc"),
    )
    .await?;

    let from = env.mk_context("from", Authentication::None)?;
    let to = env.mk_context("to", Authentication::None)?;

    from.download_all_schema_files(DownloadAllSchemaFilesOpts::<EmptyDownloadProbe>::default())
        .await?;
    to.download_all_schema_files(DownloadAllSchemaFilesOpts::<EmptyDownloadProbe>::default())
        .await?;

    let mapping = map_schemas(Arc::new(from), Arc::new(to), MapSchemasOpts::default()).await?;

    assert!(mapping.missed().is_empty(), "expected no missed schemas");
    assert_eq!(mapping.matched().len(), 3, "expected 3 matched schemas");

    Ok(())
}
