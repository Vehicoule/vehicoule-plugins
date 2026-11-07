# 0006. Verify plugins without the network: tests, 32-bit runs, fuzzing, Kani where it fits, mutants

- Status: Accepted
- Date: 2026-11-07
- Deciders: the user

## Context

- The user asked on 2026-11-07 for fuzzing and formal proofs from the start, and for fuzzing of every external input.
- A plugin's work is to build requests and to read what a service returns. The service is outside the project: its
  answers are untrusted (Vehicoule's [threat model](https://github.com/Vehicoule/Vehicoule/blob/main/docs/threat-model.md),
  "a content provider"), they change without notice, and a test that calls it depends on the network, its rate limits
  and its uptime (Vehicoule's AGENTS.md, rule 7).
- Vehicoule keeps Kani for state machines and for the host's byte-level checks, after string code did not finish
  under Kani 0.68.0 ([ADR 0014](https://github.com/Vehicoule/Vehicoule/blob/main/docs/adr/0014-prove-the-plugin-host-checks-with-kani.md)).
  A plugin has no state machine; most of its code maps a parsed document (JSON) to the contract's types.
- Vehicoule fuzzes with cargo-fuzz 0.13.2 and `libfuzzer-sys` 0.4.13 on `nightly-2026-11-01`, in a workspace of its own,
  for a fixed number of inputs from a fixed seed; the user agreed to libFuzzer's C++ for fuzzing only
  ([ADR 0013](https://github.com/Vehicoule/Vehicoule/blob/main/docs/adr/0013-fuzz-the-plugin-hosts-untrusted-inputs.md)).
- Plugins ship for `wasm32-unknown-unknown`, where `usize` has 32 bits; the tests run natively, on 64 bits.

## Options

1. **Tests on synthetic responses with a fake `fetch`; tests also on 32-bit Linux; every external input fuzzed;
   Kani on hand-written byte parsing; mutation testing everywhere.** No network in any check; a service's change of
   format is found by users or by a later scheduled check, not by pull requests.
2. **The same, plus tests that call the real services on each pull request.** Finds format changes early, but CI
   results then depend on the services, their limits and the runner's address.
3. **Kani on every plugin function.** Proofs on mapping code show little beyond what the types already say, and string
   handling did not finish under Kani in Vehicoule.

## Decision

Option 1, recommended ([`docs/plugin-crates.md`](../plugin-crates.md) gives the details):

- **Tests**: a fake `fetch` answers from responses written in the test, in the shape the service was checked to
  return, with made-up values; no recorded response is committed. Each reader is tested on what must not happen too.
- **32 bits**: `cargo test` also runs on `i686-unknown-linux-gnu`, natively on the x86-64 runner.
- **Fuzzing**: every function that reads a service's response, a text the user typed or a cursor the plugin handed
  out has a cargo-fuzz target in `fuzz/`, a workspace of its own pinned to the same nightly, cargo-fuzz and
  `libfuzzer-sys` versions as Vehicoule's (agreement for libFuzzer's C++ asked again in the first target's pull
  request, since the user gave it for Vehicoule's host only). CI runs each target for a fixed number of inputs from
  a fixed seed; 200,000 is the starting count, Vehicoule's, measured again with the first target.
  `scripts/check-layout.sh` fails a plugin with no target.
- **Kani**: a function that reads bytes by hand gets a proof in a `#[cfg(kani)] mod proofs` beside it; its bound is
  in its code and is never lowered. Mapping code is covered by tests, fuzzing and mutants.
- **Mutation testing**: `cargo mutants` on the workspace; a surviving mutant fails CI.
- **Before the first plugin**: the fuzz and Kani jobs are in the workflow and say that nothing exists yet; they run
  as soon as a target or a proof does, and the Kani job then fails if it finds no proof to run.

## Consequences

- A service that changes its format breaks the plugin for users while CI stays green. A scheduled job that calls
  each service once and compares the shape is a later decision (network in CI, runner addresses, terms of use).
- Synthetic responses must be kept in the service's shape by hand: each plugin's "Service" section (plugin crates,
  section 6) says when that shape was last checked.
