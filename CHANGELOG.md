# Changelog
All notable changes to this project will be documented in this file.

## [0.6.0] - 2026-09-09

### What's Changed
* feat: map JSON Schema subjects by canonical fingerprint by @tomverelst in https://github.com/kannika-io/same/pull/1
* fix: ignore JSON Schema annotations and fold $ref when fingerprinting by @tomverelst in https://github.com/kannika-io/same/pull/2
* chore: apply rustfmt 2024 style edition and fix clippy warnings by @tomverelst in https://github.com/kannika-io/same/pull/6
* ci: build multi-arch images (amd64, arm64) and publish to GHCR and Quay by @tomverelst in https://github.com/kannika-io/same/pull/4
* chore(deps): upgrade outdated dependencies by @tomverelst in https://github.com/kannika-io/same/pull/7

### New Contributors
* @tomverelst made their first contribution in https://github.com/kannika-io/same/pull/1

**Full Changelog**: https://github.com/kannika-io/same/compare/0.5.0...0.6.0

## [0.5.0] - 2026-03-19


### 🐛 Bug Fixes

- [Iterate all schema versions in fingerprint index](https://github.com/cymo-eu/same/commit/3bce86959ba187437b924ffcee997bdd495bff03)

- [Use containerized registry, add assertions, rename to 01xx prefix](https://github.com/cymo-eu/same/commit/69ee6306025e0f30b2ba692a84f9e00357575028)


### 🧪 Testing

- [Add cache directory override for test isolation](https://github.com/cymo-eu/same/commit/081b442fd972e9a94c64a804f345a0ed729f8c33)

- [Validate yaml output](https://github.com/cymo-eu/same/commit/135f29835430b8bee84073f0c202b2f7d0797e49)


### 🧹 Miscellaneous Tasks

- [🔧 update README](https://github.com/cymo-eu/same/commit/29237b227a4ab74d41bb77251a6985ec725844a7)

- [Upgrade apache-avro from 0.17.0 to 0.21.0](https://github.com/cymo-eu/same/commit/211abcca4bb0072abf7f60406014fea7064c2cd6)

- [Pin Rust 1.94 toolchain](https://github.com/cymo-eu/same/commit/e4e38352962e410f53d58656a15187377ef6ccd3)

- [Upgrade to Rust edition 2024](https://github.com/cymo-eu/same/commit/0604a07a3f7444686a06c5c1a16f43908bb3267f)

- [Upgrade Dockerfile Rust to 1.94](https://github.com/cymo-eu/same/commit/f342f5c76ee88fee845e159a5efae20d698a047c)

- [Upgrade Alpine to 3.23](https://github.com/cymo-eu/same/commit/161a6fb52dfb7454505fde2f46f797881e607fb2)

- [Update lockfile (anyhow, tokio, tracing, tracing-subscriber)](https://github.com/cymo-eu/same/commit/a06973984d054aaf7cb8bfebdc4ef3e15c5d7ae9)

- [Upgrade reqwest to 0.13.2, reqwest-middleware to 0.5.1, reqwest-tracing to 0.7.0](https://github.com/cymo-eu/same/commit/7ce661de598cfb523ab12c65919b7caff650fd92)

- [Upgrade keyring to 3.6.3](https://github.com/cymo-eu/same/commit/f4172c07a2569e0525bdec260d90569fe40e2d88)

- [Upgrade tokio-util to 0.7.18](https://github.com/cymo-eu/same/commit/a0960ed77e4e04ed510ac91f53ba8e261d43c401)

- [Upgrade clap to 4.6.0](https://github.com/cymo-eu/same/commit/5a0babd5979a9a0e4879f6d1339a59d7678c1096)

- [Upgrade serde to 1.0.228](https://github.com/cymo-eu/same/commit/86edc5c4c96f478772b8233f126a9d1163f302e8)

- [Upgrade serde_json to 1.0.149](https://github.com/cymo-eu/same/commit/617895a3d5064183d3e75e8c9ebb14a13642077d)

- [Upgrade strum to 0.28.0, strum_macros to 0.28.0](https://github.com/cymo-eu/same/commit/d14b602228f8fe4e4004f9a8250b992d6ea9cfa9)

- [Upgrade thiserror to 2.0.18](https://github.com/cymo-eu/same/commit/ad7af90420c691f17dc86509715ea725e60ad3b5)

- [Upgrade dirs to 6.0.0](https://github.com/cymo-eu/same/commit/025dea7a03cc33d799a577839cdf05cfaf8f3a84)

- [Upgrade tempfile to 3.27.0](https://github.com/cymo-eu/same/commit/64328bf6beb60f4437f6a08ea5e7bedb9e256d9d)

- [Upgrade dialoguer to 0.12.0](https://github.com/cymo-eu/same/commit/0fa4f03758760d3b869745dac9aa4af642a31b4c)

- [Upgrade indicatif to 0.18.4](https://github.com/cymo-eu/same/commit/8ef9a67c93a78ae180594fbd98ea9634339f0f73)

- [Upgrade rand to 0.10.0](https://github.com/cymo-eu/same/commit/60e4ec2798c3572d7e2b77f318b3bed46d1ff502)

- [Upgrade url to 2.5.8](https://github.com/cymo-eu/same/commit/2bed57edf84f3ede22a57a1b37db706b64bbb4a8)

- [Upgrade multimap to 0.10.1](https://github.com/cymo-eu/same/commit/3205c875aa4b508500356978ef9ff3ae4965bd9c)

- [Replace deprecated serde_yaml with serde_yml 0.0.12](https://github.com/cymo-eu/same/commit/183887344c31f970cb0f10aaa7a6f450ee8bd938)

- [Add release workflow to publish Docker images to GHCR (#8)](https://github.com/cymo-eu/same/commit/072b107f59dc024d7b2310ebba233d573289fc1b)

- [Bump version to 0.5.0](https://github.com/cymo-eu/same/commit/83065dc752648506ce23b988661a4593cbb38495)

