use multimap::MultiMap;

use crate::mapping::fingerprint::{Fingerprint, SubjectFingerPrintBuilder, ToFingerprint};
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
    ) -> Result<(), SchemaRegistryIndexError> {
        match schema_subject.schema_type {
            SchemaType::Avro => self.index_avro(schema_subject, resolver),
            SchemaType::Protobuf => Ok(()),
            SchemaType::Json => self.index_json(schema_subject, resolver),
        }
    }

    fn index_avro(
        &mut self,
        schema_subject: &Subject,
        resolver: &impl ResolveSchemaReferences,
    ) -> Result<(), SchemaRegistryIndexError> {
        let schema = FingerprintedSchema::from_subject(schema_subject.clone(), resolver)?;

        self.insert(schema);

        Ok(())
    }

    fn index_json(
        &mut self,
        schema_subject: &Subject,
        resolver: &impl ResolveSchemaReferences,
    ) -> Result<(), SchemaRegistryIndexError> {
        let schema = FingerprintedSchema::from_subject(schema_subject.clone(), resolver)?;

        self.insert(schema);

        Ok(())
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
    ) -> Result<Self, SchemaRegistryIndexError> {
        let fingerprint = SubjectFingerPrintBuilder::new(subject.clone())
            .resolve_references_from(resolver)
            .to_fingerprint()
            .map_err(|err| {
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
    use crate::mapping::fingerprint::{SubjectFingerPrintBuilder, ToFingerprint};
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
        let fingerprint = SubjectFingerPrintBuilder::new(schema_subject.clone())
            .to_fingerprint()
            .unwrap();

        index.index(&schema_subject, &MockResolver::new()).unwrap();
        let schema =
            FingerprintedSchema::from_subject(schema_subject, &MockResolver::new()).unwrap();
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

        let fingerprint = SubjectFingerPrintBuilder::new(schema_subject.clone())
            .resolve_references_from(&resolver)
            .to_fingerprint()
            .unwrap();

        index.index(&schema_subject, &resolver).unwrap();

        let schema = FingerprintedSchema::from_subject(schema_subject, &resolver).unwrap();
        let expected: Candidates = Candidates::PerfectMatch(schema);

        assert_eq!(index.find_by_fingerprint(&fingerprint), expected);
    }

    #[test]
    fn index_protobuf_schema_is_ignored() {
        let mut index = SchemaRegistryIndex::new();
        let schema_subject = potatobuf_subject();
        let fingerprint = SubjectFingerPrintBuilder::new(schema_subject.clone())
            .to_fingerprint()
            .unwrap();
        index.index(&schema_subject, &MockResolver::new()).unwrap();

        let results = index.find_by_fingerprint(&fingerprint);

        assert_eq!(results, Candidates::None);
    }

    #[test]
    fn find_json_schema_by_fingerprint() {
        let mut index = SchemaRegistryIndex::new();
        let schema_subject = jacksonfruit_subject();
        let fingerprint = SubjectFingerPrintBuilder::new(schema_subject.clone())
            .to_fingerprint()
            .unwrap();

        index.index(&schema_subject, &MockResolver::new()).unwrap();
        let schema =
            FingerprintedSchema::from_subject(schema_subject, &MockResolver::new()).unwrap();
        let expected: Candidates = Candidates::PerfectMatch(schema);

        assert_eq!(index.find_by_fingerprint(&fingerprint), expected);
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
            index.index(&subject, &resolver).unwrap();
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
        Subject {
            subject: "jacksonfruit".parse().unwrap(),
            version: "3".parse::<SchemaVersion>().unwrap(),
            id: "33".parse::<SchemaId>().unwrap(),
            schema_type: SchemaType::Json,
            schema: jackfruit_schema().to_string(),
            references: vec![],
        }
    }

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
                "type": "record",
                "name": "jackfruit",
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
}
