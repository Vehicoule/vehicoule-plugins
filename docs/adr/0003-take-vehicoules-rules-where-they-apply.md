# 0003. Take Vehicoule's rules and foundations, and say which ones do not apply to plugins

- Status: Proposed
- Date: 2026-11-07
- Deciders: the user

## Context

The user asked on 2026-11-07 for this repository to start from the same healthy base as Vehicoule and `nsh-m3e`:
rules for agents, decisions, CI checks, fuzzing and formal proofs, before any plugin is written. Vehicoule's threat
model, accepted on 2026-11-07, already asks for "the same rules as here for `vehicoule-plugins` and
`vehicoule-plugins-extra`: pinned actions, locked dependencies, `cargo deny`" (gap G4).

Some of Vehicoule's decisions are about an app, not about a sandboxed component: copying them unchanged would add rules
nothing can follow, or checks that check nothing.

## Options

1. **Take every Vehicoule ADR and rule as is.** Simple, but some rules would be empty here (the core's architecture,
   platform test matrices), and some would be wrong (no `unsafe` at all, while the generated bindings need it).
2. **Take each one that applies, adapt the ones that need it, and say why the others do not apply.**
3. **Start from nothing.** Loses decisions the user already took.

## Decision

Option 2, recommended. Each Vehicoule decision, and what it becomes here:

| Vehicoule | Here | Why |
|---|---|---|
| ADR 0001, Rust only | Applies ([ADR 0001](0001-write-plugins-in-rust-as-wasm-components.md)) | |
| ADR 0002, CI is the only truth | Applies: AGENTS.md rule 1; `main` protected, CI required | Same reasons: agents trusted local runs and documents |
| ADR 0003, core as synchronous state machines | Does not apply; replaced by AGENTS.md rule 4 | A plugin has no state to manage: each call starts from nothing and reaches only the host. The host and the core are Vehicoule's |
| ADR 0004, types, Kani, property tests, mutation testing | Adapted ([ADR 0006](0006-verify-plugins-without-the-network.md)) | Kani was kept for state machines, which plugins do not have; here it goes to hand-written byte parsing |
| ADR 0005, pinned versions and dependencies | Applies; every plugin dependency counts as exposed to untrusted content (AGENTS.md rule 11) | A plugin parses what a service returns |
| ADR 0006, GPL-3.0-only | Proposed separately ([ADR 0004](0004-license-the-plugins-under-gpl-3.0.md)) | A license is the user's choice |
| ADR 0007, the plugin host | Applies as the frame of every plugin ([ADR 0001](0001-write-plugins-in-rust-as-wasm-components.md)) | |
| ADR 0008, app text in Fluent files | Does not apply as is; replaced by AGENTS.md rule 16 | A plugin shows no text: its name is the service's, its errors and logs are diagnostics, and Vehicoule's ADR 0008 keeps plugin text out of its translation files |
| ADR 0009, REUSE | Applies: `REUSE.toml` declares every path, `reuse lint` 6.3.0 in CI, copyright "The Vehicoule contributors" | |
| ADR 0011, Contributor Covenant 3.0 | Applies: same text and reporting address | |
| ADR 0012, one regression test per fixed bug | Applies, with this repository's issues | |
| ADR 0013, fuzz untrusted inputs | Adapted ([ADR 0006](0006-verify-plugins-without-the-network.md)): every input a plugin reads from outside | Service responses are a plugin's main input |
| ADR 0014, Kani on the host's checks | Adapted, as for ADR 0004 | |

And the CI of Vehicoule:

| Vehicoule's CI | Here | Why |
|---|---|---|
| Format, Clippy, tests, private docs, `-D warnings`, no debug information, caches saved by `main` only, actions pinned by SHA, `persist-credentials: false` | Applies | |
| Clippy | Also for `wasm32-unknown-unknown` | The `bindings` glue is compiled only for Wasm |
| Tests on macOS and Windows | Does not apply | A plugin's bytes are the same on every platform and run in Wasmtime; how Wasmtime runs on each platform is the host's to test, in Vehicoule |
| Tests on 32-bit Linux | Applies | The plugins ship for a 32-bit target (ADR 0006) |
| Mutation testing, sharded | Applies, unsharded while the code is small | Vehicoule sharded it after one runner took 10 to 19 minutes |
| Kani, sharded | Applies once a proof exists ([ADR 0006](0006-verify-plugins-without-the-network.md)) | |
| Fuzzing | Applies once a target exists ([ADR 0006](0006-verify-plugins-without-the-network.md)) | |
| `cargo deny`, `typos`, `cargo machete`, `zizmor`, REUSE | Applies; `cargo deny` installed at a pinned version rather than through its action | One way to install every tool, which also runs outside GitHub |
| Parsing `wit/` with `wasm-tools` | Replaced by `scripts/sync-contract.sh check` ([ADR 0002](0002-pin-the-contract-and-check-plugins-in-the-real-host.md)) | The copy is checked to be Vehicoule's, which parses it; building the template parses it again |
| Building the host's test plugins | Replaced by `scripts/build-plugins.sh` and the conformance test | |

## Consequences

- `vehicoule-plugins-extra` has the same ADR; a change to one is made to the other, unless it is about mirroring,
  which only that repository does.
- AGENTS.md carries Vehicoule's rules, adapted, and three of its own: everything read from outside is fuzzed (15), a
  plugin's text is never the app's (16), a plugin reports and the core decides (17).
- A Vehicoule decision taken later does not apply here by itself: it comes with its own ADR here, or a line in this
  table through a new ADR.
