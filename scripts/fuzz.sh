#!/usr/bin/env bash
# Builds every fuzz target and runs each for a fixed number of inputs from a fixed seed (ADR 0006), never for a time,
# so the result does not depend on the machine's speed (AGENTS.md rule 7).
#
# Usage: scripts/fuzz.sh [RUNS]
# RUNS defaults to 200,000, Vehicoule's count for its fast targets (2 to 3 s each there); the first target measures
# its own and adjusts it in its pull request. Needs cargo-fuzz and the nightly that fuzz/rust-toolchain.toml pins
# (versions in docs/references.md). With no fuzz/ workspace yet, it says so and succeeds.
set -euo pipefail

runs="${1:-200000}"
root="$(cd "$(dirname "$0")/.." && pwd)"

if [[ ! -f "$root/fuzz/Cargo.toml" ]]; then
    echo "No fuzz/ workspace yet (ADR 0006): it arrives with the first plugin's targets."
    exit 0
fi

cd "$root/fuzz"
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo fuzz build
mapfile -t targets < <(cargo fuzz list)
echo "${#targets[@]} targets: ${targets[*]}"
if [[ "${#targets[@]}" -lt 1 ]]; then
    echo "error: fuzz/ exists but holds no target" >&2
    exit 1
fi
for target in "${targets[@]}"; do
    cargo fuzz run "$target" -- -seed=1 -runs="$runs"
done
