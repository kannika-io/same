use std::sync::Arc;

use same::context::{Authentication, DownloadAllSchemaFilesOpts, EmptyDownloadProbe};
use same::mapping::{MapSchemasOpts, map_schemas};

use crate::common::TestEnv;

mod common;

/// JSON Schemas that reference other subjects via `$ref` map to themselves.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_references_should_map_to_same_registry() -> anyhow::Result<()> {
    common::setup_logs();

    let env = TestEnv::new_containerized_cluster().await?;

    let customer = env
        .register_json_schema("customer", include_str!("assets/json/ref/customer.json"))
        .await?;
    let product = env
        .register_json_schema("product", include_str!("assets/json/ref/product.json"))
        .await?;
    let _order = env
        .register_json_schema_with_references(
            "order",
            include_str!("assets/json/ref/order.json"),
            vec![
                reference!("customer.json", customer),
                reference!("product.json", product),
            ],
        )
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
        3,
        "expected 3 matched JSON schemas"
    );

    Ok(())
}

/// The target registry holds the same order schema, but its `$ref` strings and reference names
/// differ (`customer.json` vs `io/kannika/Customer.schema.json`). References are matched on the
/// referenced *content*, not on the reference string, so the schemas still map.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn json_references_with_different_names_should_map_across_registries() -> anyhow::Result<()> {
    common::setup_logs();

    let source_env = TestEnv::new_containerized_cluster().await?;
    let target_env = TestEnv::new_containerized_cluster().await?;

    let customer = source_env
        .register_json_schema("customer", include_str!("assets/json/ref/customer.json"))
        .await?;
    let product = source_env
        .register_json_schema("product", include_str!("assets/json/ref/product.json"))
        .await?;
    let _ = source_env
        .register_json_schema_with_references(
            "order",
            include_str!("assets/json/ref/order.json"),
            vec![
                reference!("customer.json", customer),
                reference!("product.json", product),
            ],
        )
        .await?;

    let customer = target_env
        .register_json_schema(
            "io.kannika.customer",
            include_str!("assets/json/ref/customer.json"),
        )
        .await?;
    let product = target_env
        .register_json_schema(
            "io.kannika.product",
            include_str!("assets/json/ref/product.json"),
        )
        .await?;
    let _ = target_env
        .register_json_schema_with_references(
            "io.kannika.order",
            include_str!("assets/json/ref/order-alt-ref-names.json"),
            vec![
                reference!("io/kannika/Customer.schema.json", customer),
                reference!("io/kannika/Product.schema.json", product),
            ],
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
        3,
        "expected 3 matched JSON schemas"
    );

    Ok(())
}
