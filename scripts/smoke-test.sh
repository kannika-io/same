#!/usr/bin/env bash
#
# End-to-end check of `same map` against two real schema registries.
#
# Starts two Redpanda containers and registers in both:
#   - an Avro schema without references
#   - an Avro customer/product/order chain where order references the other two
#   - a JSON schema whose annotations differ between the registries
#   - a JSON customer/product/order chain, registered on the target under different subject
#     names and reference names so that matching has to go through referenced content
# Then runs `same map` and asserts that all 8 subjects map and nothing is missed.
# Pass --compare-main to also run the same mapping with a build of `main` and show the diff.
#
# Requirements: docker, curl, jq, cargo.
#
# Usage:
#   scripts/smoke-test.sh                 # run against the current branch
#   scripts/smoke-test.sh --compare-main  # additionally build and run `main` for comparison
#
# Environment overrides:
#   REDPANDA_IMAGE  (default: docker.redpanda.com/redpandadata/redpanda:v24.2.18)
#   SOURCE_PORT     (default: 18081)
#   TARGET_PORT     (default: 18082)

set -euo pipefail

REDPANDA_IMAGE="${REDPANDA_IMAGE:-docker.redpanda.com/redpandadata/redpanda:v24.2.18}"
SOURCE_PORT="${SOURCE_PORT:-18081}"
TARGET_PORT="${TARGET_PORT:-18082}"
COMPARE_MAIN=false
[[ "${1:-}" == "--compare-main" ]] && COMPARE_MAIN=true

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK="$(mktemp -d)"
MAIN_WORKTREE="$WORK/same-main"
CONTAINERS=(same-test-source same-test-target)

# `same` caches schemas per context name under the platform cache directory. Use names unique
# to this run so stale subjects from another registry can never show up as misses.
RUN_ID="$$"
FROM_CTX="same-test-source-$RUN_ID"
TO_CTX="same-test-target-$RUN_ID"
case "$(uname -s)" in
  Darwin) CACHE_ROOT="$HOME/Library/Caches/io.kannika.same" ;;
  *)      CACHE_ROOT="${XDG_CACHE_HOME:-$HOME/.cache}/io.kannika.same" ;;
esac

log()  { printf '\n\033[1;34m==> %s\033[0m\n' "$*"; }
fail() { printf '\033[1;31mFAIL: %s\033[0m\n' "$*" >&2; exit 1; }

cleanup() {
  log "Cleaning up"
  docker rm -f "${CONTAINERS[@]}" >/dev/null 2>&1 || true
  if [[ -d "$MAIN_WORKTREE" ]]; then
    git -C "$ROOT" worktree remove --force "$MAIN_WORKTREE" >/dev/null 2>&1 || true
  fi
  rm -rf "$WORK" "$CACHE_ROOT/$FROM_CTX" "$CACHE_ROOT/$TO_CTX"
}
trap cleanup EXIT

start_registry() {
  local name=$1 port=$2
  docker run -d --rm --name "$name" -p "$port:8081" "$REDPANDA_IMAGE" \
    redpanda start --smp 1 --memory 1G --mode dev-container >/dev/null
}

wait_for_registry() {
  local name=$1 port=$2
  for _ in $(seq 1 90); do
    if curl -sf "http://localhost:$port/subjects" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "container status: $(docker ps -a --filter "name=$name" --format '{{.Status}}')" >&2
  docker logs "$name" 2>&1 | tail -20 >&2 || true
  fail "schema registry $name on port $port did not become ready"
}

# register <port> <subject> <file> <AVRO|JSON> [references-json]
# references-json is a JSON array of {name, subject, version} as in the registry API.
register() {
  local port=$1 subject=$2 file=$3 type=$4 refs=${5:-[]}
  local id
  id=$(jq -n --rawfile s "$file" --arg t "$type" --argjson r "$refs" \
        '{schema: $s, schemaType: $t, references: $r}' \
    | curl -sf -X POST -H 'Content-Type: application/vnd.schemaregistry.v1+json' \
        -d @- "http://localhost:$port/subjects/$subject/versions" \
    | jq -r .id) || fail "registering $subject on port $port"
  echo "    $subject -> id $id (port $port)"
}

ref() { jq -cn --arg n "$1" --arg s "$2" '{name: $n, subject: $s, version: 1}'; }

run_map() {
  local binary=$1 out=$2
  "$binary" map \
    --from "$FROM_CTX" --to "$TO_CTX" \
    --registries "$WORK/registries.yaml" \
    --on-conflict strict \
    --force-update \
    -o "$out"
}

# ---------------------------------------------------------------------------

log "Starting registries ($REDPANDA_IMAGE)"
docker rm -f "${CONTAINERS[@]}" >/dev/null 2>&1 || true
start_registry same-test-source "$SOURCE_PORT"
start_registry same-test-target "$TARGET_PORT"
wait_for_registry same-test-source "$SOURCE_PORT"
wait_for_registry same-test-target "$TARGET_PORT"

AVRO_REF="$ROOT/tests/assets/avro/ref"
JSON_REF="$ROOT/tests/assets/json/ref"

log "Registering schemas"
echo "  Avro without references (identical in both):"
register "$SOURCE_PORT" user-value "$ROOT/tests/assets/avro/user/v1.avsc" AVRO
register "$TARGET_PORT" user-value "$ROOT/tests/assets/avro/user/v1.avsc" AVRO

echo "  Avro with references (identical in both, reference names are fully qualified):"
for port in "$SOURCE_PORT" "$TARGET_PORT"; do
  register "$port" customer "$AVRO_REF/customer.avsc" AVRO
  register "$port" product  "$AVRO_REF/product.avsc"  AVRO
  register "$port" order    "$AVRO_REF/order.avsc"    AVRO \
    "[$(ref io.kannika.Customer customer), $(ref io.kannika.Product product)]"
done

echo "  JSON (annotations differ between registries):"
register "$SOURCE_PORT" order-placed-value "$ROOT/tests/assets/json/annotations/source.json" JSON
register "$TARGET_PORT" order-placed-value "$ROOT/tests/assets/json/annotations/target.json" JSON

echo "  JSON with references (target uses other subject and reference names):"
register "$SOURCE_PORT" json-customer "$JSON_REF/customer.json" JSON
register "$SOURCE_PORT" json-product  "$JSON_REF/product.json"  JSON
register "$SOURCE_PORT" json-order    "$JSON_REF/order.json"    JSON \
  "[$(ref customer.json json-customer), $(ref product.json json-product)]"
register "$TARGET_PORT" io.kannika.customer "$JSON_REF/customer.json" JSON
register "$TARGET_PORT" io.kannika.product  "$JSON_REF/product.json"  JSON
register "$TARGET_PORT" io.kannika.order    "$JSON_REF/order-alt-ref-names.json" JSON \
  "[$(ref io/kannika/Customer.schema.json io.kannika.customer), $(ref io/kannika/Product.schema.json io.kannika.product)]"

cat > "$WORK/registries.yaml" <<YAML
registries:
- name: $FROM_CTX
  url: http://localhost:$SOURCE_PORT
- name: $TO_CTX
  url: http://localhost:$TARGET_PORT
YAML

log "Building current branch ($(git -C "$ROOT" rev-parse --abbrev-ref HEAD) @ $(git -C "$ROOT" rev-parse --short HEAD))"
cargo build --manifest-path "$ROOT/Cargo.toml"
echo "    built $ROOT/target/debug/same"

log "Mapping with current branch"
run_map "$ROOT/target/debug/same" "$WORK/mapping.yaml"
cat "$WORK/mapping.yaml"

EXPECTED=8
matched=$(grep -cE '^  [0-9]+: [0-9]+$' "$WORK/mapping.yaml" || true)
[[ "$matched" -eq "$EXPECTED" ]] || fail "expected $EXPECTED mapped schemas, got $matched"
grep -q '^missed:' "$WORK/mapping.yaml" && fail "expected no missed schemas"
printf '\033[1;32mOK: all %s Avro and JSON subjects mapped (with and without references), nothing missed\033[0m\n' "$EXPECTED"

if $COMPARE_MAIN; then
  log "Building main in a separate worktree and target dir"
  git -C "$ROOT" worktree add --detach "$MAIN_WORKTREE" main >/dev/null
  log "Building main (@ $(git -C "$MAIN_WORKTREE" rev-parse --short HEAD))"
  cargo build --manifest-path "$MAIN_WORKTREE/Cargo.toml" --target-dir "$WORK/main-target"
  echo "    built $WORK/main-target/debug/same"

  log "Mapping with main"
  run_map "$WORK/main-target/debug/same" "$WORK/mapping-main.yaml" || true
  cat "$WORK/mapping-main.yaml"

  log "Diff (main -> branch)"
  diff "$WORK/mapping-main.yaml" "$WORK/mapping.yaml" || true
  echo "The Avro mapping lines should be identical; the JSON subjects with annotations or references are misses on main and matches here."
fi
