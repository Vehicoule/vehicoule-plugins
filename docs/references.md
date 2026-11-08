# References

Every source an agent may rely on, with the version it is pinned to. Use the pinned copy when there is one; the live
site may describe a newer version than the one this repository builds against.

## The contract and the host

| What | Where | Pinned |
|---|---|---|
| The plugin contract (WIT) | [`wit/plugin.wit`](../wit/plugin.wit), a copy of Vehicoule's `wit/` | The Vehicoule commit in [`conformance/Cargo.toml`](../conformance/Cargo.toml), checked by `scripts/sync-contract.sh check` ([ADR 0002](adr/0002-pin-the-contract-and-check-plugins-in-the-real-host.md)) |
| The plugin host spec, providers, threat model and Vehicoule's ADRs 0007, 0012, 0013, 0014 | [`reference/vehicoule/docs/`](../reference/vehicoule/docs/), copied by `scripts/sync-contract.sh` | Same commit, checked in CI |
| The plugin host | `vehicoule-plugin-host`, built by `conformance/` | Same commit |
| Guest bindings | [`reference/wit-bindgen/`](../reference/wit-bindgen/): README and `src/lib.rs`, which documents `generate!` and `export!` | `0.63.0` in the root `Cargo.toml` |
| Components | https://github.com/bytecodealliance/wasm-tools | `1.263.0`, installed by CI (`.github/workflows/ci.yml`) |
| The WIT language | [`reference/component-model/WIT.md`](../reference/component-model/WIT.md) | `WebAssembly/component-model` at `d230073` |
| The component model, explained | https://component-model.bytecodealliance.org | |

## Rust

| What | Where |
|---|---|
| Toolchain | `1.99.1`, in [`rust-toolchain.toml`](../rust-toolchain.toml), with `wasm32-unknown-unknown` |
| Google's Rust style guide (Chrome and Fuchsia tagged rules do not apply here) | [`reference/rust/google-rust-style-guide.md`](../reference/rust/google-rust-style-guide.md), from https://google.github.io/styleguide/rustguide.html |
| Apollo's Rust best practices | [`.claude/skills/rust-best-practices/`](../.claude/skills/rust-best-practices/) |
| Rust coding guidelines (50 core rules) | [`.claude/skills/rust-coding-guidelines/`](../.claude/skills/rust-coding-guidelines/) |
| Rust API Guidelines | https://rust-lang.github.io/api-guidelines/ |
| Clippy lints | https://rust-lang.github.io/rust-clippy/ |

## Verification tools

| What | Where | Pinned |
|---|---|---|
| Mutation testing | https://mutants.rs | `27.1.0`, installed by CI |
| Dependency checks | https://embarkstudios.github.io/cargo-deny/ | `cargo-deny` `0.20.2`, installed by CI |
| Bounded model checking | https://model-checking.github.io/kani/ | `kani-verifier` `0.68.0`, installed by CI once a proof exists ([ADR 0006](adr/0006-verify-plugins-without-the-network.md)) |
| Fuzzing | https://rust-fuzz.github.io/book/cargo-fuzz.html | `cargo-fuzz` `0.13.2`, installed by CI once `fuzz/` exists; `libfuzzer-sys` `0.4.13` and `nightly-2026-11-01`, as Vehicoule ([ADR 0006](adr/0006-verify-plugins-without-the-network.md)) |
| Spell check | https://github.com/crate-ci/typos | `typos-cli` `1.52.0`, installed by CI |
| Unused dependencies | https://github.com/bnjbvr/cargo-machete | `0.9.2`, installed by CI |
| Workflow audit | https://docs.zizmor.sh | `zizmor` `1.32.0`, installed by CI |
| Licensing of every file | https://reuse.software | `reuse` `6.3.0`, run by `.github/workflows/ci.yml` |
