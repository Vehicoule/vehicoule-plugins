# Vendored references

Sources of truth copied into the repository so every agent reads the same pinned version. Never edit them by hand:
update by copying a newer revision in a pull request of its own.

| File | Source | Revision | License |
|---|---|---|---|
| `rust/google-rust-style-guide.md` | https://google.github.io/styleguide/rustguide.html, converted to Markdown with pandoc on 2026-11-06 (same copy as `Vehicoule/Vehicoule` and `Vehicoule/nsh-m3e`) | page as of 2026-11-06 | CC BY 3.0, © Google |
| `vehicoule/docs/` | Vehicoule's plugin host spec, plugin providers, threat model and ADRs 0007, 0012, 0013 and 0014, copied by `scripts/sync-contract.sh` and checked by it in CI | The Vehicoule commit in `conformance/Cargo.toml` | GPL-3.0-only, © the Vehicoule contributors |
| `wit-bindgen/README.md`, `wit-bindgen/lib.rs` | The `wit-bindgen` crate's README and `src/lib.rs`, which documents every option of `generate!` and `export!`, from crates.io | `0.63.0`, the version in the root `Cargo.toml` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`, taken under MIT |
| `component-model/WIT.md` | https://github.com/WebAssembly/component-model, `design/mvp/WIT.md`: the WIT language | `d230073ce81c6634f175b4d0c5ad900b5253f1f4` (main on 2026-11-07) | Apache-2.0 |

A pull request that bumps `wit-bindgen` copies its two files again from the new version, in the same pull request.

The links between Vehicoule's documents are relative to Vehicoule's repository; inside `vehicoule/docs/` they resolve
between the copied files only.
