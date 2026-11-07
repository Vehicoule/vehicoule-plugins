# 0007. Run content plugins as Wasm components in Wasmtime, with no WASI

- Status: Accepted
- Date: 2026-11-06
- Deciders: the user

## Context

Vehicoule ships no content source: every service-dependent capability (search, metadata, resolution of a track to a
stream, radio candidates, artwork, lyrics, link import) comes from a plugin installed from a signed feed. The user
accepted on 2026-11-06 the split between three layers: the core decides (sessions, queue, routing between plugins,
user corrections), the edges execute (plugin host, network, storage, players), the plugins provide the
service-dependent capabilities and nothing else, with no file access, no direct network access and no user state.
[ADR 0003](0003-core-architecture.md) keeps Wasm for content plugins and out of the core.

A plugin is untrusted code: anyone can publish a feed. The host must therefore bound what a plugin can reach (only
the HTTP hosts it declares), how long it runs, how much memory it takes and how much it returns, and must survive any
plugin failing.

Facts, read on 2026-11-06:

- The contract between host and plugins is meant to be written in WIT, the interface language of the WebAssembly
  component model. Wasmtime implements the component model at tier 1 (`docs/stability-tiers.md` of
  `bytecodealliance/wasmtime`, tag `v50.0.1`). `wasmi` 2.0.0, the main pure-Rust interpreter, lists no component model
  support in its README; its only embedding interface is WASI preview 1.
- `wasmer` 7.5.0, the latest stable on crates.io (MIT), has no component model: the word "component" appears nowhere
  in its source. Its README calls its interpreter experimental; its `v8` backend vendors V8's C++ (`third-party/wee8`).
- Wasmtime is Rust, licensed `Apache-2.0 WITH LLVM-exception` (allowed by `deny.toml`). On Unix with Cranelift, its
  build script compiles one 67-line C file, `src/runtime/vm/helpers.c` (weak libunwind symbols), with `cc`
  (`build.rs` of `wasmtime` 50.0.1).
- Its targets `aarch64-linux-android` and `aarch64-apple-ios` are tier 3 ("CI testing, full-time maintainer" missing);
  `aarch64-apple-darwin` is tier 2; Pulley, its interpreter, is tier 2 (same file). Cranelift needs executable memory
  created at run time; where that is not allowed (iOS), Wasmtime runs Pulley bytecode
  (`docs/stability-platform-support.md`).
- Wasmtime releases a major version on the 20th of every month; every twelfth is an LTS supported for 24 months with
  guaranteed security backports, the others for 2 months (`docs/stability-release.md`). 48.0.0, the current LTS, was
  released on 2026-08-20 and needs Rust 1.95 (crates.io); 50.0.1 is the latest stable.
- Loading precompiled code (`Component::deserialize`) is an `unsafe fn` in Wasmtime 50.0.1
  (`src/runtime/component/component.rs`), and this workspace forbids `unsafe`. Compiling from the Wasm bytes
  (`Component::new`) is safe.
- `wasmtime-wasi` 50.0.1 depends on `tokio` with the `rt-multi-thread` and `net` features (its `Cargo.toml`).
- The first app's Rust core reached 19.9 MB, mostly Wasmtime, Cranelift and Sentry ([ADR 0005](0005-pinned-versions-and-dependencies.md)).
  The size of a host built with only the features below is not measured yet: no such build exists.

## Options

### Runtime

1. **Wasmtime with the component model.** WIT contract, typed bindings generated on both sides, fuel and epoch
   interruption to bound time, store limits to bound memory. Costs binary size, a tiny C helper at build time, and
   tier 3 support on Android and iOS.
2. **A core-module interpreter (`wasmi`) with a hand-written ABI.** Pure Rust, smaller, deterministic fuel. No
   component model: the contract becomes pointers and lengths we encode ourselves, on both sides, for every type.
3. **Wasmer with a hand-written ABI.** Same cost as option 2 for the contract (no component model), with Cranelift
   or LLVM as compilers; its interpreter is experimental, which leaves iOS without a supported path. Its binary size
   is not measured.
4. **No Wasm: plugins as native libraries or scripts.** Rejected by the sandbox requirement and by ADR 0001 (no
   JavaScript engine).

### What a plugin may import

1. **Only Vehicoule's own interfaces, no WASI.** The plugin's imports are a closed list we write (HTTP through the
   host, logging, the clock). A component that imports anything else fails to instantiate. Plugins build for
   `wasm32-unknown-unknown` and are turned into components; crates that need randomness or the file system do not
   work there, which is the point.
2. **WASI preview 2 with an empty context.** Plugins build for `wasm32-wasip2` with the usual tooling; the host links
   `wasmtime-wasi` with no directory, no socket and no environment. Brings `tokio` with networking into the host, and
   the sandbox depends on configuring a large surface correctly instead of not having it.

### Version

1. **Follow the LTS (48 now).** Security fixes for 24 months, one deliberate bump a year.
2. **Follow the latest release.** Twelve major bumps a year, each possibly breaking the API.

## Decision

Wasmtime with the component model, no WASI, pinned to the LTS (48.x), with `default-features = false` and only the
features the host uses (`runtime`, `component-model`, `cranelift`; `pulley` added for iOS when iOS starts). The user
agreed on 2026-11-06 to this dependency, which is exposed to untrusted content and builds a small C file (AGENTS.md
rule 11, ADR 0005), after comparing it with Wasmer and with Wasmtime's non-LTS releases, whose support tiers for
Android and iOS are the same (`docs/stability-tiers.md` at `v48.0.0`, `v50.0.1`, `v51.0.0-rc.1`). The user left the
choice of WASI to Claude, who took option 1.

## Consequences

- The host is an edge crate. The core never imports it; the core sends plugin calls as commands and receives their
  results as events.
- The contract is a WIT package versioned with semver; the plugins repository builds against a pinned copy of it.
- Every compile happens on the device from the plugin's bytes. Keeping compiled code across launches needs either
  Wasmtime's `cache` feature or an `unsafe` exception; it is decided by a new ADR once compile time is measured on the
  Xiaomi 14 and the Mac.
- Android and iOS support relies on Wasmtime's tier 3; our own CI must build and test the host for those targets
  before a release on them.
- The host's binary size is measured in the first host pull request (release profile, each target) and reported with
  the method; a size budget, if any, is its own ADR.
- On macOS, whether the hardened runtime needs a JIT entitlement for Cranelift is checked when the macOS shell is
  signed; not checked yet.
