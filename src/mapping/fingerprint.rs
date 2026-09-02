use crate::mapping::avro::{AvroFingerprint, AvroFingerprintError};
use crate::mapping::json::{JsonCanonicalOpts, JsonFingerprint, JsonFingerprintError};
use crate::mapping::resolve::ResolveSchemaReferences;
use crate::registry::{SchemaType, Subject};

/// A schema fingerprint. Fingerprints of different schema types never compare equal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Fingerprint {
    Avro(AvroFingerprint),
    Protobuf,
    Json(JsonFingerprint),
}

#[derive(Debug, thiserror::Error)]
pub enum FingerprintError {
    #[error(transparent)]
    Avro(#[from] AvroFingerprintError),

    #[error(transparent)]
    Json(#[from] JsonFingerprintError),
}

/// Options for fingerprinting, grouped per schema type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FingerprintOpts {
    pub json: JsonCanonicalOpts,
}

impl Fingerprint {
    /// Fingerprint a subject, resolving its registry references through `resolver`.
    ///
    /// Protobuf subjects are not fingerprinted and yield [`Fingerprint::Protobuf`].
    pub fn for_subject(
        subject: &Subject,
        resolver: &impl ResolveSchemaReferences,
        opts: &FingerprintOpts,
    ) -> Result<Fingerprint, FingerprintError> {
        match subject.schema_type {
            SchemaType::Avro => Ok(Fingerprint::Avro(AvroFingerprint::from_subject(
                subject, resolver,
            )?)),
            SchemaType::Json => Ok(Fingerprint::Json(JsonFingerprint::from_subject(
                subject, resolver, &opts.json,
            )?)),
            SchemaType::Protobuf => Ok(Fingerprint::Protobuf),
        }
    }

    pub fn get_value_opt(&self) -> Option<String> {
        match self {
            Fingerprint::Avro(fingerprint) => Some(fingerprint.to_string()),
            Fingerprint::Json(fingerprint) => Some(fingerprint.to_string()),
            Fingerprint::Protobuf => None,
        }
    }
}
