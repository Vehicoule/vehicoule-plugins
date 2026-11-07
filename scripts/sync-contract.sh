#!/usr/bin/env bash
# Keeps the copy of the plugin contract in `wit/`, and the copies of Vehicoule's plugin documents in
# `reference/vehicoule/`, on the Vehicoule commit that `conformance/Cargo.toml` pins the plugin host to (ADR 0002): the
# plugins build against the contract the host of that commit enforces, and agents read the specs of that commit.
#
#   scripts/sync-contract.sh check          fail if wit/ or reference/vehicoule/ differs from Vehicoule at the pinned
#                                           commit
#   scripts/sync-contract.sh update [REV]   pin the host to REV (default: the newest commit of Vehicoule's main),
#                                           copy its wit/ and documents, and refresh conformance/Cargo.lock
#
# VEHICOULE_REPO overrides where Vehicoule is fetched from (default: its GitHub repository), for a mirror.
set -euo pipefail

repo_url="${VEHICOULE_REPO:-https://github.com/Vehicoule/Vehicoule}"
manifest="conformance/Cargo.toml"
# Vehicoule's documents about plugins, copied under reference/vehicoule/ with the same paths.
documents=(
    docs/plugin-host-spec.md
    docs/plugin-providers.md
    docs/threat-model.md
    docs/adr/0007-plugin-host.md
    docs/adr/0012-one-regression-test-per-fixed-bug.md
    docs/adr/0013-fuzz-the-plugin-hosts-untrusted-inputs.md
    docs/adr/0014-prove-the-plugin-host-checks-with-kani.md
)

root="$(git rev-parse --show-toplevel)"
cd "$root"

pinned_rev() {
    sed -n 's/^vehicoule-plugin-host = { git = "https:\/\/github\.com\/Vehicoule\/Vehicoule", rev = "\([0-9a-f]\{40\}\)", version = "[^"]*" }$/\1/p' "$manifest"
}

current="$(pinned_rev)"
if [[ ! "$current" =~ ^[0-9a-f]{40}$ ]]; then
    echo "error: no Vehicoule rev found for vehicoule-plugin-host in $manifest" >&2
    exit 1
fi

mode="${1:-}"
case "$mode" in
    check) rev="$current" ;;
    update) rev="${2:-$(git ls-remote "$repo_url" refs/heads/main | cut -f1)}" ;;
    *)
        echo "usage: $0 check | update [REV]" >&2
        exit 2
        ;;
esac
if [[ ! "$rev" =~ ^[0-9a-f]{40}$ ]]; then
    echo "error: not a full commit hash: $rev" >&2
    exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git -C "$work" init -q
git -C "$work" fetch -q --depth 1 "$repo_url" "$rev"
git -C "$work" checkout -q FETCH_HEAD

if [[ ! -d "$work/wit" ]]; then
    echo "error: Vehicoule $rev has no wit/ directory" >&2
    exit 1
fi

expected="$work/expected"
mkdir -p "$expected"
for document in "${documents[@]}"; do
    if [[ ! -f "$work/$document" ]]; then
        echo "error: Vehicoule $rev has no $document" >&2
        exit 1
    fi
    mkdir -p "$expected/$(dirname "$document")"
    cp "$work/$document" "$expected/$document"
done

if [[ "$mode" == "check" ]]; then
    status=0
    if ! diff -r "$work/wit" wit; then
        echo "error: wit/ differs from Vehicoule $rev; run scripts/sync-contract.sh update $rev" >&2
        status=1
    fi
    if ! diff -r "$expected" reference/vehicoule; then
        echo "error: reference/vehicoule/ differs from Vehicoule $rev; run scripts/sync-contract.sh update $rev" >&2
        status=1
    fi
    [[ "$status" -eq 0 ]] && echo "wit/ and reference/vehicoule/ match Vehicoule $rev"
    exit "$status"
fi

rm -rf wit reference/vehicoule
cp -R "$work/wit" wit
cp -R "$expected" reference/vehicoule
if [[ "$rev" != "$current" ]]; then
    sed -i.bak "s/rev = \"$current\"/rev = \"$rev\"/" "$manifest" && rm -f "$manifest.bak"
    # Resolves the new rev into conformance/Cargo.lock without updating anything else.
    cargo metadata --manifest-path "$manifest" --format-version 1 >/dev/null
fi
echo "Contract and host pinned to Vehicoule $rev (was $current)"
