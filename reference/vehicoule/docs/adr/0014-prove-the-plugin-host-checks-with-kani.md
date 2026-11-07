# 0014. Prove the plugin host's sandbox checks with Kani

- Status: Accepted
- Date: 2026-11-07
- Deciders: the user

## Context

[ADR 0004](0004-verification.md) keeps Kani for the core's state machines. A plugin is untrusted code
([ADR 0007](0007-plugin-host.md)): what stands between it and the network or the memory is a handful of checks in
`crates/vehicoule-plugin-host`, each a small pure function:

- `HttpsUrl::parse` (`src/network.rs`): only `https`, no port, no credentials, a host name and nothing else in the
  authority. The host it keeps is the one matched against the manifest and the one the request goes to.
- `HostPattern::matches` (`src/manifest.rs`): `api.example.org` matches itself only; `*.cdn.example.org` matches its
  subdomains only, never `cdn.example.org` itself.
- `header_refusal` (`src/network.rs`): no `Host` header, no control character but tab in a value.
- `MemoryLimiter::allow` (`src/host.rs`): the bytes of every memory and table of a call, summed, stay under the
  ceiling.

They are covered by unit tests, property tests, tests against real Wasm plugins and mutation testing. A property test
samples inputs; it cannot show that no input of a given size gets through. Fuzzing of the plugin host's public entry
points is decided by [ADR 0013](0013-fuzz-the-plugin-hosts-untrusted-inputs.md); the URL and header checks are
crate-private, out of its reach.

On 2026-11-06 the user chose, among "property tests only", "Kani on the security checks" and "Kani and fuzzing of
install", the third. ADR 0013 covers the fuzzing; this ADR covers the proofs.

Kani 0.68.0, the version the Kani job pins, did not finish on the checks as first written: a harness on
the `https://` scheme check of `HttpsUrl::parse` with 2 bytes after the scheme did not finish in 8 minutes, and one on
the host-name check with a 4-byte name ran out of memory after about 5 minutes (local container, one run each). Both
went through `str` searches (`find`, `split`, `strip_prefix` on `&str`) and
`std::str::from_utf8`, whose loops CBMC unrolls over every byte position. The same checks written as one pass over the
bytes proved in seconds to minutes. On 2026-11-07 the user chose, among "rewrite the checks as plain byte loops",
"prove only the memory limiter" and "lower the bounds until the string proofs finish", the first.

## Options

1. **Rewrite the three text checks as single passes over bytes**, keep their behavior, and prove them. The public API
   does not change; property tests check the rewritten code against the spec's rules written the plain way.
2. **Prove only `MemoryLimiter::allow`**, which has no text. The three checks that guard the network keep only their
   tests.
3. **Keep the `str` code and lower the bounds** until the proofs finish. Bounds of a few bytes prove almost nothing
   about a host name.

## Decision

Option 1, chosen by the user on 2026-11-07:

- `HttpsUrl::parse`, `HostPattern::matches` and `header_refusal` do their checks in byte functions
  (`UrlParts::of`, `pattern_matches`, `header_refusal_bytes`, `are_labels`) that use no `str` search and no
  `from_utf8`. The `&str` functions keep their signatures and call them.
- Each check gets a Kani proof in a `#[cfg(kani)] mod proofs` beside it, run by the Kani job of ADR 0004, stating
  what must hold and what must not:
  - `HttpsUrl::parse`: a URL it keeps starts with `https://`, then its host, then `/`; its host is a valid host name;
    it holds no `\`, no `#`, and no `@` or `:` before the path. Every other URL is refused.
  - `HostPattern::matches`, ignoring ASCII case: an exact pattern matches its own name and no other; `*.d` matches a
    name if and only if the name is one or more valid labels, a dot, then `d`. Never `d` itself, never `xd`.
  - The host-name check: a name it accepts is dot-separated labels of 1 to 63 bytes of `[a-z0-9-]`, no label starting
    or ending with `-`, the last label holding a letter.
  - `header_refusal`: an accepted header has a non-empty token name that is not `host` in any case, and a value
    without a control character but tab.
  - `MemoryLimiter::allow`: whatever the sequence of growths, the count never exceeds the ceiling and never wraps; a
    refused growth is not counted and sets `denied`.
- Each harness states its bound (the input length) in its code. Property tests keep covering longer inputs, against
  a reference written with `str` methods.
- A new check that guards the sandbox gets its proof in the pull request that adds it, written so Kani can prove it.

Not covered, on purpose: Wasmtime itself (too large for Kani; this repository relies on its LTS release and its own
fuzzing), and third-party plugins (the sandbox bounds them; their code is not ours to prove).

## Consequences

- The Kani job gains the plugin host's harnesses. They are listed and sharded like the core's, with no change to the
  workflow.
- The text checks are lower level than the `str` code they replace. The property tests against the plain reference are
  what keep them readable as the spec's rules.
- A bound proves every input up to its length only. Raising one is welcome; lowering one is weakening a check
  (AGENTS.md rule 2).
