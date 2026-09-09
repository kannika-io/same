use multimap::MultiMap;

use crate::mapping::fingerprint::{Fingerprint, FingerprintOpts};
use crate::mapping::resolve::ResolveSchemaReferences;
use crate::registry::{SchemaId, SchemaReference, SchemaType, SchemaVersion, Subject, SubjectName};

/// Schema registry index that allows for fast lookup of schema references by fingerprint
pub struct SchemaRegistryIndex {
    // Index by fingerprint
    fp: MultiMap<Fingerprint, FingerprintedSchema>,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SchemaRegistryIndexError {
    #[error("Failed to calculate fingerprint for subject {0} with schema version: {1}: {2}")]
    FailedToCalculateFingerprint(SubjectName, SchemaVersion, String),
    #[error("Failed to index schema: {0}")]
    IndexingError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FingerprintedSchema {
    pub subject: SubjectName,
    pub version: SchemaVersion,
    pub id: SchemaId,
    pub schema_type: SchemaType,
    pub fingerprint: Fingerprint,
    pub schema: String,
    pub references: Vec<SchemaReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Candidates {
    Multiple(Vec<FingerprintedSchema>),
    PerfectMatch(FingerprintedSchema),
    None,
}

impl SchemaRegistryIndex {
    pub fn new() -> Self {
        Self {
            fp: MultiMap::new(),
        }
    }

    pub fn index(
        &mut self,
        schema_subject: &Subject,
        resolver: &impl ResolveSchemaReferences,
        opts: &FingerprintOpts,
    ) -> Result<(), SchemaRegistryIndexError> {
        match schema_subject.schema_type {
            SchemaType::Avro | SchemaType::Json => {
                let schema =
                    FingerprintedSchema::from_subject(schema_subject.clone(), resolver, opts)?;
                self.insert(schema);
                Ok(())
            }
            SchemaType::Protobuf => Ok(()),
        }
    }

    fn insert(&mut self, reference: FingerprintedSchema) {
        self.fp
            .insert(reference.fingerprint.clone(), reference.clone());
    }

    pub fn find_by_fingerprint(&self, fingerprint: &Fingerprint) -> Candidates {
        self.fp
            .get_vec(fingerprint)
            .map(|schemas| schemas.to_owned())
            .map(|schemas| match schemas {
                mut schemas if schemas.len() == 1 => {
                    Candidates::PerfectMatch(schemas.pop().unwrap())
                }
                schema_refs => Candidates::Multiple(schema_refs),
            })
            .unwrap_or(Candidates::None)
    }
}

impl FingerprintedSchema {
    pub fn from_subject(
        subject: Subject,
        resolver: &impl ResolveSchemaReferences,
        opts: &FingerprintOpts,
    ) -> Result<Self, SchemaRegistryIndexError> {
        let fingerprint = Fingerprint::for_subject(&subject, resolver, opts).map_err(|err| {
            SchemaRegistryIndexError::FailedToCalculateFingerprint(
                subject.subject.clone(),
                subject.version.clone(),
                err.to_string(),
            )
        })?;

        Ok(FingerprintedSchema {
            subject: subject.subject.clone(),
            version: subject.version.clone(),
            id: subject.id.clone(),
            schema_type: subject.schema_type.clone(),
            fingerprint,
            schema: subject.schema.clone(),
            references: subject.references.clone(),
        })
    }
}

impl<'a> IntoIterator for &'a SchemaRegistryIndex {
    type Item = &'a FingerprintedSchema;
    type IntoIter = std::vec::IntoIter<&'a FingerprintedSchema>;

    fn into_iter(self) -> Self::IntoIter {
        self.fp
            .flat_iter()
            .map(|(_, schema)| schema)
            .collect::<Vec<_>>()
            .into_iter()
    }
}

#[cfg(test)]
mod tests {
    use crate::mapping::fingerprint::{Fingerprint, FingerprintOpts};
    use crate::mapping::index::{Candidates, FingerprintedSchema, SchemaRegistryIndex};
    use crate::mapping::resolve::{Resolution, ResolutionError, ResolveSchemaReferences};
    use crate::registry::{SchemaId, SchemaReference, SchemaType, SchemaVersion, Subject};

    struct MockResolver {
        mapping: Vec<(SchemaReference, Subject)>,
    }

    impl ResolveSchemaReferences for MockResolver {
        fn resolve_schema_reference(
            &self,
            reference: &SchemaReference,
        ) -> Result<Resolution, ResolutionError> {
            for (schema_ref, subject) in &self.mapping {
                if schema_ref == reference {
                    return Ok(Resolution::Resolved(schema_ref.clone(), subject.clone()));
                }
            }
            Ok(Resolution::Unresolved(reference.clone()))
        }
    }

    impl MockResolver {
        fn new() -> Self {
            Self {
                mapping: Vec::new(),
            }
        }
    }

    #[test]
    fn find_avro_schema_by_fingerprint() {
        let mut index = SchemaRegistryIndex::new();
        let schema_subject = avrocado_subject();
        let fingerprint = Fingerprint::for_subject(
            &schema_subject,
            &MockResolver::new(),
            &FingerprintOpts::default(),
        )
        .unwrap();

        index
            .index(
                &schema_subject,
                &MockResolver::new(),
                &FingerprintOpts::default(),
            )
            .unwrap();
        let schema = FingerprintedSchema::from_subject(
            schema_subject,
            &MockResolver::new(),
            &FingerprintOpts::default(),
        )
        .unwrap();
        let expected: Candidates = Candidates::PerfectMatch(schema);

        assert_eq!(index.find_by_fingerprint(&fingerprint), expected);
    }

    #[test]
    fn find_avro_schema_with_references_by_fingerprint() {
        // Set up references
        let mut resolver = MockResolver::new();
        resolver.mapping.push((
            SchemaReference {
                name: "Product".parse().unwrap(),
                subject: "product".to_string(),
                version: "5".parse::<SchemaVersion>().unwrap(),
            },
            product_subject(),
        ));
        resolver.mapping.push((
            SchemaReference {
                name: "Customer".parse().unwrap(),
                subject: "customer".to_string(),
                version: "6".parse::<SchemaVersion>().unwrap(),
            },
            customer_subject(),
        ));

        let mut index = SchemaRegistryIndex::new();

        let schema_subject = order_subject();

        let fingerprint =
            Fingerprint::for_subject(&schema_subject, &resolver, &FingerprintOpts::default())
                .unwrap();

        index
            .index(&schema_subject, &resolver, &FingerprintOpts::default())
            .unwrap();

        let schema = FingerprintedSchema::from_subject(
            schema_subject,
            &resolver,
            &FingerprintOpts::default(),
        )
        .unwrap();
        let expected: Candidates = Candidates::PerfectMatch(schema);

        assert_eq!(index.find_by_fingerprint(&fingerprint), expected);
    }

    #[test]
    fn index_protobuf_schema_is_ignored() {
        let mut index = SchemaRegistryIndex::new();
        let schema_subject = potatobuf_subject();
        let fingerprint = Fingerprint::for_subject(
            &schema_subject,
            &MockResolver::new(),
            &FingerprintOpts::default(),
        )
        .unwrap();
        index
            .index(
                &schema_subject,
                &MockResolver::new(),
                &FingerprintOpts::default(),
            )
            .unwrap();

        let results = index.find_by_fingerprint(&fingerprint);

        assert_eq!(results, Candidates::None);
    }

    #[test]
    fn find_json_schema_by_fingerprint() {
        let mut index = SchemaRegistryIndex::new();
        let schema_subject = jacksonfruit_subject();
        let fingerprint = Fingerprint::for_subject(
            &schema_subject,
            &MockResolver::new(),
            &FingerprintOpts::default(),
        )
        .unwrap();

        index
            .index(
                &schema_subject,
                &MockResolver::new(),
                &FingerprintOpts::default(),
            )
            .unwrap();
        let schema = FingerprintedSchema::from_subject(
            schema_subject,
            &MockResolver::new(),
            &FingerprintOpts::default(),
        )
        .unwrap();
        let expected: Candidates = Candidates::PerfectMatch(schema);

        assert_eq!(index.find_by_fingerprint(&fingerprint), expected);
    }

    #[test]
    fn json_schemas_differing_only_in_annotations_share_fingerprint() {
        let mut index = SchemaRegistryIndex::new();
        let resolver = MockResolver::new();

        let source = json_subject("order-placed-value", "1", "101", ORDER_PLACED_A);
        let target = json_subject("order-placed-value", "2", "102", ORDER_PLACED_B);
        index
            .index(&source, &resolver, &FingerprintOpts::default())
            .unwrap();
        index
            .index(&target, &resolver, &FingerprintOpts::default())
            .unwrap();

        let fingerprint =
            Fingerprint::for_subject(&source, &MockResolver::new(), &FingerprintOpts::default())
                .unwrap();

        match index.find_by_fingerprint(&fingerprint) {
            Candidates::Multiple(schemas) => {
                let ids: std::collections::HashSet<SchemaId> =
                    schemas.iter().map(|s| s.id).collect();
                assert_eq!(
                    ids.len(),
                    2,
                    "both title variants should share the fingerprint"
                );
            }
            other => panic!("expected both JSON schemas under one fingerprint, got {other:?}"),
        }
    }

    #[test]
    fn json_schemas_with_different_structure_do_not_share_fingerprint() {
        let mut index = SchemaRegistryIndex::new();
        let resolver = MockResolver::new();

        let target = json_subject(
            "order-placed-value",
            "1",
            "101",
            &ORDER_PLACED_A.replace(
                r#""eventId": { "type": "string" }"#,
                r#""eventId": { "type": "integer" }"#,
            ),
        );
        index
            .index(&target, &resolver, &FingerprintOpts::default())
            .unwrap();

        let fingerprint = Fingerprint::for_subject(
            &json_subject("x", "1", "1", ORDER_PLACED_A),
            &MockResolver::new(),
            &FingerprintOpts::default(),
        )
        .unwrap();

        assert_eq!(index.find_by_fingerprint(&fingerprint), Candidates::None);
    }

    #[test]
    fn find_json_schema_with_references_by_fingerprint() {
        let customer = json_subject("customer-json", "1", "201", JSON_CUSTOMER);
        let mut resolver = MockResolver::new();
        resolver.mapping.push((
            SchemaReference {
                name: "customer.json".to_string(),
                subject: "customer-json".to_string(),
                version: "1".parse::<SchemaVersion>().unwrap(),
            },
            customer,
        ));

        let mut order = json_subject("order-json", "1", "202", JSON_ORDER);
        order.references = vec![SchemaReference {
            name: "customer.json".to_string(),
            subject: "customer-json".to_string(),
            version: "1".parse::<SchemaVersion>().unwrap(),
        }];

        let mut index = SchemaRegistryIndex::new();
        index
            .index(&order, &resolver, &FingerprintOpts::default())
            .unwrap();

        let fingerprint =
            Fingerprint::for_subject(&order, &resolver, &FingerprintOpts::default()).unwrap();
        let expected = FingerprintedSchema::from_subject(
            order.clone(),
            &resolver,
            &FingerprintOpts::default(),
        )
        .unwrap();
        assert_eq!(
            index.find_by_fingerprint(&fingerprint),
            Candidates::PerfectMatch(expected)
        );

        // Without resolving the reference, the fingerprint is different: content matters.
        let unresolved =
            Fingerprint::for_subject(&order, &MockResolver::new(), &FingerprintOpts::default())
                .unwrap();
        assert_eq!(index.find_by_fingerprint(&unresolved), Candidates::None);
    }

    #[test]
    fn avro_and_json_schemas_never_cross_match() {
        // An Avro record definition is valid JSON. Registering the same bytes as Avro and as
        // JSON Schema must produce two unrelated fingerprints.
        let mut index = SchemaRegistryIndex::new();
        let resolver = MockResolver::new();

        let avro = avrocado_subject();
        let json = json_subject("avrocado-as-json", "1", "99", avocado_schema());
        index
            .index(&avro, &resolver, &FingerprintOpts::default())
            .unwrap();
        index
            .index(&json, &resolver, &FingerprintOpts::default())
            .unwrap();

        let avro_fp =
            Fingerprint::for_subject(&avro, &MockResolver::new(), &FingerprintOpts::default())
                .unwrap();
        let json_fp =
            Fingerprint::for_subject(&json, &MockResolver::new(), &FingerprintOpts::default())
                .unwrap();
        assert_ne!(avro_fp, json_fp);

        match index.find_by_fingerprint(&avro_fp) {
            Candidates::PerfectMatch(found) => assert_eq!(found.schema_type, SchemaType::Avro),
            other => panic!("expected the Avro subject only, got {other:?}"),
        }
        match index.find_by_fingerprint(&json_fp) {
            Candidates::PerfectMatch(found) => assert_eq!(found.schema_type, SchemaType::Json),
            other => panic!("expected the JSON subject only, got {other:?}"),
        }
    }

    /// CYM-1200: When multiple schema versions share the same fingerprint (e.g. structurally
    /// identical schemas with different IDs), iterating the index must yield ALL of them.
    #[test]
    fn iterate_should_yield_all_versions_with_same_fingerprint() {
        let mut index = SchemaRegistryIndex::new();
        let resolver = MockResolver::new();

        // Index 4 versions of the same schema — same structure, different IDs
        for (version, id) in [("1", "101"), ("2", "102"), ("3", "103"), ("4", "104")] {
            let subject = Subject {
                subject: "business-agreement-value".parse().unwrap(),
                version: version.parse::<SchemaVersion>().unwrap(),
                id: id.parse::<SchemaId>().unwrap(),
                schema_type: SchemaType::Avro,
                schema: avocado_schema().to_string(),
                references: vec![],
            };
            index
                .index(&subject, &resolver, &FingerprintOpts::default())
                .unwrap();
        }

        let iterated_ids: std::collections::HashSet<SchemaId> =
            index.into_iter().map(|s| s.id).collect();

        assert_eq!(
            iterated_ids.len(),
            4,
            "Expected all 4 versions to be iterated, but got: {:?}",
            iterated_ids
        );
    }

    fn avrocado_subject() -> Subject {
        Subject {
            subject: "avrocado".parse().unwrap(),
            version: "1".parse::<SchemaVersion>().unwrap(),
            id: "11".parse::<SchemaId>().unwrap(),
            schema_type: SchemaType::Avro,
            schema: avocado_schema().to_string(),
            references: vec![],
        }
    }

    fn potatobuf_subject() -> Subject {
        Subject {
            subject: "potatobuf".parse().unwrap(),
            version: "2".parse::<SchemaVersion>().unwrap(),
            id: "22".parse::<SchemaId>().unwrap(),
            schema_type: SchemaType::Protobuf,
            schema: potato_schema().to_string(),
            references: vec![],
        }
    }

    fn jacksonfruit_subject() -> Subject {
        json_subject("jacksonfruit", "3", "33", jackfruit_schema())
    }

    fn json_subject(subject: &str, version: &str, id: &str, schema: &str) -> Subject {
        Subject {
            subject: subject.parse().unwrap(),
            version: version.parse::<SchemaVersion>().unwrap(),
            id: id.parse::<SchemaId>().unwrap(),
            schema_type: SchemaType::Json,
            schema: schema.to_string(),
            references: vec![],
        }
    }

    const ORDER_PLACED_A: &str = r#"{
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "io.kannika.examples.OrderPlaced",
        "type": "object",
        "properties": {
            "eventType": { "type": "string" },
            "eventId": { "type": "string" }
        }
    }"#;

    const ORDER_PLACED_B: &str = r#"{
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "com.acme.orders.OrderPlaced",
        "type": "object",
        "properties": {
            "eventType": { "type": "string" },
            "eventId": { "type": "string" }
        }
    }"#;

    const JSON_CUSTOMER: &str = r#"{
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "Customer",
        "type": "object",
        "properties": { "name": { "type": "string" } },
        "required": ["name"]
    }"#;

    const JSON_ORDER: &str = r#"{
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "Order",
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "customer": { "$ref": "customer.json" }
        },
        "required": ["id", "customer"]
    }"#;

    fn order_subject() -> Subject {
        Subject {
            subject: "schema_reference".parse().unwrap(),
            version: "4".parse::<SchemaVersion>().unwrap(),
            id: "44".parse::<SchemaId>().unwrap(),
            schema_type: SchemaType::Avro,
            schema: order_schema().to_string(),
            references: vec![
                SchemaReference {
                    name: "Product".parse().unwrap(),
                    subject: "product".to_string(),
                    version: "5".parse::<SchemaVersion>().unwrap(),
                },
                SchemaReference {
                    name: "Customer".parse().unwrap(),
                    subject: "customer".to_string(),
                    version: "6".parse::<SchemaVersion>().unwrap(),
                },
            ],
        }
    }

    fn product_subject() -> Subject {
        Subject {
            subject: "product".parse().unwrap(),
            version: "5".parse::<SchemaVersion>().unwrap(),
            id: "55".parse::<SchemaId>().unwrap(),
            schema_type: SchemaType::Avro,
            schema: product_schema().to_string(),
            references: vec![],
        }
    }

    fn customer_subject() -> Subject {
        Subject {
            subject: "customer".parse().unwrap(),
            version: "6".parse::<SchemaVersion>().unwrap(),
            id: "66".parse::<SchemaId>().unwrap(),
            schema_type: SchemaType::Avro,
            schema: customer_schema().to_string(),
            references: vec![],
        }
    }

    fn order_schema() -> &'static str {
        r#"
            {
                "type": "record",
                "name": "Order",
                "namespace": "io.kannika",
                "fields": [
                    {
                        "name": "product",
                        "type": "io.kannika.Product"
                    },
                    {
                        "name": "customer",
                        "type": "io.kannika.Customer"
                    }
                ]
            }
            "#
    }

    fn product_schema() -> &'static str {
        r#"
            {
                "type": "record",
                "name": "Product",
                "namespace": "io.kannika",
                "fields": [
                    {
                        "name": "productName",
                        "type": "string"
                    }
                ]
            }
            "#
    }

    fn customer_schema() -> &'static str {
        r#"
            {
                "type": "record",
                "name": "Customer",
                "namespace": "io.kannika",
                "fields": [
                    {
                        "name": "customerName",
                        "type": "string"
                    }
                ]
            }
            "#
    }

    fn potato_schema() -> &'static str {
        r#"
            syntax = "proto3";
            package com.example;
            message Potato {
                string name = 1;
                string color = 2;
                int32 age = 3;
            }
        "#
    }

    fn avocado_schema() -> &'static str {
        r#"
            {
                "type": "record",
                "name": "avocado",
                "namespace": "com.example",
                "fields": [
                    {
                        "name": "name",
                        "type": "string"
                    },
                    {
                        "name": "color",
                        "type": "string"
                    },
                    {
                        "name": "age",
                        "type": "int"
                    }
                ]
            }
            "#
    }

    fn jackfruit_schema() -> &'static str {
        r#"
            {
                "$schema": "http://json-schema.org/draft-07/schema#",
                "title": "Jackfruit",
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "color": { "type": "string" },
                    "age": { "type": "integer" }
                },
                "required": ["name"]
            }
            "#
    }
}
