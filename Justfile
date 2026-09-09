# Common development tasks. Run `just` to list them.

default:
    @just --list

# Build the debug binary
build:
    cargo build

# Unit tests only, no Docker needed
test:
    cargo test --lib --bins

# Full suite including the Redpanda-backed integration tests (needs Docker)
test-all:
    cargo test --no-fail-fast

# End-to-end run of `same map` against two Redpanda containers (needs Docker, curl, jq)
smoke:
    scripts/smoke-test.sh

# Same as `smoke`, and also runs a build of `main` for comparison
smoke-compare:
    scripts/smoke-test.sh --compare-main

# Format and lint
check:
    cargo fmt --check
    cargo clippy --all-targets

# Open a release PR for the given version via the Prepare release workflow (needs gh). See RELEASING.md
prepare-release version:
    gh workflow run prepare-release.yml -f version={{version}}
    @echo "Dispatched. Follow with: gh run watch"
