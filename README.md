
# SAME — Schema Automated Mapping Engine by Kannika.io

> SAME is an open-source CLI tool that automatically maps schemas between different Schema Registry instances.  
> Developed and maintained by [Kannika.io](https://kannika.io) — the Kafka reliability platform.

SAME eliminates the manual work of migrating or syncing Avro schemas across registries — whether you're consolidating environments, migrating to Confluent Cloud, or keeping staging in sync with production.

## What is Kannika.io?

[Kannika.io](https://kannika.io) builds tools for Kafka observability, reliability, and schema management. SAME is our open-source CLI for teams managing schemas across multiple Schema Registry environments.

---

## Features

- **Automatic schema mapping** — detect and map equivalent schemas between two registries
- **Conflict resolution strategies** — choose from `strict`, `pick-first`, `pick-lowest-id`, or `pick-highest-id`
- **Offline mode** — work with cached schemas without a live registry connection
- **Force-update support** — overwrite existing mappings when schemas evolve
- **CI/CD-friendly** — drive everything via YAML config files, no interactive prompts needed
- **Secure credential storage** — uses the OS keyring (no plaintext secrets in config)
- **Cross-platform** — Linux, macOS, and Windows supported

## Protocol Support

| Format | Status |
|---|---|
| Avro | ✅ Fully supported |
| JSON Schema | 🚧 Planned |
| Protocol Buffers | 🚧 Planned |


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

Running this command with Docker can be done as follows,
with the current working directory mounted to `/usr/var/same`:

```sh
$ docker run \
  -v .:/usr/var/same \
  quay.io/kannika/same:0.5.0 map \
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

- Avro

These are ignored for now:

- JSON Schema
- Protocol Buffers

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
