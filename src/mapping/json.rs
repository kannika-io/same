//! Fingerprinting and canonicalization of JSON Schema subjects.
//!
//! The registry stores JSON schemas verbatim, so two structurally identical schemas can differ in
//! key order, whitespace, number formatting, annotations (`title`, `description`, ...) and in the
//! way they reference other subjects. This module reduces a schema to a canonical
//! [`serde_json::Value`] in which all of those differences are erased, so that a hash over its
//! canonical serialization identifies the schema *structure* rather than its textual form.
//!
//! The walk is keyword-aware: `title` is dropped when it is a schema keyword, but kept when it is
//! a property name under `properties` or a literal inside `enum`/`const`.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::{Debug, Display};
use std::ops::Deref;

use apache_avro::rabin::Rabin;
use digest::Digest;
use serde_json::{Map, Value};
use url::Url;

use crate::mapping::resolve::{Resolution, ResolveSchemaReferences};
use crate::registry::{SchemaVersion, Subject, SubjectName};

#[derive(Debug, thiserror::Error)]
pub enum JsonFingerprintError {
    #[error("Invalid JSON schema: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[error("Failed to canonicalize JSON schema: {0}")]
    Canonicalization(#[from] JsonCanonicalError),
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct JsonFingerprint {
    pub bytes: Vec<u8>,
}

impl JsonFingerprint {
    /// Fingerprint a JSON Schema subject, resolving its registry references transitively.
    pub fn from_subject(
        subject: &Subject,
        resolver: &impl ResolveSchemaReferences,
        opts: &JsonCanonicalOpts,
    ) -> Result<JsonFingerprint, JsonFingerprintError> {
        let references = ResolvedJsonReferences::from_subject(subject, resolver);
        Self::from_schema(&subject.schema, &references, opts)
    }

    /// Fingerprint a JSON Schema.
    ///
    /// The schema is reduced to its canonical structural form (see [`canonicalize`]): annotations
    /// dropped, registry references folded in, set-like arrays sorted, keys sorted, numbers
    /// normalized, whitespace stripped. A Rabin fingerprint is taken over the canonical bytes.
    pub fn from_schema(
        schema: &str,
        references: &ResolvedJsonReferences,
        opts: &JsonCanonicalOpts,
    ) -> Result<JsonFingerprint, JsonFingerprintError> {
        let value: Value = serde_json::from_str(schema)?;
        let canonical_value = canonicalize(&value, references, opts)?;
        let canonical = to_canonical_string(&canonical_value)?;

        let mut hasher = Rabin::default();
        hasher.update(canonical.as_bytes());
        let out = hasher.finalize();

        Ok(JsonFingerprint {
            bytes: out.to_vec(),
        })
    }

    /// Fingerprint a JSON Schema without registry references, using the default options.
    pub fn from_schema_str(schema: &str) -> Result<JsonFingerprint, JsonFingerprintError> {
        Self::from_schema(
            schema,
            &ResolvedJsonReferences::new(),
            &JsonCanonicalOpts::default(),
        )
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
        Display::fmt(self, f)
    }
}

impl Deref for JsonFingerprint {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

/// Keywords that are dropped from the canonical form by default.
///
/// These are annotation keywords: they carry documentation or metadata and do not affect which
/// instances a schema accepts. `$id` is an identifier, not structure, and is frequently generated
/// per application. `$schema` is deliberately *not* in this list because the draft changes the
/// semantics of other keywords.
pub const DEFAULT_IGNORED_KEYWORDS: &[&str] = &[
    "title",
    "description",
    "$comment",
    "examples",
    "default",
    "readOnly",
    "writeOnly",
    "deprecated",
    "$id",
];

/// Keywords whose value is a map of *names* to schemas. The names are user data (property names,
/// definition names) and are kept verbatim; only the values are canonicalized as schemas.
const SCHEMA_MAP_KEYWORDS: &[&str] = &[
    "properties",
    "patternProperties",
    "definitions",
    "$defs",
    "dependentSchemas",
    "dependencies",
];

/// Keywords whose value is a single schema.
const SCHEMA_KEYWORDS: &[&str] = &[
    "additionalItems",
    "contains",
    "additionalProperties",
    "unevaluatedProperties",
    "unevaluatedItems",
    "propertyNames",
    "if",
    "then",
    "else",
    "not",
    "contentSchema",
];

/// Keywords whose value is an ordered list of schemas. Order is significant and preserved.
const SCHEMA_ARRAY_KEYWORDS: &[&str] = &["allOf", "anyOf", "oneOf", "prefixItems"];

/// Keywords whose array value is semantically a set. Elements are sorted.
const SET_KEYWORDS: &[&str] = &["required", "type", "enum"];

/// Options controlling canonicalization.
///
/// This is the single place a future CLI flag or config entry has to feed into in order to make the
/// ignored keyword list user-configurable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonCanonicalOpts {
    /// Keywords dropped wherever they appear as a *schema keyword*.
    pub ignored_keywords: BTreeSet<String>,
}

impl Default for JsonCanonicalOpts {
    fn default() -> Self {
        Self {
            ignored_keywords: DEFAULT_IGNORED_KEYWORDS
                .iter()
                .map(|k| k.to_string())
                .collect(),
        }
    }
}

/// Schemas referenced by a subject, keyed by the reference *name* as registered in the registry.
///
/// For JSON Schema, the reference name is the string that appears in `$ref`. References are
/// transitive: each resolved reference carries the references of the referenced subject.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedJsonReferences {
    refs: HashMap<String, ResolvedJsonReference>,
}

/// A referenced schema together with its own (transitively resolved) references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedJsonReference {
    pub schema: String,
    pub references: ResolvedJsonReferences,
}

impl ResolvedJsonReferences {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve the references of a subject transitively through `resolver`.
    ///
    /// A reference back to a subject already on the current resolution path is left unresolved,
    /// which keeps its `$ref` string in the canonical form instead of recursing forever.
    /// Unresolved references and resolver errors are logged and skipped.
    pub fn from_subject(subject: &Subject, resolver: &impl ResolveSchemaReferences) -> Self {
        let mut visited = HashSet::new();
        visited.insert((subject.subject.clone(), subject.version));
        Self::resolve_transitively(subject, resolver, &mut visited)
    }

    fn resolve_transitively(
        subject: &Subject,
        resolver: &impl ResolveSchemaReferences,
        visited: &mut HashSet<(SubjectName, SchemaVersion)>,
    ) -> Self {
        let mut resolved = Self::new();

        for reference in subject.references.iter() {
            match resolver.resolve_schema_reference(reference) {
                Ok(Resolution::Resolved(schema_ref, referenced)) => {
                    let key = (referenced.subject.clone(), referenced.version);
                    if visited.contains(&key) {
                        tracing::warn!(
                            "Cyclic JSON schema reference {:?} from subject {} version {}, left unresolved",
                            schema_ref,
                            subject.subject,
                            subject.version
                        );
                        continue;
                    }

                    visited.insert(key.clone());
                    let nested = Self::resolve_transitively(&referenced, resolver, visited);
                    visited.remove(&key);

                    resolved.insert(
                        schema_ref.name.clone(),
                        ResolvedJsonReference::new(referenced.schema).with_references(nested),
                    );
                }
                Ok(Resolution::Unresolved(schema_ref)) => {
                    tracing::warn!("Unresolved schema reference: {:?}", schema_ref)
                }
                Err(err) => {
                    tracing::error!("Error resolving schema reference: {:?}", err)
                }
            }
        }

        resolved
    }

    pub fn insert(&mut self, name: impl Into<String>, reference: ResolvedJsonReference) {
        self.refs.insert(name.into(), reference);
    }

    pub fn with(mut self, name: impl Into<String>, reference: ResolvedJsonReference) -> Self {
        self.insert(name, reference);
        self
    }

    pub fn get(&self, name: &str) -> Option<&ResolvedJsonReference> {
        self.refs.get(name)
    }

    /// Find the reference a `$ref` string points to.
    ///
    /// Registries may store `$ref` verbatim (`customer.json`) or rewritten to an absolute URI
    /// against the schema's `$id` (`https://kannika.io/schemas/customer.json`, as Redpanda does).
    /// A `$ref` matches a reference when the strings are equal, or when both resolve to the same
    /// URI against `base`.
    pub fn resolve(&self, reference: &str, base: Option<&Url>) -> Option<&ResolvedJsonReference> {
        if let Some(found) = self.refs.get(reference) {
            return Some(found);
        }

        let target = resolve_uri(reference, base)?;
        self.refs
            .iter()
            .find(|(name, _)| resolve_uri(name, base).as_ref() == Some(&target))
            .map(|(_, found)| found)
    }

    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }

    pub fn len(&self) -> usize {
        self.refs.len()
    }
}

impl ResolvedJsonReference {
    pub fn new(schema: impl Into<String>) -> Self {
        Self {
            schema: schema.into(),
            references: ResolvedJsonReferences::new(),
        }
    }

    pub fn with_references(mut self, references: ResolvedJsonReferences) -> Self {
        self.references = references;
        self
    }
}

/// Resolve a (possibly relative) URI reference against a base, per RFC 3986.
/// Returns `None` when there is no base and the reference is not an absolute URI.
fn resolve_uri(reference: &str, base: Option<&Url>) -> Option<Url> {
    match base {
        Some(base) => base.join(reference).ok(),
        None => Url::parse(reference).ok(),
    }
}

/// The base URI for a schema object: its own `$id` resolved against the enclosing base.
fn base_uri_for(map: &Map<String, Value>, base: Option<&Url>) -> Option<Url> {
    match map.get("$id") {
        Some(Value::String(id)) => resolve_uri(id, base).or_else(|| base.cloned()),
        _ => base.cloned(),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JsonCanonicalError {
    #[error("Invalid JSON in referenced schema '{0}': {1}")]
    InvalidReferencedSchema(String, serde_json::Error),

    #[error("Failed to serialize canonical JSON: {0}")]
    Serialize(String),
}

/// Reduce a JSON Schema to its canonical structural form.
///
/// See the module documentation for the rules. Non-object nodes (boolean schemas, scalars, arrays
/// at the root) are returned unchanged.
pub fn canonicalize(
    schema: &Value,
    refs: &ResolvedJsonReferences,
    opts: &JsonCanonicalOpts,
) -> Result<Value, JsonCanonicalError> {
    canonicalize_schema(schema, refs, opts, None)
}

/// Serialize a canonical value to its deterministic string form (sorted keys, compact,
/// integer-valued floats written as integers).
pub fn to_canonical_string(value: &Value) -> Result<String, JsonCanonicalError> {
    jsonschema::canonical::json::to_string(value)
        .map_err(|e| JsonCanonicalError::Serialize(e.to_string()))
}

fn canonicalize_schema(
    node: &Value,
    refs: &ResolvedJsonReferences,
    opts: &JsonCanonicalOpts,
    base: Option<&Url>,
) -> Result<Value, JsonCanonicalError> {
    let Value::Object(map) = node else {
        return Ok(node.clone());
    };

    // `$id` (even when ignored for hashing) sets the base URI for `$ref` resolution below.
    let base = base_uri_for(map, base);
    let base = base.as_ref();

    let mut out = Map::with_capacity(map.len());

    for (key, value) in map {
        if opts.ignored_keywords.contains(key) {
            continue;
        }

        let canonical = match key.as_str() {
            "$ref" => fold_ref(value, refs, opts, base)?,
            "items" => match value {
                Value::Array(_) => canonicalize_schema_array(value, refs, opts, base)?,
                _ => canonicalize_schema(value, refs, opts, base)?,
            },
            k if SCHEMA_MAP_KEYWORDS.contains(&k) => {
                canonicalize_schema_map(value, refs, opts, base)?
            }
            k if SCHEMA_KEYWORDS.contains(&k) => canonicalize_schema(value, refs, opts, base)?,
            k if SCHEMA_ARRAY_KEYWORDS.contains(&k) => {
                canonicalize_schema_array(value, refs, opts, base)?
            }
            k if SET_KEYWORDS.contains(&k) => sort_set(value)?,
            _ => value.clone(),
        };

        out.insert(key.clone(), canonical);
    }

    Ok(Value::Object(out))
}

/// `"$ref": "<name>"` is replaced by `"$ref": { ...canonical referenced schema... }` when `<name>`
/// is a resolved registry reference (by exact name, or by URI resolved against `base`).
/// Local refs (`#...`) and unresolved refs are left as-is.
fn fold_ref(
    value: &Value,
    refs: &ResolvedJsonReferences,
    opts: &JsonCanonicalOpts,
    base: Option<&Url>,
) -> Result<Value, JsonCanonicalError> {
    let Value::String(name) = value else {
        return Ok(value.clone());
    };

    if name.starts_with('#') {
        return Ok(value.clone());
    }

    match refs.resolve(name, base) {
        Some(resolved) => {
            let referenced: Value = serde_json::from_str(&resolved.schema)
                .map_err(|e| JsonCanonicalError::InvalidReferencedSchema(name.clone(), e))?;
            // The referenced document is its own root: its base comes from its own `$id`.
            canonicalize_schema(&referenced, &resolved.references, opts, None)
        }
        None => {
            tracing::debug!(
                "JSON schema $ref '{}' is not a registry reference, kept as-is",
                name
            );
            Ok(value.clone())
        }
    }
}

fn canonicalize_schema_map(
    value: &Value,
    refs: &ResolvedJsonReferences,
    opts: &JsonCanonicalOpts,
    base: Option<&Url>,
) -> Result<Value, JsonCanonicalError> {
    let Value::Object(map) = value else {
        return Ok(value.clone());
    };

    let mut out = Map::with_capacity(map.len());
    for (name, schema) in map {
        // `dependencies` (draft-07) may map to an array of property names instead of a schema.
        // `canonicalize_schema` returns non-objects unchanged, so that case is handled naturally.
        out.insert(name.clone(), canonicalize_schema(schema, refs, opts, base)?);
    }
    Ok(Value::Object(out))
}

fn canonicalize_schema_array(
    value: &Value,
    refs: &ResolvedJsonReferences,
    opts: &JsonCanonicalOpts,
    base: Option<&Url>,
) -> Result<Value, JsonCanonicalError> {
    let Value::Array(items) = value else {
        return canonicalize_schema(value, refs, opts, base);
    };

    items
        .iter()
        .map(|item| canonicalize_schema(item, refs, opts, base))
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

/// Sort a set-like array deterministically. Elements are compared by their canonical string form
/// so heterogeneous `enum` values sort consistently. Non-arrays (e.g. `"type": "string"`) are kept.
fn sort_set(value: &Value) -> Result<Value, JsonCanonicalError> {
    let Value::Array(items) = value else {
        return Ok(value.clone());
    };

    let mut keyed = items
        .iter()
        .map(|item| to_canonical_string(item).map(|key| (key, item.clone())))
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(Value::Array(
        keyed.into_iter().map(|(_, item)| item).collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fp(schema: &str) -> JsonFingerprint {
        JsonFingerprint::from_schema_str(schema)
            .unwrap_or_else(|e| panic!("failed to fingerprint {schema}: {e}"))
    }

    fn fp_with(schema: &str, refs: &ResolvedJsonReferences) -> JsonFingerprint {
        JsonFingerprint::from_schema(schema, refs, &JsonCanonicalOpts::default())
            .unwrap_or_else(|e| panic!("failed to fingerprint {schema}: {e}"))
    }

    fn canonical(schema: &str) -> String {
        let value: Value = serde_json::from_str(schema).unwrap();
        let canonical = canonicalize(
            &value,
            &ResolvedJsonReferences::new(),
            &JsonCanonicalOpts::default(),
        )
        .unwrap();
        to_canonical_string(&canonical).unwrap()
    }

    #[track_caller]
    fn assert_same_fp(a: &str, b: &str) {
        assert_eq!(
            fp(a),
            fp(b),
            "expected same fingerprint\nleft:  {}\nright: {}",
            canonical(a),
            canonical(b)
        );
    }

    #[track_caller]
    fn assert_diff_fp(a: &str, b: &str) {
        assert_ne!(
            fp(a),
            fp(b),
            "expected different fingerprint, both canonicalized to: {}",
            canonical(a)
        );
    }

    // ------------------------------------------------------------------
    // The reported bug: identical structure, different title.
    // ------------------------------------------------------------------

    const ORDER_PLACED_A: &str = r##"{
      "$schema": "http://json-schema.org/draft-07/schema#",
      "title": "io.kannika.examples.OrderPlaced",
      "type": "object",
      "properties": {
        "eventType": { "type": "string" },
        "eventId": { "type": "string" }
      }
    }"##;

    const ORDER_PLACED_B: &str = r##"{
      "$schema": "http://json-schema.org/draft-07/schema#",
      "title": "com.acme.orders.OrderPlaced",
      "type": "object",
      "properties": {
        "eventType": { "type": "string" },
        "eventId": { "type": "string" }
      }
    }"##;

    #[test]
    fn title_only_difference_is_ignored() {
        assert_same_fp(ORDER_PLACED_A, ORDER_PLACED_B);
    }

    #[test]
    fn canonical_form_of_reported_schema_has_no_title() {
        assert_eq!(
            canonical(ORDER_PLACED_A),
            r##"{"$schema":"http://json-schema.org/draft-07/schema#","properties":{"eventId":{"type":"string"},"eventType":{"type":"string"}},"type":"object"}"##
        );
    }

    // ------------------------------------------------------------------
    // Each default ignored keyword, at root and nested.
    // ------------------------------------------------------------------

    #[test]
    fn every_default_ignored_keyword_is_dropped_at_root() {
        let base = r##"{"type": "object", "properties": {"a": {"type": "string"}}}"##;
        for (keyword, value) in [
            ("title", json!("T")),
            ("description", json!("D")),
            ("$comment", json!("C")),
            ("examples", json!([{"a": "x"}])),
            ("default", json!({"a": "x"})),
            ("readOnly", json!(true)),
            ("writeOnly", json!(true)),
            ("deprecated", json!(true)),
            ("$id", json!("https://example.com/schemas/a.json")),
        ] {
            let mut with: Value = serde_json::from_str(base).unwrap();
            with[keyword] = value;
            assert_eq!(
                fp(base),
                fp(&with.to_string()),
                "keyword `{keyword}` should be ignored at root"
            );
        }
    }

    #[test]
    fn every_default_ignored_keyword_is_dropped_when_nested() {
        // Nested under: properties.x, items, oneOf[0], $defs.X, additionalProperties, if/then
        let base = r##"{
          "type": "object",
          "properties": {"x": {"type": "string"}},
          "items": {"type": "integer"},
          "oneOf": [{"type": "string"}, {"type": "null"}],
          "$defs": {"X": {"type": "number"}},
          "additionalProperties": {"type": "boolean"},
          "if": {"properties": {"x": {"const": "a"}}},
          "then": {"required": ["y"]}
        }"##;
        let pointers = [
            "/properties/x",
            "/items",
            "/oneOf/0",
            "/$defs/X",
            "/additionalProperties",
            "/if",
            "/if/properties/x",
            "/then",
        ];
        for keyword in DEFAULT_IGNORED_KEYWORDS {
            for pointer in pointers {
                let mut with: Value = serde_json::from_str(base).unwrap();
                with.pointer_mut(pointer).unwrap()[*keyword] = json!("noise");
                assert_eq!(
                    fp(base),
                    fp(&with.to_string()),
                    "keyword `{keyword}` should be ignored at {pointer}"
                );
            }
        }
    }

    #[test]
    fn description_and_examples_differences_are_ignored() {
        assert_same_fp(
            r##"{"type": "object", "description": "one", "examples": [{"a": 1}]}"##,
            r##"{"type": "object", "description": "two", "examples": []}"##,
        );
        assert_same_fp(
            r##"{"type": "object", "description": "one"}"##,
            r##"{"type": "object"}"##,
        );
    }

    #[test]
    fn id_difference_is_ignored_but_schema_draft_is_not() {
        assert_same_fp(
            r##"{"$id": "https://a.example/x.json", "type": "string"}"##,
            r##"{"$id": "https://b.example/y.json", "type": "string"}"##,
        );
        assert_diff_fp(
            r##"{"$schema": "http://json-schema.org/draft-07/schema#", "type": "string"}"##,
            r##"{"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "string"}"##,
        );
    }

    // ------------------------------------------------------------------
    // Textual normalization.
    // ------------------------------------------------------------------

    #[test]
    fn key_order_and_whitespace_are_ignored() {
        assert_same_fp(
            "{ \"type\" : \"object\",\n\n \"properties\" : { \"b\" : {\"type\":\"string\"}, \"a\" : {\"type\":\"integer\"} } }",
            r##"{"properties":{"a":{"type":"integer"},"b":{"type":"string"}},"type":"object"}"##,
        );
    }

    #[test]
    fn number_representations_are_normalized() {
        assert_same_fp(
            r##"{"type": "number", "minimum": 1.0, "maximum": 1e2, "multipleOf": 0.5}"##,
            r##"{"type": "number", "minimum": 1, "maximum": 100, "multipleOf": 0.5}"##,
        );
        assert_same_fp(
            r##"{"type": "integer", "minimum": -0.0}"##,
            r##"{"type": "integer", "minimum": 0}"##,
        );
    }

    #[test]
    fn unicode_escapes_are_normalized() {
        assert_same_fp(
            r##"{"type": "string", "pattern": "^café$"}"##,
            r##"{"type": "string", "pattern": "^café$"}"##,
        );
    }

    // ------------------------------------------------------------------
    // Set-like arrays.
    // ------------------------------------------------------------------

    #[test]
    fn required_order_is_ignored() {
        assert_same_fp(
            r##"{"type": "object", "required": ["b", "a", "c"]}"##,
            r##"{"type": "object", "required": ["a", "b", "c"]}"##,
        );
    }

    #[test]
    fn required_contents_matter() {
        assert_diff_fp(
            r##"{"type": "object", "required": ["a"]}"##,
            r##"{"type": "object", "required": ["a", "b"]}"##,
        );
        assert_diff_fp(
            r##"{"type": "object", "required": ["a"]}"##,
            r##"{"type": "object"}"##,
        );
    }

    #[test]
    fn type_array_order_is_ignored() {
        assert_same_fp(
            r##"{"type": ["string", "null"]}"##,
            r##"{"type": ["null", "string"]}"##,
        );
        assert_diff_fp(
            r##"{"type": ["string", "null"]}"##,
            r##"{"type": "string"}"##,
        );
        assert_diff_fp(
            r##"{"type": ["string", "null"]}"##,
            r##"{"type": ["integer", "null"]}"##,
        );
    }

    #[test]
    fn enum_order_is_ignored_but_members_matter() {
        assert_same_fp(
            r##"{"enum": ["red", "green", "blue"]}"##,
            r##"{"enum": ["blue", "red", "green"]}"##,
        );
        assert_same_fp(
            r##"{"enum": [1, "a", null, {"k": "v"}, [1, 2], true]}"##,
            r##"{"enum": [true, [1, 2], {"k": "v"}, null, "a", 1]}"##,
        );
        assert_diff_fp(
            r##"{"enum": ["red", "green"]}"##,
            r##"{"enum": ["red", "green", "blue"]}"##,
        );
    }

    // ------------------------------------------------------------------
    // Keyword-awareness: "title" as property name or literal is NOT stripped.
    // ------------------------------------------------------------------

    #[test]
    fn property_named_like_an_annotation_is_kept() {
        let with_title_property = r##"{
          "type": "object",
          "properties": {
            "title": {"type": "string"},
            "description": {"type": "string"},
            "default": {"type": "string"},
            "$id": {"type": "string"}
          }
        }"##;
        let without = r##"{"type": "object", "properties": {}}"##;
        assert_diff_fp(with_title_property, without);

        let canonical = canonical(with_title_property);
        assert!(
            canonical.contains(r##""title":{"type":"string"}"##),
            "{canonical}"
        );
        assert!(
            canonical.contains(r##""$id":{"type":"string"}"##),
            "{canonical}"
        );
    }

    #[test]
    fn property_named_title_with_different_types_differs() {
        assert_diff_fp(
            r##"{"type": "object", "properties": {"title": {"type": "string"}}}"##,
            r##"{"type": "object", "properties": {"title": {"type": "integer"}}}"##,
        );
    }

    #[test]
    fn property_named_title_with_same_type_but_different_annotation_matches() {
        assert_same_fp(
            r##"{"type": "object", "properties": {"title": {"type": "string", "title": "Title A"}}}"##,
            r##"{"type": "object", "properties": {"title": {"type": "string", "title": "Title B"}}}"##,
        );
    }

    #[test]
    fn annotation_keys_inside_pattern_properties_and_definitions_names_are_kept() {
        // Names under patternProperties / definitions / $defs / dependentSchemas are data.
        assert_diff_fp(
            r##"{"type": "object", "patternProperties": {"^title$": {"type": "string"}}}"##,
            r##"{"type": "object", "patternProperties": {"^name$": {"type": "string"}}}"##,
        );
        assert_diff_fp(
            r##"{"definitions": {"title": {"type": "string"}}}"##,
            r##"{"definitions": {}}"##,
        );
        assert_diff_fp(
            r##"{"$defs": {"description": {"type": "string"}}}"##,
            r##"{"$defs": {}}"##,
        );
        assert_diff_fp(
            r##"{"dependentSchemas": {"default": {"required": ["x"]}}}"##,
            r##"{"dependentSchemas": {}}"##,
        );
    }

    #[test]
    fn const_and_enum_literals_are_not_stripped() {
        assert_diff_fp(
            r##"{"const": {"title": "a"}}"##,
            r##"{"const": {"title": "b"}}"##,
        );
        assert_diff_fp(r##"{"const": {"title": "a"}}"##, r##"{"const": {}}"##);
        assert_diff_fp(
            r##"{"enum": [{"description": "x"}, {"description": "y"}]}"##,
            r##"{"enum": [{"description": "x"}, {"description": "z"}]}"##,
        );
    }

    #[test]
    fn dependencies_array_form_is_kept_verbatim() {
        // draft-07 `dependencies` can map to a list of property names.
        assert_diff_fp(
            r##"{"type": "object", "dependencies": {"a": ["b"]}}"##,
            r##"{"type": "object", "dependencies": {"a": ["c"]}}"##,
        );
        // ...or to a schema, in which case annotations inside are stripped.
        assert_same_fp(
            r##"{"type": "object", "dependencies": {"a": {"required": ["b"], "title": "x"}}}"##,
            r##"{"type": "object", "dependencies": {"a": {"required": ["b"]}}}"##,
        );
    }

    // ------------------------------------------------------------------
    // Structural differences must change the fingerprint.
    // ------------------------------------------------------------------

    #[test]
    fn structural_edits_change_fingerprint() {
        let cases: &[(&str, &str, &str)] = &[
            (
                "property type",
                r##"{"type": "object", "properties": {"a": {"type": "string"}}}"##,
                r##"{"type": "object", "properties": {"a": {"type": "integer"}}}"##,
            ),
            (
                "added property",
                r##"{"type": "object", "properties": {"a": {"type": "string"}}}"##,
                r##"{"type": "object", "properties": {"a": {"type": "string"}, "b": {"type": "string"}}}"##,
            ),
            (
                "additionalProperties false vs absent",
                r##"{"type": "object", "additionalProperties": false}"##,
                r##"{"type": "object"}"##,
            ),
            (
                "additionalProperties schema",
                r##"{"type": "object", "additionalProperties": {"type": "string"}}"##,
                r##"{"type": "object", "additionalProperties": {"type": "integer"}}"##,
            ),
            (
                "minimum",
                r##"{"type": "integer", "minimum": 0}"##,
                r##"{"type": "integer", "minimum": 1}"##,
            ),
            (
                "exclusiveMinimum",
                r##"{"type": "integer", "exclusiveMinimum": 0}"##,
                r##"{"type": "integer", "minimum": 0}"##,
            ),
            (
                "items type",
                r##"{"type": "array", "items": {"type": "string"}}"##,
                r##"{"type": "array", "items": {"type": "number"}}"##,
            ),
            (
                "oneOf member",
                r##"{"oneOf": [{"type": "string"}, {"type": "null"}]}"##,
                r##"{"oneOf": [{"type": "string"}, {"type": "integer"}]}"##,
            ),
            (
                "anyOf vs oneOf",
                r##"{"anyOf": [{"type": "string"}, {"type": "null"}]}"##,
                r##"{"oneOf": [{"type": "string"}, {"type": "null"}]}"##,
            ),
            (
                "if/then/else branch",
                r##"{"if": {"properties": {"k": {"const": "a"}}}, "then": {"required": ["x"]}, "else": {"required": ["y"]}}"##,
                r##"{"if": {"properties": {"k": {"const": "a"}}}, "then": {"required": ["y"]}, "else": {"required": ["x"]}}"##,
            ),
            (
                "prefixItems order",
                r##"{"type": "array", "prefixItems": [{"type": "string"}, {"type": "integer"}]}"##,
                r##"{"type": "array", "prefixItems": [{"type": "integer"}, {"type": "string"}]}"##,
            ),
            (
                "allOf order",
                r##"{"allOf": [{"required": ["a"]}, {"required": ["b"]}]}"##,
                r##"{"allOf": [{"required": ["b"]}, {"required": ["a"]}]}"##,
            ),
            (
                "pattern",
                r##"{"type": "string", "pattern": "^a$"}"##,
                r##"{"type": "string", "pattern": "^b$"}"##,
            ),
            (
                "format",
                r##"{"type": "string", "format": "date-time"}"##,
                r##"{"type": "string", "format": "date"}"##,
            ),
            (
                "minLength",
                r##"{"type": "string", "minLength": 1}"##,
                r##"{"type": "string", "minLength": 2}"##,
            ),
            (
                "uniqueItems",
                r##"{"type": "array", "uniqueItems": true}"##,
                r##"{"type": "array", "uniqueItems": false}"##,
            ),
            (
                "multipleOf",
                r##"{"type": "number", "multipleOf": 0.5}"##,
                r##"{"type": "number", "multipleOf": 1}"##,
            ),
            (
                "not",
                r##"{"not": {"type": "string"}}"##,
                r##"{"not": {"type": "integer"}}"##,
            ),
            (
                "contains",
                r##"{"type": "array", "contains": {"const": 1}}"##,
                r##"{"type": "array", "contains": {"const": 2}}"##,
            ),
            (
                "propertyNames",
                r##"{"type": "object", "propertyNames": {"pattern": "^a"}}"##,
                r##"{"type": "object", "propertyNames": {"pattern": "^b"}}"##,
            ),
            (
                "dependentRequired",
                r##"{"type": "object", "dependentRequired": {"a": ["b"]}}"##,
                r##"{"type": "object", "dependentRequired": {"a": ["c"]}}"##,
            ),
            ("const", r##"{"const": 1}"##, r##"{"const": 2}"##),
            (
                "nullable via type array vs plain",
                r##"{"type": "object", "properties": {"a": {"type": ["string", "null"]}}}"##,
                r##"{"type": "object", "properties": {"a": {"type": "string"}}}"##,
            ),
        ];

        for (name, a, b) in cases {
            assert_ne!(
                fp(a),
                fp(b),
                "case `{name}` should produce different fingerprints"
            );
        }
    }

    #[test]
    fn custom_keywords_are_part_of_fingerprint() {
        // Denylist policy: unknown keys are kept. Making these configurable is a follow-up.
        assert_diff_fp(
            r##"{"type": "object", "dataOwner": "IoT Platform"}"##,
            r##"{"type": "object"}"##,
        );
        assert_diff_fp(
            r##"{"type": "object", "x-kafka-topic": "a"}"##,
            r##"{"type": "object", "x-kafka-topic": "b"}"##,
        );
    }

    #[test]
    fn custom_ignored_keywords_can_be_configured() {
        let opts = JsonCanonicalOpts {
            ignored_keywords: ["dataOwner".to_string()].into_iter().collect(),
        };
        let refs = ResolvedJsonReferences::new();
        let a = JsonFingerprint::from_schema(
            r##"{"type": "object", "dataOwner": "IoT Platform"}"##,
            &refs,
            &opts,
        )
        .unwrap();
        let b = JsonFingerprint::from_schema(r##"{"type": "object"}"##, &refs, &opts).unwrap();
        assert_eq!(a, b);

        // With only `dataOwner` ignored, `title` is now significant.
        let c = JsonFingerprint::from_schema(r##"{"type": "object", "title": "x"}"##, &refs, &opts)
            .unwrap();
        assert_ne!(b, c);
    }

    // ------------------------------------------------------------------
    // $ref folding through registry references.
    // ------------------------------------------------------------------

    const CUSTOMER: &str = r##"{
      "$schema": "http://json-schema.org/draft-07/schema#",
      "title": "Customer",
      "type": "object",
      "properties": {"name": {"type": "string"}, "email": {"type": "string", "format": "email"}},
      "required": ["name"]
    }"##;

    const CUSTOMER_INT_NAME: &str = r##"{
      "$schema": "http://json-schema.org/draft-07/schema#",
      "title": "Customer",
      "type": "object",
      "properties": {"name": {"type": "integer"}, "email": {"type": "string", "format": "email"}},
      "required": ["name"]
    }"##;

    const ORDER: &str = r##"{
      "$schema": "http://json-schema.org/draft-07/schema#",
      "title": "Order",
      "type": "object",
      "properties": {
        "id": {"type": "string"},
        "customer": {"$ref": "customer.json"}
      },
      "required": ["id", "customer"]
    }"##;

    fn customer_refs(schema: &str) -> ResolvedJsonReferences {
        ResolvedJsonReferences::new().with("customer.json", ResolvedJsonReference::new(schema))
    }

    #[test]
    fn external_ref_is_folded_to_referenced_content() {
        let folded = fp_with(ORDER, &customer_refs(CUSTOMER));

        let hand_inlined = r##"{
          "$schema": "http://json-schema.org/draft-07/schema#",
          "type": "object",
          "properties": {
            "id": {"type": "string"},
            "customer": {"$ref": {
              "$schema": "http://json-schema.org/draft-07/schema#",
              "type": "object",
              "properties": {"name": {"type": "string"}, "email": {"type": "string", "format": "email"}},
              "required": ["name"]
            }}
          },
          "required": ["id", "customer"]
        }"##;
        assert_eq!(folded, fp(hand_inlined));
    }

    #[test]
    fn folded_ref_differs_from_unfolded_ref() {
        assert_ne!(fp_with(ORDER, &customer_refs(CUSTOMER)), fp(ORDER));
    }

    #[test]
    fn same_content_under_different_reference_names_matches() {
        let order_a = ORDER;
        let order_b = ORDER.replace("customer.json", "com/acme/Customer.schema.json");

        let refs_a = customer_refs(CUSTOMER);
        let refs_b = ResolvedJsonReferences::new().with(
            "com/acme/Customer.schema.json",
            ResolvedJsonReference::new(CUSTOMER),
        );

        assert_eq!(fp_with(order_a, &refs_a), fp_with(&order_b, &refs_b));
    }

    #[test]
    fn referenced_content_difference_changes_fingerprint() {
        assert_ne!(
            fp_with(ORDER, &customer_refs(CUSTOMER)),
            fp_with(ORDER, &customer_refs(CUSTOMER_INT_NAME))
        );
    }

    #[test]
    fn referenced_annotation_difference_is_ignored() {
        let customer_other_title =
            CUSTOMER.replace(r##""title": "Customer""##, r##""title": "Kunde""##);
        assert_eq!(
            fp_with(ORDER, &customer_refs(CUSTOMER)),
            fp_with(ORDER, &customer_refs(&customer_other_title))
        );
    }

    #[test]
    fn nested_refs_fold_transitively() {
        let address = r##"{"type": "object", "properties": {"street": {"type": "string"}}}"##;
        let customer_with_address = r##"{
          "type": "object",
          "properties": {"name": {"type": "string"}, "address": {"$ref": "address.json"}}
        }"##;
        let refs = ResolvedJsonReferences::new().with(
            "customer.json",
            ResolvedJsonReference::new(customer_with_address).with_references(
                ResolvedJsonReferences::new()
                    .with("address.json", ResolvedJsonReference::new(address)),
            ),
        );

        let folded = fp_with(ORDER, &refs);
        let hand_inlined = r##"{
          "$schema": "http://json-schema.org/draft-07/schema#",
          "type": "object",
          "properties": {
            "id": {"type": "string"},
            "customer": {"$ref": {
              "type": "object",
              "properties": {
                "name": {"type": "string"},
                "address": {"$ref": {"type": "object", "properties": {"street": {"type": "string"}}}}
              }
            }}
          },
          "required": ["id", "customer"]
        }"##;
        assert_eq!(folded, fp(hand_inlined));

        // Not resolving the nested ref leaves the string and yields a different fingerprint.
        let shallow = ResolvedJsonReferences::new().with(
            "customer.json",
            ResolvedJsonReference::new(customer_with_address),
        );
        assert_ne!(folded, fp_with(ORDER, &shallow));
    }

    #[test]
    fn cyclic_references_terminate_deterministically() {
        // a -> b -> a. The builder guards cycles by leaving the back-reference unresolved,
        // so the innermost `$ref` stays a string. The canonicalizer must handle that tree.
        let a = r##"{"type": "object", "properties": {"b": {"$ref": "b.json"}}}"##;
        let b = r##"{"type": "object", "properties": {"a": {"$ref": "a.json"}}}"##;
        let refs = ResolvedJsonReferences::new().with(
            "b.json",
            ResolvedJsonReference::new(b), // b's reference back to a is left unresolved
        );

        let first = fp_with(a, &refs);
        let second = fp_with(a, &refs);
        assert_eq!(first, second);

        let expected = r##"{"type": "object", "properties": {"b": {"$ref":
          {"type": "object", "properties": {"a": {"$ref": "a.json"}}}
        }}}"##;
        assert_eq!(first, fp(expected));
    }

    #[test]
    fn unresolved_ref_is_kept_as_string() {
        let refs = ResolvedJsonReferences::new();
        assert_eq!(fp_with(ORDER, &refs), fp(ORDER));
        let canonical = canonical(ORDER);
        assert!(
            canonical.contains(r##""$ref":"customer.json""##),
            "{canonical}"
        );
    }

    #[test]
    fn local_refs_are_untouched() {
        let schema = r##"{
          "type": "object",
          "definitions": {"Name": {"type": "string"}},
          "$defs": {"Age": {"type": "integer"}},
          "properties": {
            "name": {"$ref": "#/definitions/Name"},
            "age": {"$ref": "#/$defs/Age"},
            "self": {"$ref": "#"}
          }
        }"##;
        let canonical = canonical(schema);
        assert!(
            canonical.contains(r##""$ref":"#/definitions/Name""##),
            "{canonical}"
        );
        assert!(
            canonical.contains(r##""$ref":"#/$defs/Age""##),
            "{canonical}"
        );
        assert!(canonical.contains(r##""$ref":"#""##), "{canonical}");

        assert_diff_fp(
            r##"{"properties": {"a": {"$ref": "#/definitions/X"}}}"##,
            r##"{"properties": {"a": {"$ref": "#/definitions/Y"}}}"##,
        );
    }

    #[test]
    fn ref_with_siblings_keeps_siblings() {
        let schema = r##"{"properties": {"c": {"$ref": "customer.json", "minProperties": 1, "title": "x"}}}"##;
        let refs = customer_refs(CUSTOMER);
        let value: Value = serde_json::from_str(schema).unwrap();
        let canonical = canonicalize(&value, &refs, &JsonCanonicalOpts::default()).unwrap();

        let c = &canonical["properties"]["c"];
        assert_eq!(c["minProperties"], json!(1));
        assert!(c["$ref"].is_object(), "$ref should be folded: {c}");
        assert!(c.get("title").is_none());
    }

    #[test]
    fn refs_are_folded_in_every_schema_position() {
        let schema = r##"{
          "type": "object",
          "properties": {"a": {"$ref": "customer.json"}},
          "items": {"$ref": "customer.json"},
          "oneOf": [{"type": "null"}, {"$ref": "customer.json"}],
          "$defs": {"C": {"$ref": "customer.json"}},
          "additionalProperties": {"$ref": "customer.json"},
          "not": {"$ref": "customer.json"},
          "prefixItems": [{"$ref": "customer.json"}],
          "patternProperties": {"^x": {"$ref": "customer.json"}}
        }"##;
        let refs = customer_refs(CUSTOMER);
        let value: Value = serde_json::from_str(schema).unwrap();
        let canonical = canonicalize(&value, &refs, &JsonCanonicalOpts::default()).unwrap();
        let text = to_canonical_string(&canonical).unwrap();
        assert!(
            !text.contains(r##""$ref":"customer.json""##),
            "all refs should be folded: {text}"
        );
        assert_eq!(text.matches(r##""$ref":{"##).count(), 8, "{text}");
    }

    #[test]
    fn non_string_ref_is_kept() {
        // Already-folded or malformed `$ref` values are passed through.
        assert_diff_fp(
            r##"{"$ref": {"type": "string"}}"##,
            r##"{"$ref": {"type": "integer"}}"##,
        );
        assert_same_fp(r##"{"$ref": 1}"##, r##"{"$ref": 1.0}"##);
    }

    #[test]
    fn invalid_json_in_referenced_schema_is_an_error() {
        let refs = ResolvedJsonReferences::new()
            .with("customer.json", ResolvedJsonReference::new("{ nope"));
        let err =
            JsonFingerprint::from_schema(ORDER, &refs, &JsonCanonicalOpts::default()).unwrap_err();
        assert!(err.to_string().contains("customer.json"), "{err}");
    }

    // ------------------------------------------------------------------
    // $ref strings rewritten to absolute URIs by the registry (Redpanda resolves relative
    // refs against the root `$id` before storing the schema).
    // ------------------------------------------------------------------

    const ORDER_WITH_ID: &str = r##"{
      "$schema": "http://json-schema.org/draft-07/schema#",
      "$id": "https://kannika.io/schemas/order.json",
      "title": "Order",
      "type": "object",
      "properties": {
        "id": {"type": "string"},
        "customer": {"$ref": "customer.json"}
      },
      "required": ["id", "customer"]
    }"##;

    #[test]
    fn absolute_ref_resolved_against_root_id_matches_relative_reference_name() {
        let stored_by_redpanda = ORDER_WITH_ID.replace(
            r#""$ref": "customer.json""#,
            r#""$ref": "https://kannika.io/schemas/customer.json""#,
        );
        let refs = customer_refs(CUSTOMER);

        assert_eq!(
            fp_with(&stored_by_redpanda, &refs),
            fp_with(ORDER_WITH_ID, &refs),
            "absolute and relative forms of the same reference must fold identically"
        );
        assert_ne!(fp_with(&stored_by_redpanda, &refs), fp(&stored_by_redpanda));
    }

    #[test]
    fn relative_ref_matches_absolute_reference_name() {
        let refs = ResolvedJsonReferences::new().with(
            "https://kannika.io/schemas/customer.json",
            ResolvedJsonReference::new(CUSTOMER),
        );
        assert_eq!(
            fp_with(ORDER_WITH_ID, &refs),
            fp_with(ORDER, &customer_refs(CUSTOMER)),
        );
    }

    #[test]
    fn absolute_ref_under_different_base_does_not_match() {
        let other_host = ORDER_WITH_ID.replace(
            r#""$ref": "customer.json""#,
            r#""$ref": "https://other.example/customer.json""#,
        );
        let refs = customer_refs(CUSTOMER);
        // Resolves to a different URI than `customer.json` against the root `$id`: left as-is.
        assert_eq!(fp_with(&other_host, &refs), fp(&other_host));
    }

    #[test]
    fn ref_without_any_base_only_matches_exactly() {
        let refs = customer_refs(CUSTOMER);
        let absolute = ORDER.replace(
            r#""$ref": "customer.json""#,
            r#""$ref": "https://kannika.io/schemas/customer.json""#,
        );
        assert_eq!(fp_with(&absolute, &refs), fp(&absolute));
    }

    #[test]
    fn nested_id_changes_base_for_refs_below_it() {
        let schema = r##"{
          "$id": "https://kannika.io/schemas/order.json",
          "type": "object",
          "properties": {
            "customer": {"$ref": "customer.json"},
            "nested": {
              "$id": "https://other.example/sub/",
              "properties": {"customer": {"$ref": "customer.json"}}
            }
          }
        }"##;
        let refs = ResolvedJsonReferences::new().with(
            "https://kannika.io/schemas/customer.json",
            ResolvedJsonReference::new(CUSTOMER),
        );
        let value: Value = serde_json::from_str(schema).unwrap();
        let canonical = canonicalize(&value, &refs, &JsonCanonicalOpts::default()).unwrap();

        assert!(canonical["properties"]["customer"]["$ref"].is_object());
        assert_eq!(
            canonical["properties"]["nested"]["properties"]["customer"]["$ref"],
            json!("customer.json"),
            "under the nested $id the ref resolves elsewhere and stays unresolved"
        );
    }

    #[test]
    fn id_is_still_stripped_after_being_used_as_base() {
        let refs = customer_refs(CUSTOMER);
        let value: Value = serde_json::from_str(ORDER_WITH_ID).unwrap();
        let canonical = canonicalize(&value, &refs, &JsonCanonicalOpts::default()).unwrap();
        assert!(canonical.get("$id").is_none());
    }

    #[test]
    fn invalid_id_does_not_break_canonicalization() {
        let schema =
            r##"{"$id": "::not a uri::", "properties": {"c": {"$ref": "customer.json"}}}"##;
        let refs = customer_refs(CUSTOMER);
        assert_eq!(
            fp_with(schema, &refs),
            fp_with(
                r##"{"properties": {"c": {"$ref": "customer.json"}}}"##,
                &refs
            )
        );
    }

    // ------------------------------------------------------------------
    // Robustness.
    // ------------------------------------------------------------------

    #[test]
    fn boolean_and_scalar_root_schemas_do_not_panic() {
        assert_same_fp("true", "true");
        assert_same_fp("false", " false ");
        assert_diff_fp("true", "false");
        assert_diff_fp("true", "{}");
        assert_diff_fp("false", "{\"not\": {}}");

        for scalar in ["1", "\"str\"", "null", "[]", "[{\"type\": \"string\"}]"] {
            let _ = fp(scalar);
        }
    }

    #[test]
    fn empty_schema_and_whitespace_variants_match() {
        assert_same_fp("{}", " { } ");
        assert_same_fp(
            r##"{"title": "only annotations", "description": "here"}"##,
            "{}",
        );
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(JsonFingerprint::from_schema_str("{ not valid json").is_err());
        assert!(JsonFingerprint::from_schema_str("").is_err());
        assert!(JsonFingerprint::from_schema_str(r##"{"type": "object",}"##).is_err());
    }

    #[test]
    fn excessive_nesting_is_an_error_not_a_panic() {
        let depth = 300;
        let mut schema = String::new();
        for _ in 0..depth {
            schema.push_str(r##"{"items":"##);
        }
        schema.push_str(r##"{"type":"string"}"##);
        for _ in 0..depth {
            schema.push('}');
        }
        assert!(JsonFingerprint::from_schema_str(&schema).is_err());
    }

    #[test]
    fn canonicalization_is_deterministic() {
        let a = fp(ORDER_PLACED_A);
        for _ in 0..10 {
            assert_eq!(a, fp(ORDER_PLACED_A));
        }
    }

    #[test]
    fn golden_fingerprint_guards_against_canonicalization_drift() {
        // If this changes, every previously computed JSON mapping changes. Bump deliberately.
        let fingerprint = fp(ORDER_PLACED_A).to_string();
        assert_eq!(fingerprint.len(), 16);
        assert!(
            fingerprint
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_eq!(fingerprint, "0d4d19db3a2eeeb2");
    }

    #[test]
    fn display_and_debug_print_hex() {
        let fingerprint = fp(r##"{"type": "string"}"##);
        assert_eq!(format!("{fingerprint}"), format!("{fingerprint:?}"));
        assert_eq!(format!("{fingerprint}").len(), 16);
    }

    // ------------------------------------------------------------------
    // Drafts.
    // ------------------------------------------------------------------

    #[test]
    fn draft_04_schema() {
        // draft-04 uses `id` (not `$id`), boolean exclusiveMinimum, no `const`.
        let a = r##"{
          "$schema": "http://json-schema.org/draft-04/schema#",
          "id": "http://example.com/person.json",
          "title": "Person",
          "type": "object",
          "properties": {
            "age": {"type": "integer", "minimum": 0, "exclusiveMinimum": true},
            "name": {"type": "string"}
          },
          "required": ["name", "age"]
        }"##;
        let b = r##"{
          "$schema": "http://json-schema.org/draft-04/schema#",
          "id": "http://example.com/person.json",
          "title": "Human",
          "description": "A person",
          "type": "object",
          "required": ["age", "name"],
          "properties": {
            "name": {"type": "string", "description": "full name"},
            "age": {"exclusiveMinimum": true, "minimum": 0, "type": "integer"}
          }
        }"##;
        assert_same_fp(a, b);
        // draft-04 `id` is not in the default ignore list, so it stays significant.
        assert_diff_fp(a, &a.replace("person.json", "human.json"));
    }

    #[test]
    fn draft_06_schema() {
        let a = r##"{
          "$schema": "http://json-schema.org/draft-06/schema#",
          "$id": "http://example.com/a.json",
          "title": "A",
          "type": "object",
          "properties": {"k": {"const": "fixed"}, "n": {"type": "number", "exclusiveMinimum": 0}},
          "propertyNames": {"pattern": "^[a-z]+$"},
          "contains": {"type": "string"}
        }"##;
        let b = r##"{
          "$schema": "http://json-schema.org/draft-06/schema#",
          "$id": "http://example.com/b.json",
          "type": "object",
          "contains": {"type": "string", "title": "c"},
          "propertyNames": {"pattern": "^[a-z]+$", "examples": ["abc"]},
          "properties": {"n": {"exclusiveMinimum": 0.0, "type": "number"}, "k": {"const": "fixed"}}
        }"##;
        assert_same_fp(a, b);
    }

    #[test]
    fn draft_07_schema() {
        let a = r##"{
          "$schema": "http://json-schema.org/draft-07/schema#",
          "$id": "https://example.com/event.schema.json",
          "title": "Event",
          "$comment": "generated",
          "type": "object",
          "definitions": {"Ts": {"type": "string", "format": "date-time", "description": "when"}},
          "properties": {
            "ts": {"$ref": "#/definitions/Ts"},
            "kind": {"enum": ["created", "deleted", "updated"]},
            "payload": {"if": {"properties": {"kind": {"const": "created"}}}, "then": {"required": ["body"]}, "else": false}
          },
          "readOnly": true,
          "writeOnly": false,
          "additionalProperties": false
        }"##;
        let b = r##"{
          "additionalProperties": false,
          "properties": {
            "payload": {"else": false, "then": {"required": ["body"]}, "if": {"properties": {"kind": {"const": "created"}}}},
            "kind": {"enum": ["updated", "created", "deleted"], "default": "created"},
            "ts": {"$ref": "#/definitions/Ts"}
          },
          "definitions": {"Ts": {"format": "date-time", "type": "string"}},
          "type": "object",
          "$schema": "http://json-schema.org/draft-07/schema#"
        }"##;
        assert_same_fp(a, b);
    }

    #[test]
    fn draft_2019_09_schema() {
        let a = r##"{
          "$schema": "https://json-schema.org/draft/2019-09/schema",
          "$id": "https://example.com/x",
          "title": "X",
          "type": "object",
          "$defs": {"Id": {"type": "string", "minLength": 1}},
          "properties": {"id": {"$ref": "#/$defs/Id"}, "tags": {"type": "array", "items": {"type": "string"}, "minContains": 1, "contains": {"const": "a"}}},
          "dependentRequired": {"id": ["tags"]},
          "dependentSchemas": {"tags": {"properties": {"tagged": {"const": true}}, "deprecated": true}},
          "unevaluatedProperties": false
        }"##;
        let b = r##"{
          "$schema": "https://json-schema.org/draft/2019-09/schema",
          "unevaluatedProperties": false,
          "dependentSchemas": {"tags": {"properties": {"tagged": {"const": true}}}},
          "dependentRequired": {"id": ["tags"]},
          "properties": {"tags": {"contains": {"const": "a"}, "minContains": 1, "items": {"type": "string", "title": "tag"}, "type": "array"}, "id": {"$ref": "#/$defs/Id"}},
          "$defs": {"Id": {"minLength": 1, "type": "string", "examples": ["abc"]}},
          "type": "object"
        }"##;
        assert_same_fp(a, b);
    }

    #[test]
    fn draft_2020_12_schema() {
        let a = r##"{
          "$schema": "https://json-schema.org/draft/2020-12/schema",
          "$id": "https://example.com/coords",
          "title": "Coordinates",
          "type": "array",
          "prefixItems": [{"type": "number", "title": "lat"}, {"type": "number", "title": "lng"}],
          "items": false,
          "unevaluatedItems": false,
          "$defs": {"Unused": {"type": "null", "deprecated": true}}
        }"##;
        let b = r##"{
          "$defs": {"Unused": {"type": "null"}},
          "items": false,
          "unevaluatedItems": false,
          "prefixItems": [{"type": "number"}, {"type": "number"}],
          "type": "array",
          "$schema": "https://json-schema.org/draft/2020-12/schema"
        }"##;
        assert_same_fp(a, b);
    }

    // ------------------------------------------------------------------
    // Realistic schemas.
    // ------------------------------------------------------------------

    #[test]
    fn confluent_style_event_envelope() {
        let a = r##"{
          "$schema": "http://json-schema.org/draft-07/schema#",
          "$id": "https://kannika.io/schemas/order-created-value.json",
          "title": "OrderCreated",
          "description": "Emitted when an order is created",
          "type": "object",
          "additionalProperties": false,
          "required": ["eventId", "eventType", "occurredAt", "payload"],
          "properties": {
            "eventId": {"type": "string", "format": "uuid", "description": "Unique id"},
            "eventType": {"const": "OrderCreated"},
            "occurredAt": {"type": "string", "format": "date-time"},
            "payload": {"$ref": "#/definitions/Order"},
            "metadata": {"type": "object", "additionalProperties": {"type": "string"}, "default": {}}
          },
          "definitions": {
            "Money": {"type": "object", "properties": {"amount": {"type": "number", "multipleOf": 0.01}, "currency": {"type": "string", "pattern": "^[A-Z]{3}$"}}, "required": ["amount", "currency"]},
            "Line": {"type": "object", "properties": {"sku": {"type": "string"}, "qty": {"type": "integer", "minimum": 1}, "price": {"$ref": "#/definitions/Money"}}, "required": ["sku", "qty", "price"]},
            "Order": {"type": "object", "properties": {"id": {"type": "string"}, "lines": {"type": "array", "items": {"$ref": "#/definitions/Line"}, "minItems": 1}, "total": {"$ref": "#/definitions/Money"}, "status": {"enum": ["NEW", "PAID", "SHIPPED"]}}, "required": ["id", "lines", "total"]}
          }
        }"##;
        // Same structure, different app id/title/descriptions, reordered, no defaults.
        let b = r##"{
          "$schema": "http://json-schema.org/draft-07/schema#",
          "$id": "https://other.example/schemas/order-created-value.json",
          "title": "app42.OrderCreated",
          "type": "object",
          "definitions": {
            "Order": {"required": ["total", "lines", "id"], "properties": {"status": {"enum": ["SHIPPED", "NEW", "PAID"]}, "total": {"$ref": "#/definitions/Money"}, "lines": {"minItems": 1, "items": {"$ref": "#/definitions/Line"}, "type": "array"}, "id": {"type": "string"}}, "type": "object"},
            "Line": {"required": ["price", "qty", "sku"], "properties": {"price": {"$ref": "#/definitions/Money"}, "qty": {"minimum": 1.0, "type": "integer"}, "sku": {"type": "string"}}, "type": "object"},
            "Money": {"required": ["currency", "amount"], "properties": {"currency": {"pattern": "^[A-Z]{3}$", "type": "string"}, "amount": {"multipleOf": 0.01, "type": "number"}}, "type": "object"}
          },
          "properties": {
            "metadata": {"additionalProperties": {"type": "string"}, "type": "object"},
            "payload": {"$ref": "#/definitions/Order"},
            "occurredAt": {"format": "date-time", "type": "string"},
            "eventType": {"const": "OrderCreated"},
            "eventId": {"format": "uuid", "type": "string"}
          },
          "required": ["payload", "occurredAt", "eventType", "eventId"],
          "additionalProperties": false
        }"##;
        assert_same_fp(a, b);

        // One structural change deep inside a definition breaks the match.
        let c = b.replace(
            r##""qty": {"minimum": 1.0, "type": "integer"}"##,
            r##""qty": {"minimum": 0, "type": "integer"}"##,
        );
        assert_diff_fp(a, &c);
    }

    #[test]
    fn address_schema_with_pattern_properties_and_nested_arrays() {
        let a = r##"{
          "type": "object",
          "title": "Address",
          "properties": {
            "lines": {"type": "array", "items": {"type": "array", "items": {"type": "string"}, "maxItems": 2}},
            "country": {"type": "string", "enum": ["BE", "NL", "DE"]}
          },
          "patternProperties": {"^x-": {"type": "string", "description": "extension"}},
          "propertyNames": {"maxLength": 32},
          "required": ["country"]
        }"##;
        let b = r##"{
          "required": ["country"],
          "propertyNames": {"maxLength": 32},
          "patternProperties": {"^x-": {"type": "string"}},
          "properties": {
            "country": {"enum": ["DE", "BE", "NL"], "type": "string"},
            "lines": {"items": {"maxItems": 2, "items": {"type": "string"}, "type": "array"}, "type": "array"}
          },
          "type": "object"
        }"##;
        assert_same_fp(a, b);
        assert_diff_fp(a, &b.replace(r##""maxItems": 2"##, r##""maxItems": 3"##));
    }

    #[test]
    fn default_opts_contain_documented_keywords() {
        let opts = JsonCanonicalOpts::default();
        for keyword in DEFAULT_IGNORED_KEYWORDS {
            assert!(opts.ignored_keywords.contains(*keyword));
        }
        assert!(!opts.ignored_keywords.contains("$schema"));
        assert!(!opts.ignored_keywords.contains("type"));
    }
}
