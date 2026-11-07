#!/usr/bin/env bash
# Builds every plugin, and the template, into a Wasm component named after its manifest id.
#
# Usage: scripts/build-plugins.sh OUT_DIR
# Needs the `wasm32-unknown-unknown` target of the pinned toolchain and `wasm-tools` (version in docs/references.md).
# Writes OUT_DIR/<id>.wasm for `template/` and for each `plugins/<id>/`; the conformance test installs every
# component of OUT_DIR in Vehicoule's plugin host (ADR 0002).
set -euo pipefail

out="${1:?usage: scripts/build-plugins.sh OUT_DIR}"
root="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$out"

# Each crate as `<id> <directory>`; a plugin's directory is its id (docs/plugin-crates.md).
crates=("template $root/template")
for dir in "$root"/plugins/*/; do
  [[ -f "$dir/Cargo.toml" ]] || continue
  crates+=("$(basename "$dir") ${dir%/}")
done

for entry in "${crates[@]}"; do
  id="${entry%% *}"
  dir="${entry#* }"
  package="vehicoule-plugin-$id"
  cargo build --manifest-path "$root/Cargo.toml" --locked --release --target wasm32-unknown-unknown -p "$package"
  wasm-tools component new "$root/target/wasm32-unknown-unknown/release/${package//-/_}.wasm" -o "$out/$id.wasm"
  echo "built $out/$id.wasm from $dir"
done
