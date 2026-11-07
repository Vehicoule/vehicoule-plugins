#!/usr/bin/env bash
# Checks the layout every plugin follows (docs/plugin-crates.md), so a plugin cannot arrive without what the rules
# require of it.
#
# Usage: scripts/check-layout.sh
# For each `plugins/<id>/`: the package is `vehicoule-plugin-<id>`, and at least one fuzz target
# `fuzz/fuzz_targets/<id>_*.rs` reads what the plugin receives from outside (AGENTS.md rule 15, ADR 0006).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

status=0
count=0
for dir in plugins/*/; do
    [[ -d "$dir" ]] || continue
    count=$((count + 1))
    id="$(basename "$dir")"
    if [[ ! "$id" =~ ^[a-z0-9-]+$ ]]; then
        echo "error: $dir: a plugin's directory is its manifest id: lowercase ASCII letters, digits and -" >&2
        status=1
    fi
    if ! grep -qx "name = \"vehicoule-plugin-$id\"" "$dir/Cargo.toml" 2>/dev/null; then
        echo "error: $dir/Cargo.toml: the package must be named vehicoule-plugin-$id" >&2
        status=1
    fi
    if ! compgen -G "fuzz/fuzz_targets/${id//-/_}_*.rs" >/dev/null; then
        echo "error: $dir: no fuzz target fuzz/fuzz_targets/${id//-/_}_*.rs for what the plugin reads from outside" >&2
        status=1
    fi
done

if [[ "$status" -eq 0 ]]; then
    echo "layout checked: $count plugin(s)"
fi
exit "$status"
