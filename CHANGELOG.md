# Changelog

All notable changes to this project will be documented in this file.

## [0.6.0] - 2026-09-09

### 🌟 Features

- [Map JSON Schema subjects by canonical fingerprint ([#1](https://github.com/kannika-io/same/pull/1))](https://github.com/kannika-io/same/commit/24c7448147fe58878c2dabe82bf30cd777633a90)

### 🐛 Bug Fixes

- [Ignore JSON Schema annotations and fold $ref when fingerprinting ([#2](https://github.com/kannika-io/same/pull/2))](https://github.com/kannika-io/same/commit/3297af0f7b3de64f3775357bad5bba2a6672d99d)

### 📚 Documentation

- [Update Docker example to 0.5.0](https://github.com/kannika-io/same/commit/5d924bdaabb77b6233c33890952b09e32a5293b1)

### 🧹 Miscellaneous Tasks

- [Add BUSL-1.1 license](https://github.com/kannika-io/same/commit/77b705440dcdfa0489d4a94b3cc63757b51ab99e)
- [Apply rustfmt 2024 style edition and fix clippy warnings ([#6](https://github.com/kannika-io/same/pull/6))](https://github.com/kannika-io/same/commit/13cbfaa30e6644f786a92be0658af7fd0e3272cb)
- [Build multi-arch images (amd64, arm64) and publish to GHCR and Quay ([#4](https://github.com/kannika-io/same/pull/4))](https://github.com/kannika-io/same/commit/fa2023438b07ad20bcd35c3f6758bec1a3903285)

### 📦 Dependencies

- [Upgrade anyhow to 1.0.104](https://github.com/kannika-io/same/commit/80a48837280d437689c19809337b98ff4931b45f)
- [Upgrade clap to 4.6.6](https://github.com/kannika-io/same/commit/198207d2fdf62018949e3996d814891586618442)
- [Upgrade indicatif to 0.18.6](https://github.com/kannika-io/same/commit/cd34c4eb9a87b517e06ef0afdb1591517209ffda)
- [Upgrade rand to 0.10.2](https://github.com/kannika-io/same/commit/f693a86724086e5df0e7f2222b9fcdb445d8b5d5)
- [Upgrade reqwest to 0.13.5](https://github.com/kannika-io/same/commit/d191e267367951f9b49ba310233a4dd987612a03)
- [Upgrade reqwest-middleware to 0.5.2](https://github.com/kannika-io/same/commit/45cad68944a8f1e613705a46d3e9aff7838a28a1)
- [Upgrade reqwest-tracing to 0.7.1](https://github.com/kannika-io/same/commit/9685b1d395cc8bc93a16594bace1e32b62cca98b)
- [Upgrade serde to 1.0.229](https://github.com/kannika-io/same/commit/390132e3de5abeb93515e3983819c28c01ed61d6)
- [Upgrade serde_json to 1.0.151](https://github.com/kannika-io/same/commit/a9c29d22a728282c6a0309099b9d18f731a51772)
- [Upgrade thiserror to 2.0.20](https://github.com/kannika-io/same/commit/79bcfe927d963c1d23476ba8d4808ebe1094c377)
- [Upgrade tokio to 1.53.1](https://github.com/kannika-io/same/commit/70fbe58ffc27ed222dfa4d0ba33dc74dee387886)
- [Upgrade tokio-util to 0.7.19](https://github.com/kannika-io/same/commit/1a3b72d9f98e33c4def2777f56c3812e1ac27b7d)
- [Upgrade apache-avro to 0.22.0 and digest to 0.11](https://github.com/kannika-io/same/commit/129e0db55aec2f4f5c7e007d3ad1fc304f471efe)
- [Upgrade dirs to 7.0.0](https://github.com/kannika-io/same/commit/fb93da3b4e1cf5dd0efc85a1b25882e36a6fb23c)
- [Upgrade jsonschema to 0.55](https://github.com/kannika-io/same/commit/a30e0c89c008d50901d9127c984580ddf2ac7d94)
- [Upgrade keyring to 4.2.0](https://github.com/kannika-io/same/commit/44a0f7e7b343ddafe3ad02534c9ff5ecd157ffef)
- [Upgrade testcontainers to 0.28](https://github.com/kannika-io/same/commit/bf8c8de30698fa2d0d6af5657b331e832b5fcebf)

## [0.5.0] - 2026-03-19

### 🐛 Bug Fixes

- [Iterate all schema versions in fingerprint index](https://github.com/kannika-io/same/commit/3bce86959ba187437b924ffcee997bdd495bff03)
- [Use containerized registry, add assertions, rename to 01xx prefix](https://github.com/kannika-io/same/commit/69ee6306025e0f30b2ba692a84f9e00357575028)

### 🧪 Testing

- [Add cache directory override for test isolation](https://github.com/kannika-io/same/commit/081b442fd972e9a94c64a804f345a0ed729f8c33)
- [Validate yaml output](https://github.com/kannika-io/same/commit/135f29835430b8bee84073f0c202b2f7d0797e49)

### 🧹 Miscellaneous Tasks

- [🔧 update README](https://github.com/kannika-io/same/commit/29237b227a4ab74d41bb77251a6985ec725844a7)
- [Upgrade apache-avro from 0.17.0 to 0.21.0](https://github.com/kannika-io/same/commit/211abcca4bb0072abf7f60406014fea7064c2cd6)
- [Pin Rust 1.94 toolchain](https://github.com/kannika-io/same/commit/e4e38352962e410f53d58656a15187377ef6ccd3)
- [Upgrade to Rust edition 2024](https://github.com/kannika-io/same/commit/0604a07a3f7444686a06c5c1a16f43908bb3267f)
- [Upgrade Dockerfile Rust to 1.94](https://github.com/kannika-io/same/commit/f342f5c76ee88fee845e159a5efae20d698a047c)
- [Upgrade Alpine to 3.23](https://github.com/kannika-io/same/commit/161a6fb52dfb7454505fde2f46f797881e607fb2)
- [Add release workflow to publish Docker images to GHCR ([#8](https://github.com/kannika-io/same/pull/8))](https://github.com/kannika-io/same/commit/072b107f59dc024d7b2310ebba233d573289fc1b)

### 📦 Dependencies

- [Update lockfile (anyhow, tokio, tracing, tracing-subscriber)](https://github.com/kannika-io/same/commit/a06973984d054aaf7cb8bfebdc4ef3e15c5d7ae9)
- [Upgrade reqwest to 0.13.2, reqwest-middleware to 0.5.1, reqwest-tracing to 0.7.0](https://github.com/kannika-io/same/commit/7ce661de598cfb523ab12c65919b7caff650fd92)
- [Upgrade keyring to 3.6.3](https://github.com/kannika-io/same/commit/f4172c07a2569e0525bdec260d90569fe40e2d88)
- [Upgrade tokio-util to 0.7.18](https://github.com/kannika-io/same/commit/a0960ed77e4e04ed510ac91f53ba8e261d43c401)
- [Upgrade clap to 4.6.0](https://github.com/kannika-io/same/commit/5a0babd5979a9a0e4879f6d1339a59d7678c1096)
- [Upgrade serde to 1.0.228](https://github.com/kannika-io/same/commit/86edc5c4c96f478772b8233f126a9d1163f302e8)
- [Upgrade serde_json to 1.0.149](https://github.com/kannika-io/same/commit/617895a3d5064183d3e75e8c9ebb14a13642077d)
- [Upgrade strum to 0.28.0, strum_macros to 0.28.0](https://github.com/kannika-io/same/commit/d14b602228f8fe4e4004f9a8250b992d6ea9cfa9)
- [Upgrade thiserror to 2.0.18](https://github.com/kannika-io/same/commit/ad7af90420c691f17dc86509715ea725e60ad3b5)
- [Upgrade dirs to 6.0.0](https://github.com/kannika-io/same/commit/025dea7a03cc33d799a577839cdf05cfaf8f3a84)
- [Upgrade tempfile to 3.27.0](https://github.com/kannika-io/same/commit/64328bf6beb60f4437f6a08ea5e7bedb9e256d9d)
- [Upgrade dialoguer to 0.12.0](https://github.com/kannika-io/same/commit/0fa4f03758760d3b869745dac9aa4af642a31b4c)
- [Upgrade indicatif to 0.18.4](https://github.com/kannika-io/same/commit/8ef9a67c93a78ae180594fbd98ea9634339f0f73)
- [Upgrade rand to 0.10.0](https://github.com/kannika-io/same/commit/60e4ec2798c3572d7e2b77f318b3bed46d1ff502)
- [Upgrade url to 2.5.8](https://github.com/kannika-io/same/commit/2bed57edf84f3ede22a57a1b37db706b64bbb4a8)
- [Upgrade multimap to 0.10.1](https://github.com/kannika-io/same/commit/3205c875aa4b508500356978ef9ff3ae4965bd9c)
- [Replace deprecated serde_yaml with serde_yml 0.0.12](https://github.com/kannika-io/same/commit/183887344c31f970cb0f10aaa7a6f450ee8bd938)

## [0.4.0] - 2025-10-09

### 🌟 Features

- [✨ add conflict resolution ([#4](https://github.com/kannika-io/same/pull/4))](https://github.com/kannika-io/same/commit/8cc18d3180295e09bb141c20ff9d4693d0672972)
- [✨ add tcp keep alive setting ([#1](https://github.com/kannika-io/same/pull/1))](https://github.com/kannika-io/same/commit/9da7a079b004f3e377045b42e8bdfaa9207f2565)
- [✨ print missed schemas ([#5](https://github.com/kannika-io/same/pull/5))](https://github.com/kannika-io/same/commit/e198e74ca6bd715dde8d27c95157fdf6ce2edb36)

### 🧹 Miscellaneous Tasks

- [🔧 cargo fmt](https://github.com/kannika-io/same/commit/675d04d188cd83d1244405544a7bb0228761e91b)
- [🔧 bump version to 0.3.0](https://github.com/kannika-io/same/commit/fc10c65ed84662700d937fd4ee5e83c62244ece3)
- [🔧 update deps](https://github.com/kannika-io/same/commit/a17fe14c8e26683d5106d9286a987ef0f2e9372c)
- [🔧 update to rust 1.80](https://github.com/kannika-io/same/commit/a336d80f7663fa87c5675f52105fff7098acf209)
- [🔧 update to apache-avro 0.17.0](https://github.com/kannika-io/same/commit/f5e3ae06dd0c757af8b441f394d95e5e43bc2d87)

### Release

- [🚀 update changelog](https://github.com/kannika-io/same/commit/1fd398c7de70a99eb403cbac072c7fff2436290c)

## [0.2.1] - 2024-07-25

### 🌟 Features

- [✨ support logicalTypes when calculating fingerprints](https://github.com/kannika-io/same/commit/0caac3fe48970912a8634e18fb62fdc0e51384f9)

### 🧹 Miscellaneous Tasks

- [🔧 add .dockerignore](https://github.com/kannika-io/same/commit/995933235d53ccebac96402ffb6ce9b499300a52)

## [0.2.0] - 2024-07-24

### 🌟 Features

- [✨ add --ignore-indexing-errors and --offline options](https://github.com/kannika-io/same/commit/2c8063521a666bff20d00d99c27e9f51e261c3c0)

### 🧪 Testing

- [🚨 set up test env](https://github.com/kannika-io/same/commit/ec6a3d998905cca5aefb87eb3de05ed39b9cd83b)

### Dev

- [Add cliff.toml](https://github.com/kannika-io/same/commit/5a37629a897d0fc8d0d283cf5b9a462537191e5b)
- [Set version to 0.2.0](https://github.com/kannika-io/same/commit/84d28e5224d38074b3387f124d36ff5b0f7f3f28)

## [0.1.0] - 2024-04-26

### 🌟 Features

- [Add registry module to work with schema registry](https://github.com/kannika-io/same/commit/6a94b064adbe5fcdb9d995b2111fe4035f401d32)
- [✨ save contexts to config file in data directory](https://github.com/kannika-io/same/commit/838687ef2e2ef0275a6d173148924422f1d5db23)
- [✨ store passwords in keychain](https://github.com/kannika-io/same/commit/b312f92158129aedfaecea70ab7cea96dd8bc6d8)
- [✨ download schema files](https://github.com/kannika-io/same/commit/8518470ced19c40af81ddd67baa222a16d3b92ba)
- [✨ calculate fingerprint of Avro schemas](https://github.com/kannika-io/same/commit/e4513da6c6c2d7bb85fd4317bb296b34d84ea111)
- [✨ index schemas by fingerprint and ids per context](https://github.com/kannika-io/same/commit/296e83ce0f877b1637163461d3438993ce3fdedb)
- [✨ map schemas](https://github.com/kannika-io/same/commit/f5ae314da5f31eef69f2b251cc4f4c116f4d2343)
- [✨ allow force updating schemas](https://github.com/kannika-io/same/commit/29559e5831ddc196a42a1c76a31957a9af20f13b)
- [✨ handle tmux issues](https://github.com/kannika-io/same/commit/9942c7a64492dd8f0004db85c6558e4ad1c9dfa0)
- [✨ handle different subjects with the same schema](https://github.com/kannika-io/same/commit/7296582ffd2ab82b250f1c267bf00662caa151ee)
- [✨ handle schema references](https://github.com/kannika-io/same/commit/a80816b29945ac4d971cae2d1340efb7123d8eee)
- [✨ add --registries option](https://github.com/kannika-io/same/commit/456c78fd679e851d1b1794c5048144f55a21ad62)
- [✨ add Dockerfile](https://github.com/kannika-io/same/commit/f18be80f1fb8bc801da439367a61beac6c8e3b40)

### 🐛 Bug Fixes

- [🐛 load correct entry from keyring](https://github.com/kannika-io/same/commit/d6ab74414a96ff7ef3a33713b5853fe784625805)
- [🐛 correct doc](https://github.com/kannika-io/same/commit/2876af71a1cc93a753c03ffbc912e860f5cf17aa)

### 🛠️ Refactor

- [♻️ improve logging](https://github.com/kannika-io/same/commit/2d8b9df3ec3c293ed7a303bf6c4607a92692f8d4)
- [♻️ improve logging](https://github.com/kannika-io/same/commit/1f875c02c22eac47b369aacbfd16db59762802b4)
- [♻️ improve logging](https://github.com/kannika-io/same/commit/625f84bd39a775136e9af368926011632589bfc6)
- [♻️ remove empty file](https://github.com/kannika-io/same/commit/50463066e2302c650f7c1a7680c4554727721329)

### 📚 Documentation

- [📚️ docker example](https://github.com/kannika-io/same/commit/72546ae042606f595008134b1c626762bf120a12)

### 🧪 Testing

- [🚨 check if docs are ignored](https://github.com/kannika-io/same/commit/299beec548f7f2aa634b4b19209bb2141eee59a3)

### 🧹 Miscellaneous Tasks

- [🔧 ignore .idea/](https://github.com/kannika-io/same/commit/2a64f7c39617ca48e0ac96ef5a1954a88a5d68f8)
- [🔧 improve verbose flag](https://github.com/kannika-io/same/commit/35a0a9b29463cd55affcbfa9b2882dff118af539)
- [🔧 improve logging](https://github.com/kannika-io/same/commit/2d6f6dad57e99ad66eb3b1a81342c0934945f6c3)
- [🔧 use apache-avro crate instead of avro-rs](https://github.com/kannika-io/same/commit/df10d90c0cbfead49eb8a4c5491958c3aeb4f809)
- [🔧 ignore .DS_Store](https://github.com/kannika-io/same/commit/57cc04eb522c6e1bb4cc3176db2b919be0a76b23)

<!-- generated by git-cliff -->
