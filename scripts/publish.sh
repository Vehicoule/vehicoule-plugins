#!/usr/bin/env bash
# Builds release wasm32-wasip2 for every plugin under plugins/*, copies
# <id>-<version>.wasm and <id>-<version>.manifest.json into the output directory,
# computes their sha256, increments feed/sequence, and signs the resulting payload
# into <output-dir>/feed.json. URLs in the feed are relative to the feed itself.
#
# Needs `vehicoule-feed` on PATH (see README.md) and VEHICOULE_FEED_KEY set to the
# path of a signing key file (`vehicoule-feed keygen <path>`).
set -euo pipefail

usage() { echo "usage: $0 <output-dir>" >&2; exit 2; }
[ $# -eq 1 ] || usage

if ! command -v vehicoule-feed >/dev/null 2>&1; then
    echo "error: vehicoule-feed not found on PATH. Install it with:" >&2
    echo "  cargo install --git file:///Users/btw/Documents/repo/Vehicoule --tag plugin-host-v1.0.0 plugin-host --bin vehicoule-feed" >&2
    exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
    echo "error: jq not found on PATH." >&2
    exit 1
fi
if ! command -v shasum >/dev/null 2>&1; then
    echo "error: shasum not found on PATH." >&2
    exit 1
fi
if [ -z "${VEHICOULE_FEED_KEY:-}" ]; then
    echo "error: VEHICOULE_FEED_KEY must be set to a signing key file (see 'vehicoule-feed keygen')." >&2
    exit 1
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="$1"
mkdir -p "$out"
# The unsigned payload (and a half-written feed) never land in the output dir.
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp" "$out/feed.json.tmp"' EXIT

feed_name="official"

entries="[]"
for manifest in "$root"/plugins/*/manifest.json; do
    [ -f "$manifest" ] || continue
    dir="$(dirname "$manifest")"
    id="$(basename "$dir")"
    version="$(jq -r .version "$manifest")"
    contract="$(jq -r .contract "$manifest")"

    cargo build --release --target wasm32-wasip2 -p "$id" \
        --manifest-path "$root/Cargo.toml" --target-dir "$root/target/publish" >&2

    wasm_name="$id-$version.wasm"
    manifest_name="$id-$version.manifest.json"
    cp "$root/target/publish/wasm32-wasip2/release/${id//-/_}.wasm" "$out/$wasm_name"
    cp "$manifest" "$out/$manifest_name"

    wasm_sha="$(shasum -a 256 "$out/$wasm_name" | cut -d' ' -f1)"
    manifest_sha="$(shasum -a 256 "$out/$manifest_name" | cut -d' ' -f1)"

    entry="$(jq -n \
        --arg id "$id" --arg version "$version" --arg contract "$contract" \
        --arg wasm_url "$wasm_name" --arg wasm_sha256 "$wasm_sha" \
        --arg manifest_url "$manifest_name" --arg manifest_sha256 "$manifest_sha" \
        '{id: $id, version: $version, contract: $contract, wasm_url: $wasm_url, wasm_sha256: $wasm_sha256, manifest_url: $manifest_url, manifest_sha256: $manifest_sha256}')"
    entries="$(jq -c ". + [$entry]" <<<"$entries")"
done

# Computed in memory first: feed/sequence is only persisted once signing below has
# actually succeeded, so a failed sign (bad/missing key) never burns a sequence number.
sequence_file="$root/feed/sequence"
sequence=$(( $(cat "$sequence_file") + 1 ))

payload="$tmp/payload.json"
jq -n --arg name "$feed_name" --argjson sequence "$sequence" --argjson plugins "$entries" \
    '{name: $name, sequence: $sequence, plugins: $plugins}' >"$payload"

vehicoule-feed sign "$payload" "$VEHICOULE_FEED_KEY" >"$out/feed.json.tmp"
mv "$out/feed.json.tmp" "$out/feed.json"
echo "$sequence" > "$sequence_file"
echo "published sequence $sequence -> $out/feed.json" >&2
echo "reminder: commit feed/sequence (git -C \"$root\" commit -s -m \"feed: sequence $sequence\" -- feed/sequence)" >&2
