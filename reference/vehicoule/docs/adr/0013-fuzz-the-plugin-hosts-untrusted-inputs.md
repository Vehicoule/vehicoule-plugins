# 0013. Fuzz the plugin host's untrusted inputs with cargo-fuzz and libFuzzer

- Status: Accepted
- Date: 2026-11-07
- Deciders: the user

## Context

The plugin host takes input nobody in the project wrote: a plugin's bytes come from a feed, its manifest from its own
`describe`, and its requests from its own code ([ADR 0007](0007-plugin-host.md), `docs/plugin-host-spec.md`). Property
tests ([ADR 0004](0004-verification.md)) check the rules over random values chosen by their strategies; a
coverage-guided fuzzer instead looks for the inputs that reach code no test reached, which is how parser bugs are
usually found.

ripgrep (`BurntSushi/ripgrep` at 91f5d22) keeps cargo-fuzz targets in `fuzz/` and has CI check that they build. The
usual Rust fuzzer, cargo-fuzz, needs a nightly toolchain (it instruments the code with `-Z` flags) and links LLVM's
libFuzzer, which is C++, through the `libfuzzer-sys` crate, under `(MIT OR Apache-2.0) AND NCSA`. AGENTS.md rule 11
and [ADR 0005](0005-pinned-versions-and-dependencies.md) require the user's explicit agreement for a dependency written
in C; the user gave it on 2026-11-07 for this dependency only.

## Options

1. **cargo-fuzz with libFuzzer, in a workspace of its own, on a pinned nightly.** The usual tool, the one the Rust
   fuzzing book documents. Needs nightly and a C++ dependency, kept out of the app's graph.
2. **Another engine: AFL++ (`afl.rs`) or honggfuzz (`honggfuzz-rs`).** Both engines are written in C, so they do not
   avoid the dependency, and they need tools of their own on the machine.
3. **No fuzzing; property tests only.** No new tool, but no coverage guidance on the inputs the app trusts least.

## Decision

Option 1:

- The targets live in `crates/vehicoule-plugin-host/fuzz/`, a workspace of its own with its own `Cargo.lock` (like the
  test plugins in `fixtures/`), so libFuzzer and the nightly never reach the app's build, `cargo test --workspace`,
  mutation testing or Kani. Its `rust-toolchain.toml` pins the nightly; a newer one is a pull request of its own.
- `cargo deny` checks that workspace with the root `deny.toml`. libFuzzer's NCSA license is allowed for
  `libfuzzer-sys` only, as a license exception: it is GPL-3.0 compatible, and the fuzz targets are never shipped.
- The first targets cover the host's public entry points for untrusted input:
  - `install`: any bytes installed as a plugin; the host never panics or crashes, never reaches the network, and a
    plugin it accepts has a manifest that passes the manifest rules again.
  - `manifest`: any manifest fields; never a panic, the accessors give back what was validated, validating it again
    gives the same manifest, and `allows_host` agrees with the patterns.
  - `host_pattern`: any pattern and host; never a panic, an accepted pattern is plain lowercase ASCII, and a match
    happens only where section 4 allows one.
- CI (the `fuzz` job) builds the targets, checks their format and Clippy, replays the test plugins through `install`
  (`-runs=0`), and runs the fast targets for a fixed number of inputs from a fixed seed. A run is bounded by its number
  of inputs, never by time, so its result does not depend on the runner's speed (AGENTS.md rule 7).
- A crash found by fuzzing is a bug: its fix comes with a regression test that reproduces it
  ([ADR 0012](0012-one-regression-test-per-fixed-bug.md)). The corpus and crash files cargo-fuzz writes are not kept in
  the repository.
- The URL and header checks of the network sandbox (`HttpsUrl::parse`, `header_refusal`) are crate-private, so a
  target cannot reach them yet; reaching them (a feature that exposes them, or a target inside the crate) is a later
  decision.

## Consequences

- One more CI job, `fuzz`, with a nightly toolchain and a build of Wasmtime with coverage instrumentation. It is not a
  required check until the user makes it one.
- `install` runs about 13 inputs a second (measured locally: 3,000 runs in 220 s, 4 cores, cargo-fuzz's release
  profile, one run), so long fuzzing of it happens outside the pull request checks: locally, or in a scheduled job if
  the user wants one later.
- Bumping the nightly or cargo-fuzz follows the same rules as every pinned version.
