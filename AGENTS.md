# AGENTS.md

`vehicoule-plugins` holds the content plugins of [Vehicoule](https://github.com/Vehicoule/Vehicoule), a music, video and
books app that ships no content source of its own. A plugin is a WebAssembly component, written in Rust, that the app's
plugin host runs in a sandbox: it provides service-dependent capabilities (search, metadata, artwork, lyrics, link
import, ...) for one service, reaches the network only through the host and only to the hosts it declares, and keeps
nothing between calls. The contract between the host and the plugins lives in Vehicoule; this repository builds
against a pinned copy of it. The plugins planned here are `deezer`, `itunes` and `lrclib` (Vehicoule,
[`docs/plugin-providers.md`](https://github.com/Vehicoule/Vehicoule/blob/main/docs/plugin-providers.md)); none is
written yet. Plugins that may be the target of takedown requests (`ytmusic`) live in
[`vehicoule-plugins-extra`](https://github.com/Vehicoule/vehicoule-plugins-extra), under the same rules.

This file is the rule book. Every agent, human or AI, reads it before touching the repository, and follows it over
any habit, skill or training data. When a rule here conflicts with another document, this file wins; when it
conflicts with what the user says in the task, the user wins and the rule gets an ADR.

## What is true

The only truth is what the code does, shown by tests that run in CI. Not a document, not a plan, not a local run.

The first version of the app was written by agents that invented APIs, skipped tests and loosened checks to get green.
Its code is gone on purpose, its plugins included. Its notes survive as a list of ideas and lessons, often wrong or
contradictory: never copy a value, an endpoint, a design or an API from them, and never cite them as a source. A
requirement enters this repository by being written into a spec or an ADR that the user accepts, and is true once a
test in CI shows it.

## Read first, in this order

1. This file.
2. The decisions: [`docs/adr/`](docs/adr/). They say what is settled and why. Do not reopen a decision without a new
   ADR that the user accepts.
3. How a plugin crate is laid out, tested, fuzzed and proved: [`docs/plugin-crates.md`](docs/plugin-crates.md).
4. The contract and the sandbox, in Vehicoule at the commit this repository pins (ADR 0002): the
   [plugin host spec](https://github.com/Vehicoule/Vehicoule/blob/main/docs/plugin-host-spec.md), the
   [plugin providers](https://github.com/Vehicoule/Vehicoule/blob/main/docs/plugin-providers.md) and the
   [threat model](https://github.com/Vehicoule/Vehicoule/blob/main/docs/threat-model.md). The copy of the contract
   itself is [`wit/plugin.wit`](wit/plugin.wit).
5. For Rust style: the `rust-best-practices` and `rust-coding-guidelines` skills in [`.claude/skills/`](.claude/skills/),
   and [`reference/rust/google-rust-style-guide.md`](reference/rust/google-rust-style-guide.md).

[`docs/references.md`](docs/references.md) lists every source with its pinned version.

## Non-negotiable rules

1. **CI is the only truth.** Nothing reaches `main` without a pull request whose CI is green. A local run is a
   convenience, never evidence. Never push to `main`.
2. **Never weaken a check to get green.** No lowered lint level, no `#[allow]` without an ADR, no skipped or ignored
   test, no Kani proof removed or bounded lower, no fuzz target removed or run fewer times, no reduced scope. If a
   check is wrong, say so and stop: changing a check is the user's decision.
3. **Never invent an API or a value.** Look a crate's name up in its source at the pinned version before using it.
   Every endpoint, field, header, status code and limit of a service that a plugin relies on is checked against the
   service (one real call, or its documentation) and written down with the date and how it was checked
   ([`docs/plugin-crates.md`](docs/plugin-crates.md), section 6). If something cannot be found, write "not found" and
   stop. Do not write it by analogy with another service, and do not take it from the first version's notes.
4. **A plugin reaches nothing but the host, and keeps nothing.** It imports only the contract's `http`, `log` and
   `types`: no WASI, no file, no clock, no randomness, no thread. Each call starts from nothing: no state survives
   between calls, and nothing in the plugin pretends otherwise (no cache, no global). The host enforces this
   (Vehicoule's [ADR 0007](https://github.com/Vehicoule/Vehicoule/blob/main/docs/adr/0007-plugin-host.md)); a plugin
   that tries fails the conformance test (ADR 0002).
5. **Rust only.** No JavaScript, no other language compiled to Wasm (ADR 0001).
6. **The contract is Vehicoule's.** [`wit/`](wit/) is a copy pinned to a Vehicoule commit, never edited by hand: a
   change to the contract starts in Vehicoule, with its host and tests, and arrives here through
   `scripts/sync-contract.sh update` in a pull request of its own (ADR 0002).
7. **A test proves what must happen and what must not.** A test that only confirms the intended action proves little:
   a parser test also feeds what the service may send wrongly (a missing field, a wrong type, an empty list, an
   error status) and checks the plugin's answer. Never change a test to make it pass: a changed or deleted assertion
   is listed and justified in the pull request. No test reaches the network, depends on real time or on the machine's
   load.
8. **Every number has a source.** A limit cites the service's documentation or the call that showed it. A
   measurement says what was measured, where, in which build profile, and how many runs. If something was not
   measured, write "not measured" and the technical reason.
9. **No panics in plugin code.** No `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, indexing that can fail or
   arithmetic that can overflow outside tests; in Wasm a panic is a trap, and the call fails. No `unsafe`, except in
   the `bindings` module that `wit-bindgen` generates (ADR 0005). Errors are typed.
10. **Every public item is documented**, with a doc test when it has a usage. `missing_docs` is an error.
11. **Dependencies are rare.** A new dependency is declared once in the root `Cargo.toml`, builds for
    `wasm32-unknown-unknown` without WASI, passes `cargo deny`, and its pull request says in one line why it is needed
    and what was considered instead. Every dependency of a plugin handles untrusted content (what a service returns),
    so each one, and any written in C, needs the user's explicit agreement (Vehicoule's
    [ADR 0005](https://github.com/Vehicoule/Vehicoule/blob/main/docs/adr/0005-pinned-versions-and-dependencies.md)).
12. **One pull request does one thing.** One plugin, or one change to the tooling; no drive-by refactors, renames or
    formatting.
13. **Decisions belong to the user.** Anything that changes the architecture, the scope, a license, a dependency
    policy, which plugin provides what, or a rule here is written as an ADR with options and a recommendation, and
    waits for the user.
14. **Do not guess the environment.** Use the pinned toolchain and versions. If a tool is missing or a command fails,
    report the exact output instead of working around it.
15. **Everything a plugin reads from outside is fuzzed.** A service's response, a text the user typed (a query, a
    link) and a cursor the plugin handed out and gets back are untrusted input: each function that reads one has a
    fuzz target, run in CI for a fixed number of inputs (ADR 0006). `scripts/check-layout.sh` fails a plugin without
    any.
16. **A plugin's text is never the app's text.** The manifest's `name` is the service's own name; errors and log
    messages are English diagnostics for developers, never written to be shown to the user. Every text the app shows
    lives in the app's translation files (Vehicoule's
    [ADR 0008](https://github.com/Vehicoule/Vehicoule/blob/main/docs/adr/0008-app-text-in-translation-files.md)).
17. **A plugin reports, the core decides.** A plugin never retries a request, never waits, never falls back to another
    service or another endpoint to hide a failure: it returns the contract's error (`rate-limited` with the
    service's own retry delay, `upstream`, `not-found`, ...) and the app's core decides what to do (Vehicoule's
    [plugin host spec](https://github.com/Vehicoule/Vehicoule/blob/main/docs/plugin-host-spec.md), sections 1 and 7).

## Commands

The same commands run in CI ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --locked --target wasm32-unknown-unknown -- -D warnings
cargo test --workspace --locked
cargo test --workspace --locked --target i686-unknown-linux-gnu   # 32-bit, as the plugins ship (ADR 0006)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items --locked
scripts/check-layout.sh
scripts/sync-contract.sh check                    # wit/ is Vehicoule's at the pinned commit
scripts/build-plugins.sh "$OUT"                    # wasm-tools 1.263.0
VEHICOULE_PLUGINS="$OUT" cargo test --manifest-path conformance/Cargo.toml --locked
cargo deny check                                   # cargo-deny 0.20.2
cargo deny --manifest-path conformance/Cargo.toml check
cargo mutants --workspace --cargo-arg=--locked     # cargo-mutants 27.1.0
scripts/prove.sh                                   # Kani 0.68.0, after `cargo kani setup`; every proof (ADR 0006)
typos                                              # typos-cli 1.52.0
cargo machete                                      # cargo-machete 0.9.2, no unused dependency
zizmor .github/workflows/                          # zizmor 1.32.0, the workflows' security audit
reuse lint                                         # reuse 6.3.0 (ADR 0003)
scripts/fuzz.sh                                    # cargo-fuzz 0.13.2, fuzz/'s pinned nightly; every target (ADR 0006)
```

`scripts/prove.sh` and `scripts/fuzz.sh` say so and succeed while no proof or fuzz target exists; they run as soon as
one does.

The toolchain is pinned in [`rust-toolchain.toml`](rust-toolchain.toml). Run the full list before opening a pull
request, and report what you ran.

## Layout

| Path | What it holds |
|---|---|
| `plugins/<id>/` | One plugin per crate, named after its manifest id; none yet ([layout](docs/plugin-crates.md)) |
| `template/` | The crate a new plugin starts from; built and installed in CI like a plugin, never published |
| `wit/` | The plugin contract, a copy pinned to a Vehicoule commit; never edited by hand (ADR 0002) |
| `conformance/` | Installs every built plugin in Vehicoule's real plugin host, at the same commit (ADR 0002) |
| `fuzz/` | cargo-fuzz targets, a workspace of its own on a pinned nightly; arrives with the first target (ADR 0006) |
| `scripts/` | `build-plugins.sh`, `sync-contract.sh`, `check-layout.sh`, `prove.sh`, `fuzz.sh`: what CI runs, runnable anywhere |
| `docs/adr/` | Decisions, numbered, one per file ([template](docs/adr/0000-template.md)) |
| `docs/` | How plugin crates are written, and references |
| `reference/` | Vendored sources of truth, pinned, never edited by hand |
| `.claude/skills/` | Vendored skills for coding agents, pinned |

## Conventions

- Everything in the repository is written in English: code, comments, docs, commit messages. Comments say why, not
  what.
- Commit subjects and pull request titles: `<area>: <What the change does>`, imperative, capitalized, no trailing
  period, about 72 characters (`deezer: Search tracks, albums and artists`). Areas: a plugin's id (`deezer`,
  `lrclib`), `template`, `contract` (a bump of `wit/` and the pinned host), `conformance`, `fuzz`, `docs`, `ci`,
  `deps`.
- The body explains why. A pull request that adds or changes a plugin lists every capability, declared host and
  service endpoint it uses under `## Plugin`.
- Public structs are `#[non_exhaustive]` and implement `Default` when a default makes sense. Public enums are
  `#[non_exhaustive]`. A plugin's public items exist for its tests, fuzz targets and proofs
  ([`docs/plugin-crates.md`](docs/plugin-crates.md), section 2).
- States are enums, never several booleans. Invalid states are unrepresentable.
- A pull request that fixes a bug adds a test that fails without the fix, named `regression_<issue>_<what>` with a
  link to this repository's issue above it (Vehicoule's
  [ADR 0012](https://github.com/Vehicoule/Vehicoule/blob/main/docs/adr/0012-one-regression-test-per-fixed-bug.md)).
  A crash found by fuzzing is such a bug.
- Durations are `std::time::Duration` inside a plugin and become the contract's `-ms` fields only at the boundary.
