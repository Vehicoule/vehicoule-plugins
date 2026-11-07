# 0004. License the plugins under GPL-3.0

- Status: Accepted
- Date: 2026-11-07
- Deciders: the user

## Context

- Vehicoule is `GPL-3.0-only` (its [ADR 0006](https://github.com/Vehicoule/Vehicoule/blob/main/docs/adr/0006-license.md));
  `nsh-m3e` is `MPL-2.0` (its ADR 0007).
- The contract, `wit/plugin.wit`, is declared `GPL-3.0-only` in Vehicoule's `REUSE.toml`. This repository holds a copy
  of it, and every plugin's bindings are generated from it.
- Whether a component that only implements an interface is a work based on the program that loads it is a legal
  question this ADR does not answer. Under the same license, the question does not arise for the plugins written here.
- `vehicoule-plugins-extra` has the same ADR, with the same recommendation, so both repositories stay under one license.
- Third parties may want to write plugins under another license. That depends on the contract's license, which is
  decided in Vehicoule, not here.

## Options

1. **`GPL-3.0-only`, as the app.** One license for the app, its contract and its plugins; the copy of `wit/` keeps its
   license. Anyone who forks a plugin keeps it free.
2. **A permissive license (`MIT OR Apache-2.0`).** Plugins can be reused anywhere, including in closed apps. The copy
   of `wit/` stays `GPL-3.0-only`, so the repository holds two licenses and generated bindings derive from a GPL file.
3. **`MPL-2.0`, as `nsh-m3e`.** File-level copyleft. Same remark on the copy of `wit/`.

## Decision

Recommended: option 1, `GPL-3.0-only`. The files are written with it while this ADR is proposed; choosing another
option changes `LICENSE`, `Cargo.toml` and `REUSE.toml` in the same pull request.

## Consequences

- `LICENSE` holds the GPL-3.0 text; `Cargo.toml` declares `license = "GPL-3.0-only"`.
- Every dependency must be GPL-3.0 compatible; `deny.toml` lists the allowed licenses, as Vehicoule's does.
- Opening the contract to plugins under other licenses is a decision for Vehicoule's `wit/`.
