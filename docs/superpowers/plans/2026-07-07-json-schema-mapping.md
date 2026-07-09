# JSON Schema Mapping Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make JSON Schema subjects participate in SAME's fingerprint-based schema mapping instead of being silently ignored.

**Architecture:** Mirror the existing Avro fingerprint path. For a JSON subject, parse its schema string to a `serde_json::Value`, reduce it to a deterministic canonical string via `jsonschema::canonical::json::to_string` (lexicographic key order, integer-valued floats normalized, compact), then take a Rabin fingerprint over that string's bytes. Index and match JSON subjects exactly as Avro subjects are.

**Tech Stack:** Rust 2024, `apache-avro` (Rabin via the `digest` trait), `jsonschema` (canonical JSON serializer), `serde_json`.

## Global Constraints

- Rust edition: `2024` (from `Cargo.toml`).
- `jsonschema` dependency pinned to `"0.46"` — `canonical::json::to_string` exists since 0.44.0; latest is 0.46.10.
- `digest` dependency pinned to `"0.10"` — must match the `digest 0.10.7` that `apache-avro 0.21` already resolves, so `use digest::Digest` refers to the same trait `Rabin` implements.
- Rabin fingerprint output is **8 bytes, little-endian**; `Display`/`Debug` render it as lowercase hex, matching `AvroFingerprint`.
- Keep `index_avro` and `index_json` as **separate** methods even though their v1 bodies match (dedicated home for later JSON-specific logic).
- v1 non-goals — do NOT implement: annotation stripping (`title`/`description`/etc.), `$ref` content folding, Protobuf. A `description`-only edit is expected to change the fingerprint (safe false-miss).

---

### Task 1: JSON canonical-form Rabin fingerprint in `fingerprint.rs`

**Files:**
- Modify: `Cargo.toml` (dependencies)
- Modify: `src/mapping/fingerprint.rs`

**Interfaces:**
- Consumes: `apache_avro::rabin::Rabin`, `jsonschema::canonical::json::to_string`, `digest::Digest`.
- Produces:
  - `Fingerprint::Json(JsonFingerprint)` — the `Json` variant now carries a value.
  - `struct JsonFingerprint { pub bytes: Vec<u8> }` with `Display`/`Debug` (lowercase hex) and `Deref<Target=[u8]>`.
  - `impl JsonFingerprint { pub fn from_schema_str(schema: &str) -> Result<JsonFingerprint, FingerprintError> }`.
  - `FingerprintError::InvalidJsonSchema(serde_json::Error)`.
  - `Fingerprint::get_value_opt()` returns `Some(hex)` for the `Json` arm.

- [ ] **Step 1: Add dependencies to `Cargo.toml`**

In the `[dependencies]` section of `Cargo.toml`, add below the existing `apache-avro = "0.21.0"` line:

```toml
jsonschema = "0.46"
digest = "0.10"
```

- [ ] **Step 2: Write the failing tests**

Add these tests inside the existing `#[cfg(test)] mod tests` block at the bottom of `src/mapping/fingerprint.rs`. Update its `use` line to also import `JsonFingerprint`:

```rust
    use crate::mapping::fingerprint::JsonFingerprint;

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
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib mapping::fingerprint 2>&1 | tail -20`
Expected: FAIL — compile errors ("cannot find type `JsonFingerprint`", `from_schema_str` not found).

- [ ] **Step 4: Add imports and the error variant**

At the top of `src/mapping/fingerprint.rs`, below the existing `use apache_avro::...` lines, add:

```rust
use digest::Digest;
```

Extend the `FingerprintError` enum:

```rust
#[derive(Debug, thiserror::Error)]
pub enum FingerprintError {
    #[error(transparent)]
    InvalidAvroSchema(#[from] apache_avro::Error),

    #[error("Invalid JSON schema: {0}")]
    InvalidJsonSchema(#[from] serde_json::Error),
}
```

- [ ] **Step 5: Give `Fingerprint::Json` a value and update `get_value_opt`**

Change the enum variant:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Fingerprint {
    Avro(AvroFingerprint),
    Protobuf,
    Json(JsonFingerprint),
}
```

Update `get_value_opt`:

```rust
    pub fn get_value_opt(&self) -> Option<String> {
        match self {
            Fingerprint::Avro(fingerprint) => Some(fingerprint.to_string()),
            Fingerprint::Json(fingerprint) => Some(fingerprint.to_string()),
            Fingerprint::Protobuf => None,
        }
    }
```

- [ ] **Step 6: Add the `JsonFingerprint` type and pipeline**

Add near the `AvroFingerprint` definition in `src/mapping/fingerprint.rs`:

```rust
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
```

Note: `Rabin::finalize()` returns a `GenericArray<u8, 8>` which has `.to_vec()`. `Rabin` is already imported (`use apache_avro::rabin::Rabin;`). `std::io::Error::other` maps the `jsonschema` canonical error into a `serde_json::Error` so it flows through `FingerprintError::InvalidJsonSchema` without a second error variant.

- [ ] **Step 7: Implement the `SchemaType::Json` arm in `to_fingerprint`**

In `impl ToFingerprint for SubjectFingerPrintBuilder`, replace `SchemaType::Json => Ok(Fingerprint::Json),` with:

```rust
            SchemaType::Json => {
                let fingerprint = JsonFingerprint::from_schema_str(self.subject.schema.as_str())?;
                Ok(Fingerprint::Json(fingerprint))
            }
```

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test --lib mapping::fingerprint 2>&1 | tail -20`
Expected: PASS — all `fingerprint` tests, including the four new JSON tests, pass. `cargo build` succeeds (no other `Fingerprint::Json` match sites exist outside this file).

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml Cargo.lock src/mapping/fingerprint.rs
git commit -m "feat: fingerprint JSON schemas via canonical JSON + Rabin"
```

---

### Task 2: Index and match JSON subjects in `index.rs`

**Files:**
- Modify: `src/mapping/index.rs`

**Interfaces:**
- Consumes: `Fingerprint::Json` from Task 1 (via `FingerprintedSchema::from_subject`, which is already schema-type-generic).
- Produces: `SchemaRegistryIndex::index_json` — indexes a JSON subject so `find_by_fingerprint` returns it.

- [ ] **Step 1: Rewrite the failing test**

In the `#[cfg(test)] mod tests` block of `src/mapping/index.rs`, replace the existing `index_json_schema_is_ignored` test with:

```rust
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
```

The existing `jacksonfruit_subject()` and `jackfruit_schema()` helpers already produce a `SchemaType::Json` subject with a valid JSON body — reuse them unchanged.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib mapping::index::tests::find_json_schema_by_fingerprint 2>&1 | tail -20`
Expected: FAIL — `find_by_fingerprint` returns `Candidates::None` because the `SchemaType::Json` arm of `index()` still returns `Ok(())` without inserting.

- [ ] **Step 3: Index JSON subjects**

In `src/mapping/index.rs`, update the `index` method's match arm and add an `index_json` method mirroring `index_avro`:

```rust
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
```

Add directly below `index_avro`:

```rust
    fn index_json(
        &mut self,
        schema_subject: &Subject,
        resolver: &impl ResolveSchemaReferences,
    ) -> Result<(), SchemaRegistryIndexError> {
        let schema = FingerprintedSchema::from_subject(schema_subject.clone(), resolver)?;

        self.insert(schema);

        Ok(())
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib mapping::index 2>&1 | tail -20`
Expected: PASS — `find_json_schema_by_fingerprint` passes; `index_protobuf_schema_is_ignored` still passes; no other index tests regress.

- [ ] **Step 5: Run the full test suite**

Run: `cargo test 2>&1 | tail -25`
Expected: PASS — entire suite green, confirming JSON subjects now flow through `map_schemas` (which is fingerprint-generic and needs no change).

- [ ] **Step 6: Commit**

```bash
git add src/mapping/index.rs
git commit -m "feat: index and match JSON schema subjects by fingerprint"
```

---

## Self-Review

**Spec coverage:**
- Pipeline (parse → canonical → Rabin) → Task 1, Steps 6–7. ✓
- `Cargo.toml` adds `jsonschema` → Task 1, Step 1. ✓
- `Fingerprint::Json` carries value + `JsonFingerprint` + `get_value_opt` + error variant → Task 1, Steps 4–6. ✓
- `to_fingerprint` `SchemaType::Json` arm → Task 1, Step 7. ✓
- `index.rs` indexes JSON via separate `index_json` → Task 2, Step 3. ✓
- `mapping/mod.rs` unchanged (fingerprint-generic) → confirmed, no task needed. ✓
- Rabin reused → Task 1, Step 6. ✓
- Tests (display, same, different, ignored→found flip) → Task 1 Step 2, Task 2 Step 1. ✓
- v1 non-goal documented behavior (description edit ⇒ different fp) → Task 1 Step 2 guard test. ✓
- README table update: noted in spec as a separate docs edit, not part of the core change — intentionally out of scope for this plan.

**Placeholder scan:** No TBD/TODO; every code and test step shows complete code. ✓

**Type consistency:** `JsonFingerprint`, `from_schema_str`, `Fingerprint::Json(JsonFingerprint)`, `FingerprintError::InvalidJsonSchema`, `index_json` are named identically everywhere they appear across both tasks. ✓
