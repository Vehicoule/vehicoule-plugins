#!/usr/bin/env bash
# Runs every Kani proof of the workspace (ADR 0006), and fails unless Kani verifies exactly as many harnesses as it
# lists, and at least one.
#
# Usage: scripts/prove.sh
# Needs Kani (version in docs/references.md), after `cargo kani setup`. With no `cfg(kani)` module in the repository
# yet, it says so and succeeds without running Kani.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

if ! grep -rqs --include='*.rs' 'cfg(kani)' template plugins; then
    echo "No Kani proof in the repository yet (ADR 0006): nothing to prove."
    exit 0
fi

log="$(mktemp)"
trap 'rm -f "$log" kani-list.json' EXIT
cargo kani list --format json
total="$(jq '.totals."standard-harnesses"' kani-list.json)"
contracts="$(jq '.totals."contract-harnesses"' kani-list.json)"
echo "$total harnesses, $contracts contract harnesses."
if [[ "$contracts" != 0 ]]; then
    echo "error: contract harnesses are not run by this script yet" >&2
    exit 1
fi
if [[ "$total" -lt 1 ]]; then
    echo "error: a cfg(kani) module exists but Kani lists no harness" >&2
    exit 1
fi
cargo kani --workspace | tee "$log"
grep -qx "Complete - $total successfully verified harnesses, 0 failures, $total total." "$log"
