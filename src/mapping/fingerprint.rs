use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::ops::Deref;

use apache_avro::rabin::Rabin;
use apache_avro::Schema as AvroSchema;
use digest::Digest;

use crate::mapping::resolve::{Resolution, ResolveSchemaReferences};
use crate::registry::{SchemaType, Subject};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Fingerprint {
    Avro(AvroFingerprint),
    Protobuf,
    Json(JsonFingerprint),
}

impl Fingerprint {
    pub fn get_value_opt(&self) -> Option<String> {
        match self {
            Fingerprint::Avro(fingerprint) => Some(fingerprint.to_string()),
            Fingerprint::Json(fingerprint) => Some(fingerprint.to_string()),
            Fingerprint::Protobuf => None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FingerprintError {
    #[error(transparent)]
    InvalidAvroSchema(#[from] apache_avro::Error),

    #[error("Invalid JSON schema: {0}")]
    InvalidJsonSchema(#[from] serde_json::Error),
}

pub trait ToFingerprint {
    fn to_fingerprint(&self) -> Result<Fingerprint, FingerprintError>;
}

pub struct SubjectFingerPrintBuilder {
    pub subject: Subject,
    pub referenced_schemas: Vec<String>,
}

impl SubjectFingerPrintBuilder {
    pub fn new(subject: Subject) -> SubjectFingerPrintBuilder {
        SubjectFingerPrintBuilder {
            subject,
            referenced_schemas: Vec::new(),
        }
    }

    pub fn resolve_references_from(
        &mut self,
        resolver: &impl ResolveSchemaReferences,
    ) -> &SubjectFingerPrintBuilder {
        let mut resolved = Vec::new();

        for reference in self.subject.references.iter() {
            match resolver.resolve_schema_reference(reference) {
                Ok(Resolution::Resolved(_schema_ref, resolved_schema)) => {
                    resolved.push(resolved_schema.schema);
                }
                Ok(Resolution::Unresolved(schema_ref)) => {
                    tracing::warn!("Unresolved schema reference: {:?}", schema_ref)
                }
                Err(err) => {
                    tracing::error!("Error resolving schema reference: {:?}", err)
                }
            }
        }
        self.referenced_schemas = resolved;
        self
    }
}

impl ToFingerprint for SubjectFingerPrintBuilder {
    fn to_fingerprint(&self) -> Result<Fingerprint, FingerprintError> {
        match self.subject.schema_type {
            SchemaType::Avro => {
                let mut schemas = Vec::<&str>::new();

                // Add the subject schema, MUST be first of the list
                schemas.push(self.subject.schema.as_str());

                for schema in self.referenced_schemas.iter() {
                    schemas.push(schema.as_str());
                }

                let input = &schemas[..];

                let schema = AvroSchema::parse_list(input)
                    .map_err(|e| FingerprintError::InvalidAvroSchema(e))?;

                // Get the first schema in the list
                let first = schema.first().unwrap();

                let fingerprint = AvroFingerprint::from_schema(&first);

                Ok(Fingerprint::Avro(fingerprint))
            }
            SchemaType::Json => {
                let fingerprint = JsonFingerprint::from_schema_str(self.subject.schema.as_str())?;
                Ok(Fingerprint::Json(fingerprint))
            }
            SchemaType::Protobuf => Ok(Fingerprint::Protobuf),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AvroFingerprint {
    pub bytes: Vec<u8>,
}

impl AvroFingerprint {
    pub fn from_schema(schema: &AvroSchema) -> AvroFingerprint {
        let fingerprint = schema.fingerprint::<Rabin>();

        AvroFingerprint {
            bytes: fingerprint.bytes,
        }
    }
}

impl Display for AvroFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = self.deref();
        for byte in bytes {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl Deref for AvroFingerprint {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl Debug for AvroFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = self.deref();
        for byte in bytes {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct JsonFingerprint {
    pub bytes: Vec<u8>,
}

impl JsonFingerprint {
    /// Fingerprint a JSON Schema by reducing it to canonical JSON
    /// (lexicographic keys, integer-valued floats normalized, compact)
    /// and taking a Rabin fingerprint over the canonical bytes.
    pub fn from_schema_str(schema: &str) -> Result<JsonFingerprint, FingerprintError> {
        let value: serde_json::Value = serde_json::from_str(schema)?;
        let canonical = jsonschema::canonical::json::to_string(&value)
            .map_err(|e| serde_json::Error::io(std::io::Error::other(e.to_string())))?;

        let mut hasher = Rabin::default();
        hasher.update(canonical.as_bytes());
        let out = hasher.finalize();

        Ok(JsonFingerprint {
            bytes: out.to_vec(),
        })
    }
}

impl Display for JsonFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.deref() {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl Debug for JsonFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.deref() {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl Deref for JsonFingerprint {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use crate::mapping::fingerprint::AvroFingerprint;
    use crate::mapping::fingerprint::JsonFingerprint;
    use crate::AvroSchema;

    #[test]
    fn display_should_print_fingerprint() {
        let schema = r#"
        {
            "type": "record",
            "name": "test",
            "namespace": "com.example",
            "fields": [
                {
                    "name": "a",
                    "type": "long"
                }
            ]
        }
        "#;

        let schema = AvroSchema::parse_str(schema).unwrap();

        let fingerprint = AvroFingerprint::from_schema(&schema);

        assert_eq!(format!("{}", fingerprint), "6c286d2ee6d243cd");
    }

    #[test]
    fn same_schemas_should_have_same_fingerprint() {
        let one = AvroSchema::parse_str(
            r#"
        {
            "type": "record",
            "name": "test",
            "namespace": "com.example",
            "fields": [
                {
                    "name": "a",
                    "type": "long"
                }
            ]
        }
        "#,
        )
        .unwrap();

        let two = AvroSchema::parse_str(
            r#"
        {
            "namespace": "com.example",
            "type": "record",
            "name": "test",
            "fields": [
                {
                    "type": "long",
                    "name": "a"
                }
            ]
        }
        "#,
        )
        .unwrap();

        assert_eq!(
            AvroFingerprint::from_schema(&one),
            AvroFingerprint::from_schema(&two)
        );
    }

    #[test]
    fn different_schemas_should_have_different_fingerprint() {
        let one = AvroSchema::parse_str(
            r#"
        {
            "type": "record",
            "name": "test",
            "namespace": "com.example",
            "fields": [
                {
                    "name": "a",
                    "type": "long"
                }
            ]
        }
        "#,
        )
        .unwrap();

        let two = AvroSchema::parse_str(
            r#"
        {
            "namespace": "com.example",
            "type": "record",
            "name": "test",
            "fields": [
                {
                    "type": "string",
                    "name": "a"
                }
            ]
        }
        "#,
        )
        .unwrap();

        assert_ne!(
            AvroFingerprint::from_schema(&one),
            AvroFingerprint::from_schema(&two)
        );
    }

    #[test]
    fn custom_root_properties_should_be_ignored() {
        let without_custom = AvroSchema::parse_str(
            r#"
        {
            "type": "record",
            "name": "SensorReading",
            "namespace": "io.kannika.test",
            "fields": [
                { "name": "sensorId", "type": "string" },
                { "name": "value", "type": "double" }
            ]
        }
        "#,
        )
        .unwrap();

        let with_custom = AvroSchema::parse_str(
            r#"
        {
            "type": "record",
            "name": "SensorReading",
            "namespace": "io.kannika.test",
            "fields": [
                { "name": "sensorId", "type": "string" },
                { "name": "value", "type": "double" }
            ],
            "dataOwnerEmail": "sensors@example.com",
            "dataOwner": "IoT Platform",
            "sourceApplication": "SensorHub"
        }
        "#,
        )
        .unwrap();

        assert_eq!(
            AvroFingerprint::from_schema(&without_custom),
            AvroFingerprint::from_schema(&with_custom),
            "Custom root-level properties should not affect the Rabin fingerprint"
        );
    }

    #[test]
    fn docs_should_be_ignored() {
        let one = AvroSchema::parse_str(r#"
        {
            "type": "record",
            "docs": "Experience is that marvelous thing that enables you recognize a mistake when you make it again.",
            "name": "test",
            "namespace": "com.example",
            "fields": [
                {
                    "name": "a",
                    "type": "long"
                }
            ]
        }
        "#).unwrap();

        let two = AvroSchema::parse_str(
            r#"
        {
            "namespace": "com.example",
            "type": "record",
            "name": "test",
            "fields": [
                {
                    "type": "long",
                    "name": "a",
                    "docs": "Trying to establish voice contact ... please yell into keyboard."
                }
            ]
        }
        "#,
        )
        .unwrap();

        assert_eq!(
            AvroFingerprint::from_schema(&one),
            AvroFingerprint::from_schema(&two)
        );
    }

    #[test]
    fn json_display_should_print_fingerprint() {
        let schema = r#"{ "type": "object", "properties": { "a": { "type": "integer" } } }"#;
        let fingerprint = JsonFingerprint::from_schema_str(schema).unwrap();
        // 16 lowercase hex chars (8 bytes)
        assert_eq!(fingerprint.to_string().len(), 16);
        assert!(fingerprint.to_string().chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn same_json_schemas_should_have_same_fingerprint() {
        // Differ only in key order, whitespace, and 1.0 vs 1 — canonicalization erases all three.
        let one = r#"{ "type": "object", "properties": { "a": { "type": "number", "minimum": 1.0 } } }"#;
        let two = r#"{"properties":{"a":{"minimum":1,"type":"number"}},"type":"object"}"#;
        assert_eq!(
            JsonFingerprint::from_schema_str(one).unwrap(),
            JsonFingerprint::from_schema_str(two).unwrap()
        );
    }

    #[test]
    fn different_json_schemas_should_have_different_fingerprint() {
        let one = r#"{ "type": "object", "properties": { "a": { "type": "string" } } }"#;
        let two = r#"{ "type": "object", "properties": { "a": { "type": "integer" } } }"#;
        assert_ne!(
            JsonFingerprint::from_schema_str(one).unwrap(),
            JsonFingerprint::from_schema_str(two).unwrap()
        );
    }

    /// v1 non-goal guard: annotations are NOT stripped, so a description-only edit
    /// currently produces a different fingerprint (a safe false-miss). This test
    /// documents intended v1 behavior and guards against accidental change.
    #[test]
    fn json_description_edit_changes_fingerprint_in_v1() {
        let one = r#"{ "type": "object", "description": "one" }"#;
        let two = r#"{ "type": "object", "description": "two" }"#;
        assert_ne!(
            JsonFingerprint::from_schema_str(one).unwrap(),
            JsonFingerprint::from_schema_str(two).unwrap()
        );
    }

    #[test]
    fn invalid_json_schema_returns_error() {
        let result = JsonFingerprint::from_schema_str("{ not valid json");
        assert!(result.is_err());
    }
}
