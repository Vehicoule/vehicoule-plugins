# 0002. Pin the contract to a Vehicoule commit, and install every plugin in that commit's host

- Status: Accepted
- Date: 2026-11-07
- Deciders: the user

## Context

- The contract (`wit/plugin.wit`, package `vehicoule:plugin`) lives in Vehicoule, next to the host, "so a contract
  change and its host change are one pull request"; this repository builds "against a copy of `wit/` pinned to a
  Vehicoule commit and checked by a sync script, as `nsh-m3e` does for NSH's skills" (Vehicoule's
  [plugin host spec](https://github.com/Vehicoule/Vehicoule/blob/main/docs/plugin-host-spec.md), section 2, accepted
  by the user on 2026-11-06). The plugins repository bumps its copy "in a pull request of its own" (section 8).
- `nsh-m3e`'s `scripts/sync-nsh.sh` keeps vendored files on the commit its `Cargo.toml` pins, with a `check` mode run
  in CI and an `update` mode.
- At install, the host checks the size, compiles, checks every import, calls `describe` with every request refused,
  validates the manifest and matches the declared capabilities against the exports (spec, sections 3 to 5). Its
  public API, `PluginHost::new` and `PluginHost::install`, takes a `Network` the caller writes, and its own tests use
  a fake one (Vehicoule, `crates/vehicoule-plugin-host` at `4f61277`).
- A plugin that compiles against the contract can still be refused by the host: a manifest id with an upper-case
  letter, a host pattern like `*`, a capability declared but not exported. Only the host's checks say so; copying
  them here would make a second owner of the rules that can drift (Vehicoule's AGENTS.md, rule 6).

## Options

1. **Copy of `wit/` checked by a script; nothing else.** Plugins build against the right contract, but a plugin the
   app would refuse is only found by installing it in the app.
2. **Copy of `wit/` and a conformance test that installs every built plugin in Vehicoule's host, pinned by Cargo to
   the same commit as the copy.** One pin for both, in `conformance/Cargo.toml`; the script checks that `wit/` is that
   commit's. Costs a build of Wasmtime in CI (cached from `main`), in a workspace of its own so the plugins never link
   it.
3. **Re-implement the manifest rules in this repository.** A second copy of rules that already have an owner.

## Decision

Option 2, recommended:

- `wit/` is a copy of Vehicoule's `wit/`, never edited by hand.
- `conformance/` is a workspace of its own that depends on `vehicoule-plugin-host` by Git at one Vehicoule commit. Its
  test installs every component that `scripts/build-plugins.sh` built, the template included, with a network that
  never opens a socket, and checks that each installs, under the id that is its file name, with no request reaching
  the network. A missing or empty output directory fails it.
- `scripts/sync-contract.sh check` (CI) fails when `wit/` differs from Vehicoule's `wit/` at that commit;
  `scripts/sync-contract.sh update [REV]` moves the pin, copies `wit/` and refreshes `conformance/Cargo.lock`.
- A bump is a pull request of its own (area `contract`), green only when every plugin still builds and installs.

## Consequences

- A plugin the host would refuse fails CI here, with the host's own error, before any feed.
- The budgets the conformance test gives the host are arbitrary and generous, as in the host's tests: the app's real
  budgets come from measurements (spec, section 9), so passing here does not show that a plugin fits them.
- `deny.toml` allows the Vehicoule Git repository as a source, for `conformance/` only.
- The host's API is not stable (`0.0.0`): a bump may need the conformance test changed, in the same pull request.
- Calling capabilities through the host, with a fake network serving synthetic responses, is added with the first
  plugin, once the host calls the capabilities it provides.
- No scheduled job opens bumps yet; one can be added like `nsh-m3e`'s `nsh-bump.yml`, as its own decision.
