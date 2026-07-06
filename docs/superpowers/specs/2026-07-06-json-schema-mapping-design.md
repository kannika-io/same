# JSON Schema Mapping — v1 (basics)

**Date:** 2026-07-06
**Status:** Approved design
**Scope:** Make JSON Schema subjects participate in schema mapping, using the
`jsonschema` crate's canonical JSON serialization as the canonical form. This is
the minimal, symmetric-with-Avro implementation. Annotation stripping and `$ref`
content folding are explicit non-goals for v1.

## Background

SAME maps schemas between two Schema Registry instances by **fingerprint**: it
indexes every target-registry schema by a canonical fingerprint, then for each
source schema looks up the same fingerprint in the target. A match means "these
are the same logical schema — rewrite the ID/reference."

Today only Avro is implemented:

- **Avro** (`src/mapping/fingerprint.rs`): resolves references, folds them in via
  `AvroSchema::parse_list`, reduces to Avro **Parsing Canonical Form (PCF)**, and
  takes a **Rabin** fingerprint. PCF strips `doc`, `default`, `aliases`, and
  normalizes field order — this is why the existing tests show reordering, `doc`
  strings, and custom root properties all produce the same fingerprint.
- **JSON Schema & Protobuf**: stubbed. `Fingerprint::Json` is a valueless
  sentinel; `SchemaRegistryIndex::index()` returns `Ok(())` without inserting, so
  these subjects are silently ignored during mapping.

**Key fact driving the design:** Avro has a standardized canonical form (PCF);
JSON Schema does not. The `jsonschema` crate exposes
`jsonschema::canonical::json::to_string(&Value)`, a byte-level canonical JSON
serializer (RFC 8785 JCS-like). Its documented rules are exactly:

- Object keys emitted in lexicographic order.
- Integer-valued floats emitted as integers (`1.0` → `1`).
- Compact output (no whitespace).

It does **not** strip annotation keywords, does **not** treat `required` as an
unordered set, and does **not** resolve `$ref`. It is a stable *serialization*
primitive, not a semantic normalizer. v1 uses it as-is.

## Safety principle

Mapping errors are asymmetric:

- A **false miss** (a mappable schema reported as unmapped) is safe — the schema
  is surfaced to a human who handles it.
- A **false match** (fingerprint collision between genuinely different schemas)
  writes the wrong target ID — potential data corruption.

v1 is therefore deliberately conservative: it errs toward false misses. Anything
that could collapse distinct schemas together is deferred.

## Design

### Pipeline

Mirror the Avro path, minus reference folding and annotation stripping:

```
subject.schema (String)
  → serde_json::from_str::<serde_json::Value>          // parse
  → jsonschema::canonical::json::to_string(&value)     // canonical form
  → Rabin fingerprint over the canonical string bytes  // → JsonFingerprint
```

### Changes

All changes are small and localized; the pipeline reuses the shapes the Avro path
already established.

**1. `Cargo.toml`**
Add the `jsonschema` dependency. Only `canonical::json::to_string` is used.

**2. `src/mapping/fingerprint.rs`**
- `Fingerprint::Json` gains a value: `Json(JsonFingerprint)`.
- Add `JsonFingerprint { bytes: Vec<u8> }` with hex `Display`/`Debug`/`Deref`,
  structurally identical to the existing `AvroFingerprint`.
- `Fingerprint::get_value_opt()` returns `Some(hex)` for the `Json` arm.
- Add `FingerprintError::InvalidJsonSchema(#[from] serde_json::Error)`.
- In `ToFingerprint::to_fingerprint()`, replace
  `SchemaType::Json => Ok(Fingerprint::Json)` with an arm that runs the pipeline:
  parse the subject schema string to `Value`, canonicalize via
  `jsonschema::canonical::json::to_string`, Rabin-hash the resulting string bytes,
  and return `Fingerprint::Json(JsonFingerprint { .. })`.

**3. `src/mapping/index.rs`**
- `SchemaRegistryIndex::index()`: the `SchemaType::Json => Ok(())` arm becomes a
  call to a new `index_json` method, which does the same
  `FingerprintedSchema::from_subject` + `insert` that `index_avro` does. Keep
  `index_avro` and `index_json` as **separate** methods even though their v1
  bodies match — this gives the deferred JSON-specific logic (`$ref` folding,
  annotation stripping) a dedicated home later without disturbing the Avro path.

**4. `src/mapping/mod.rs`**
No change. Matching logic (`find_by_fingerprint`, conflict resolution) is
fingerprint-generic and works unchanged once `Json` fingerprints carry values.

### Hash function

Reuse **Rabin** (`apache_avro::rabin::Rabin`) over the canonical string bytes.
This keeps every `Fingerprint` variant's `Display` uniform (16-char hex) and adds
no new dependency. The Rabin fingerprint here is computed over an arbitrary byte
string (the canonical JSON), not over an Avro schema — this is fine, Rabin is a
general-purpose CRC-64-AVRO-style fingerprint.

## Non-goals (v1)

Deferred, and documented as known limitations:

- **No annotation stripping.** A schema re-registered with an edited `title` /
  `description` / `$comment` / `default` / `examples` produces a different
  canonical string, hence a different fingerprint, hence a reported miss. This is
  *less* aggressive than Avro (whose PCF drops `doc`/`default`). It is safe
  (false-miss, not false-match) and can be added later as a pre-canonicalization
  tree-walk that deletes the annotation vocabulary.
- **No `$ref` content folding.** The `$ref` value (a URL/string) is fingerprinted
  verbatim. Reference-free schemas fingerprint perfectly; schemas with references
  match iff their `$ref` strings match. Content-blindness across references is a
  documented v1 limitation — a follow-up can inline referenced subject content via
  the existing `ResolveSchemaReferences` resolver, matching Avro's reference
  folding.
- **Protobuf** stays stubbed (`Fingerprint::Protobuf`, `index()` returns `Ok(())`).

## Testing

Add unit tests to `fingerprint.rs` and `index.rs` mirroring the existing Avro
tests:

- `json_display_should_print_fingerprint` — canonical form of a known JSON schema
  produces a stable hex string (pin the value).
- `same_json_schemas_should_have_same_fingerprint` — two schemas differing only in
  key order / whitespace / `1.0` vs `1` fingerprint identically (this is what the
  canonical serializer guarantees).
- `different_json_schemas_should_have_different_fingerprint` — a type change
  (`string` → `integer`) changes the fingerprint.
- Flip `index_json_schema_is_ignored` → `find_json_schema_by_fingerprint`: a JSON
  subject is now indexed and found (parallel to `find_avro_schema_by_fingerprint`).
- Add a note/test asserting the v1 non-goal: a `description`-only edit currently
  yields a *different* fingerprint (documents intended behavior, guards against
  accidental change).

## Files touched

- `Cargo.toml` — add `jsonschema`.
- `src/mapping/fingerprint.rs` — `JsonFingerprint`, `Json` arm, error variant.
- `src/mapping/index.rs` — index JSON subjects via a separate `index_json` method.
- (README "Protocol Support" / "Supported Protocols" tables can be updated to move
  JSON Schema from Planned/ignored to supported once implemented — separate docs
  edit, not part of the core change.)
