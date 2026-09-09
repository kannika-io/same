
# SAME: Schema Automated Mapping Engine by Kannika.io

> SAME is an open-source CLI tool that automatically maps schemas between different Schema Registry instances.  
> Developed and maintained by [Kannika.io](https://kannika.io), the Kafka reliability platform.

SAME eliminates the manual work of migrating or syncing Avro schemas across registries, whether you're consolidating environments, migrating to Confluent Cloud, or keeping staging in sync with production.

## What is Kannika.io?

[Kannika.io](https://kannika.io) builds tools for Kafka observability, reliability, and schema management. SAME is our open-source CLI for teams managing schemas across multiple Schema Registry environments.

---

## Features

- **Automatic schema mapping**: detect and map equivalent schemas between two registries
- **Conflict resolution strategies**: choose from `strict`, `pick-first`, `pick-lowest-id`, or `pick-highest-id`
- **Offline mode**: work with cached schemas without a live registry connection
- **Force-update support**: overwrite existing mappings when schemas evolve
- **CI/CD-friendly**: drive everything via YAML config files, no interactive prompts needed
- **Secure credential storage**: uses the OS keyring (no plaintext secrets in config)
- **Cross-platform**: Linux, macOS, and Windows supported

## Protocol Support

| Format | Status |
|---|---|
| Avro | ✅ Fully supported |
| JSON Schema | ✅ Supported¹ |
| Protocol Buffers | 🚧 Planned |

¹ JSON Schema is matched by a structural fingerprint: annotations such as `title` and `description` are ignored, and `$ref` references to other subjects are matched on the referenced content. See [JSON Schema](#json-schema).


## 👩‍💻 Usage

- [Configuring schema registries](#configuring-schema-registries)
- [Generating a mapping](#generating-a-mapping)

### Configuring schema registries

There are two ways to configure schema registries:
- Using the `same add` command (interactive).
- Using a configuration file (non-interactive). See [Mapping registries using a file](#mapping-registries-using-a-file) below.

You can configure a schema registry using the `same add` command.

```
$ same add
Enter the url for the schema registry: https://somewhere.europe-west3.gcp.confluent.cloud
Select the authentication method: Basic Auth
Enter the username: DEADBEEFCAFEBABE
Enter the password: [hidden]
Enter a name for the context: prod
```

### Generating a mapping

Generate a mapping between two schema registries:

```
$ same map --from [SOURCE_CTX] --to [TARGET_CTX] -o mapping
```

Options:

- `--from`: The name of the context to map from (required).
- `--to`: The name of the context to map to (required).
- `-o`, `--output`: The output file to write the mapping to (optional).
- `-U`, `--force-update`: Force update the schemas in the cache (optional, default false).
- `--registries`: The config file containing the schema registries (optional).
- `--offline`: Run in offline mode, do not download schemas from the registries (optional, default false).
- `--ignore-indexing-errors`: Ignore indexing errors (optional, default false).
- `--on-conflict=[strict|pick-first|pick-lowest-id|pick-highest-id]`: How to handle conflicts (optional, default `strict`). See [Conflict resolution](#conflict-resolution) below.
- `--json-ignore-keyword=<KEYWORD,...>`: JSON Schema keywords to ignore when fingerprinting (optional). Passing the flag replaces the default list. See [JSON Schema](#json-schema) below.

### Mapping registries using a file

It is possible to pass the schema registries as a file.
This is useful when you are not able to configure the schema registries using the `same add` command,
e.g. in a CI/CD pipeline.

Example:

```yaml
registries:
- name: source
  url: https://aaaa-1234.europe-west3.gcp.confluent.cloud
  username: <API KEY> # Optional
  password: <API SECRET> # Optional
- name: target
  url: https://bbbb-4567.europe-west3.gcp.confluent.cloud
  username: <API KEY> # Optional
  password: <API SECRET> # Optional
```

This can then be used as follows:

```sh
$ same map \
  --from source \
  --to target \
  -o mapping.yaml  \
  --registries /path/to/registries.yaml
```

Docker images are published to `quay.io/kannika/same` and `ghcr.io/kannika-io/same`
for both `linux/amd64` and `linux/arm64`.
Running this command with Docker can be done as follows,
with the current working directory mounted to `/usr/var/same`:

```sh
$ docker run \
  -v .:/usr/var/same \
  quay.io/kannika/same:0.6.0 map \
  --from=source \
  --to=target \
  --ignore-indexing-errors \
  --on-conflict=pick-first \
  -o /usr/var/same/mapping.yaml \
  --registries /usr/var/same/registries.yaml
```

## 🔎 Where are my configurations and mappings stored?

Credentials are stored in the platform's specific secure storage.
We use [keyring](https://lib.rs/crates/keyring) for this purpose.

We use [dirs](https://lib.rs/crates/dirs) for determining the location of configuration and cache files.

Configuration is stored in the following locations:

- Linux: `$XDG_CONFIG_HOME/io.kannika.same/config`
- macOs: `$HOME/Library/Application Support/io.kannika.same/config`
- Windows: `{FOLDERID_RoamingAppData}\io.kannika.same\config`

Schemas are cached locally to avoid unnecessary network requests in the following locations:

- Linux: `$XDG_CACHE_HOME/io.kannika.same` or `$HOME/.cache/io.kannika.same`
- macOs: `$HOME/Library/Application Support/io.kannika.same`
- Windows: `{FOLDERID_RoamingAppData}\io.kannika.same`

## 💾 Supported Protocols

Following protocols are supported:

- [Avro](#avro)
- [JSON Schema](#json-schema)

These are ignored for now:

- Protocol Buffers

### Avro

Avro schemas are matched by the Rabin fingerprint of their Parsing Canonical Form.
Attributes that do not affect parsing, such as `doc`, `default`, `aliases` and custom attributes, are not part of the canonical form, so schemas that differ only in those map to each other.
Schemas that reference other subjects are parsed together with the referenced schemas, so named types from other subjects resolve.

### JSON Schema

JSON schemas are matched by a structural fingerprint.
The schema is reduced to a canonical form and hashed, so two schemas map to each other when they accept the same instances, regardless of how they are written.
The canonical form:

- drops annotation keywords: `title`, `description`, `$comment`, `examples`, `default`, `readOnly`, `writeOnly`, `deprecated` and `$id`;
- keeps `$schema`, because the draft changes the meaning of other keywords;
- folds `$ref` references to other subjects (as registered in the subject's `references`) into the referenced schema's canonical content, transitively, so the reference name and the referenced subject's name do not matter.
  A `$ref` is matched to a reference by exact name, or by resolving both against the enclosing `$id` (some registries, such as Redpanda, store relative `$ref` values as absolute URIs);
- treats `required`, `enum` and `type` arrays as sets (order does not matter);
- sorts object keys, normalizes numbers (`1.0` equals `1`) and strips whitespace.

The keyword walk is schema-aware: a *property* named `title` and a `title` value inside `enum` or `const` are part of the structure and are kept.
Local references (`#/definitions/...`, `#/$defs/...`) are kept as-is.
Unknown or custom keywords (for example `x-owner`) are part of the fingerprint.
The list of ignored keywords can be changed with `--json-ignore-keyword`.
The flag replaces the default list, so include the defaults you want to keep:

```sh
$ same map --from source --to target \
  --json-ignore-keyword title,description,\$comment,examples,default,readOnly,writeOnly,deprecated,\$id \
  --json-ignore-keyword x-owner,x-kafka-topic
```

Unparseable JSON surfaces as an indexing error; use `--ignore-indexing-errors` to skip such subjects.

## Conflict resolution

When a conflict is detected during mapping,
you can pass the `--on-conflict` flag to specify how to handle it.
Available strategies are:

- `strict`: The mapping process will log a warnig and report the conflict as a missing mapping. This is the default behavior.
- `pick-first`: Pick the first schema encountered and ignore the rest.
- `pick-lowest-id`: Pick the schema with the lowest ID.
- `pick-highest-id`: Pick the schema with the highest ID.

## 👀 Debugging

Append `--verbose` to the `same` command (before the subcommand).
Adjust the `RUST_LOG` environment variable to `info`, `debug` or `trace`.

```
$ RUST_LOG=debug same --verbose [COMMAND]
```
