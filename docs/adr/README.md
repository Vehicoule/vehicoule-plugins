# Architecture Decision Records

One decision per file, numbered, never renumbered. A decision is changed by a new ADR that supersedes it, never by
editing the old one beyond its status line. Numbers are this repository's own; Vehicoule's ADRs are cited with their
repository.

Status: **Proposed** (written, waiting for the user), **Accepted** (the user decided), **Superseded by NNNN**.
Only the user moves an ADR from Proposed to Accepted.

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-write-plugins-in-rust-as-wasm-components.md) | Plugins in Rust, as Wasm components with no WASI | Accepted |
| [0002](0002-pin-the-contract-and-check-plugins-in-the-real-host.md) | The contract pinned to a Vehicoule commit, every plugin installed in that commit's host | Accepted |
| [0003](0003-take-vehicoules-rules-where-they-apply.md) | Vehicoule's rules and foundations, and which ones do not apply to plugins | Accepted |
| [0004](0004-license-the-plugins-under-gpl-3.0.md) | The plugins are licensed under GPL-3.0 | Accepted |
| [0005](0005-allow-unsafe-only-in-the-generated-bindings.md) | `unsafe` only in the bindings that wit-bindgen generates | Accepted |
| [0006](0006-verify-plugins-without-the-network.md) | Tests without the network, 32-bit runs, fuzzing, Kani where it fits, mutants | Accepted |
| [0007](0007-test-only-the-changed-lines-mutants-on-pull-requests.md) | Pull requests test only the mutants in the lines they change; `main` tests them all | Accepted |
