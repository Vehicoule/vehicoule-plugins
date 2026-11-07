# Plugin crates: layout, tests, fuzzing and proofs

- Status: Accepted
- Scope: how every plugin of this repository is written, so that the first one (`deezer`, `itunes` or `lrclib`) and
  every later one look the same and are checked the same way. What a plugin provides is decided in Vehicoule
  ([plugin providers](https://github.com/Vehicoule/Vehicoule/blob/main/docs/plugin-providers.md)); what the host
  enforces, in its [spec](https://github.com/Vehicoule/Vehicoule/blob/main/docs/plugin-host-spec.md).
- Rules: [AGENTS.md](../AGENTS.md), [ADR 0002](adr/0002-pin-the-contract-and-check-plugins-in-the-real-host.md),
  [ADR 0005](adr/0005-allow-unsafe-only-in-the-generated-bindings.md),
  [ADR 0006](adr/0006-verify-plugins-without-the-network.md).

Every "must" below is checked by CI or by review; the ones CI checks name the check.

## 1. Where a plugin lives

- `plugins/<id>/`, where `<id>` is the plugin's manifest id (lowercase ASCII letters, digits and `-`). The package is
  `vehicoule-plugin-<id>` (`scripts/check-layout.sh`).
- `scripts/build-plugins.sh` builds it to `<id>.wasm`, and the conformance test installs it in Vehicoule's host and
  checks that the manifest id is the file name (ADR 0002). Three names, one id.
- A new plugin starts as a copy of [`template/`](../template/); the first plugin also adds `"plugins/*"` to the
  workspace members in the root `Cargo.toml`.

## 2. What the crate holds

- `crate-type = ["cdylib", "rlib"]`: the `cdylib` is the Wasm module turned into a component; the `rlib` lets the
  tests, the fuzz targets and the Kani proofs reach the plugin's code. Items the fuzz targets need are `pub`, in a
  `pub mod`, documented like any public item; nothing else is public.
- A `bindings` module with nothing but the `wit_bindgen::generate!` call for the plugin's world and an `export!` for
  Wasm only. It is the only place `unsafe` is allowed (ADR 0005). The world is the capability worlds the plugin
  provides (`search-plugin`, `metadata-plugin`, ...), or a world of its own that includes them
  ([spec](https://github.com/Vehicoule/Vehicoule/blob/main/docs/plugin-host-spec.md), section 8). Exporting a
  capability the manifest does not declare, or the reverse, fails the conformance test.
- The plugin's own code, in plain Rust modules, split in three kinds of function:
  - **Requests**: build the `http` request for a call (URL, query string, headers) from the call's arguments. Pure.
  - **Readers**: turn a response (status, headers, body) into the contract's value or error. Pure; each one is a
    fuzz target (section 4).
  - **Calls**: the contract's functions; they build a request, send it through a `fetch` function they are given,
    and read the response. The `bindings` glue passes the host's `http::fetch`; the tests pass a fake (section 3).
    Natively the host's import does not exist, so nothing native can call it by mistake.
- No `static` with interior mutability, no `thread_local!`, no cache: a call starts from nothing (AGENTS.md rule 4).

## 3. Tests

- **No network.** A test gives the call a fake `fetch` that checks each request it receives and answers it from a
  response written in the test. A test also asserts the requests that must not happen: none to another host, no
  second request after an error, no retry.
- **Responses are written by us, in the service's shape.** A response used by a test is synthetic: the structure the
  service was checked to return (section 6), with made-up values. A recorded response is never committed: it is the
  service's data, under its terms, and may hold personal data.
- **What must not happen.** For each reader: a missing field, a field of the wrong type, an empty list, an error
  status (`404`, `429` with and without `Retry-After`, `5xx`), a body that is not the expected format, and a value
  past what the contract can carry. Each gives the contract's error, never a panic, never a made-up default.
- **32 bits.** The tests also run on `i686-unknown-linux-gnu`, because the plugins ship for `wasm32-unknown-unknown`,
  where `usize` has 32 bits (ADR 0006).
- **Mutation testing** runs on every crate; a surviving mutant fails CI.
- **Conformance.** CI builds every plugin and installs it in the real host (ADR 0002). Calling a capability through
  the host, with a fake network serving the same synthetic responses, comes with the first plugin, once the host
  calls every capability it provides.

## 4. Fuzzing

- The targets live in `fuzz/`, a workspace of its own on a pinned nightly, as in Vehicoule
  ([ADR 0013](https://github.com/Vehicoule/Vehicoule/blob/main/docs/adr/0013-fuzz-the-plugin-hosts-untrusted-inputs.md)).
  The first plugin creates it.
- Each target is named `<id>_<what>` (`deezer_search_response`) and calls one reader, or one function that reads a
  query, a link or a cursor, with arbitrary bytes. It asserts: no panic, and what the reader returns respects the
  contract's rules it is responsible for (for example: every artwork URL it returns is `https`).
- Every plugin has at least one target (`scripts/check-layout.sh`). CI builds them and runs each for a fixed number
  of inputs from a fixed seed, never for a time.
- A crash found by fuzzing gets an issue and a regression test (AGENTS.md, "Conventions").

## 5. Kani

A plugin that reads bytes by hand, without a parsing crate (a cursor it encodes itself, a header value, a custom text
format), proves that function with Kani in a `#[cfg(kani)] mod proofs` beside it: never a panic, and the property the
function exists for, for every input up to the bound the harness states. Code that only maps a parsed document to the
contract's types is covered by tests, fuzzing and mutation testing instead (ADR 0006).

## 6. What was checked about the service

Each plugin's crate documentation (`//!` in `src/lib.rs`) has a section "Service" that lists every endpoint, field,
header, status code and limit the plugin relies on, each with the date it was checked and how (one call from where,
or which page of the documentation), as Vehicoule's
[plugin providers](https://github.com/Vehicoule/Vehicoule/blob/main/docs/plugin-providers.md) does in its section 4.
What was not checked says "not checked" and why. A finding that changes which plugin provides what goes back to
Vehicoule first.

## 7. Size and budgets

The host bounds a plugin's size, fuel, memory, requests and output; the values come from measurements of real plugins
([spec](https://github.com/Vehicoule/Vehicoule/blob/main/docs/plugin-host-spec.md), sections 6 and 9). The pull
request of each plugin reports the size of its component in the release profile, as `scripts/build-plugins.sh` builds
it.
