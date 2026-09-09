use common::TestEnv;
use same::context::{Authentication, DownloadAllSchemaFilesOpts, EmptyDownloadProbe};
use same::mapping::{MapSchemasOpts, map_schemas};
use std::sync::Arc;

mod common;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_references() -> anyhow::Result<()> {
    common::setup_logs();

    let env = TestEnv::new_containerized_cluster().await?;
    let customer_subject = env
        .register_avro_schema("customer", include_str!("assets/avro/ref/customer.avsc"))
        .await?;
    let product_subject = env
        .register_avro_schema("product", include_str!("assets/avro/ref/product.avsc"))
        .await?;

    let _order_subject = env
        .register_avro_schema_with_references(
            "order",
            include_str!("assets/avro/ref/order.avsc"),
            vec![
                reference!("Customer", customer_subject),
                reference!("Product", product_subject),
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

    assert!(mapping.missed().is_empty(), "expected no missed schemas");
    assert_eq!(mapping.matched().len(), 3, "expected 3 matched schemas");

    Ok(())
}
