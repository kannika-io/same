use std::sync::Arc;

use same::context::{Authentication, DownloadAllSchemaFilesOpts, EmptyDownloadProbe};
use same::mapping::{MapSchemasOpts, map_schemas};

use crate::common::TestEnv;

mod common;

/// Two registries hold the same JSON Schema, but with a different `title`
/// (`io.kannika.examples.OrderPlaced` vs `com.acme.orders.OrderPlaced`) and different
/// `description`/`examples`/`default` annotations. Annotations do not affect structure and must
/// not prevent the schemas from being mapped.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_schemas_differing_only_in_annotations_should_map() -> anyhow::Result<()> {
    common::setup_logs();

    let source_env = TestEnv::new_containerized_cluster().await?;
    let target_env = TestEnv::new_containerized_cluster().await?;

    let _ = source_env
        .register_json_schema(
            "order-placed-value",
            include_str!("assets/json/annotations/source.json"),
        )
        .await?;
    let _ = target_env
        .register_json_schema(
            "order-placed-value",
            include_str!("assets/json/annotations/target.json"),
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
        "expected no missed schemas, got: {:?}",
        mapping.missed()
    );
    assert_eq!(
        mapping.matched().len(),
        1,
        "expected the OrderPlaced schema to be mapped"
    );

    Ok(())
}
