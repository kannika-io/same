//! Fingerprinting of Avro subjects.
//!
//! Avro schemas are hashed with the Rabin fingerprint of their Parsing Canonical Form, which
//! already drops documentation, defaults, aliases and custom attributes.

use std::fmt::{Debug, Display};
use std::ops::Deref;

use apache_avro::Schema as AvroSchema;
use apache_avro::rabin::Rabin;

use crate::mapping::resolve::{Resolution, ResolveSchemaReferences};
use crate::registry::Subject;

#[derive(Debug, thiserror::Error)]
pub enum AvroFingerprintError {
    #[error(transparent)]
    InvalidSchema(#[from] apache_avro::Error),
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

    /// Fingerprint an Avro subject.
    ///
    /// Directly referenced schemas are resolved and parsed together with the subject schema, so
    /// that named types from other subjects resolve. Unresolved references are logged and skipped.
    pub fn from_subject(
        subject: &Subject,
        resolver: &impl ResolveSchemaReferences,
    ) -> Result<AvroFingerprint, AvroFingerprintError> {
        let mut referenced = Vec::new();

        for reference in subject.references.iter() {
            match resolver.resolve_schema_reference(reference) {
                Ok(Resolution::Resolved(_schema_ref, resolved)) => {
                    referenced.push(resolved.schema);
                }
                Ok(Resolution::Unresolved(schema_ref)) => {
                    tracing::warn!("Unresolved schema reference: {:?}", schema_ref)
                }
                Err(err) => {
                    tracing::error!("Error resolving schema reference: {:?}", err)
                }
            }
        }

        // The subject schema MUST be first: parse_list returns schemas in input order.
        let mut schemas = Vec::with_capacity(1 + referenced.len());
        schemas.push(subject.schema.as_str());
        schemas.extend(referenced.iter().map(String::as_str));

        let parsed = AvroSchema::parse_list(&schemas)?;
        let first = parsed
            .first()
            .expect("parse_list yields one schema per input");

        Ok(AvroFingerprint::from_schema(first))
    }
}

impl Display for AvroFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.deref() {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl Debug for AvroFingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

impl Deref for AvroFingerprint {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use crate::AvroSchema;
    use crate::mapping::avro::AvroFingerprint;

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
}
