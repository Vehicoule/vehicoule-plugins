# Threat model: plugins

- Status: Accepted by the user on 2026-11-07
- Scope: what a plugin, a feed or the network can do to a user of Vehicoule, and which rule or test stops it. The
  plugin host spec ([`plugin-host-spec.md`](plugin-host-spec.md)) says what the host does; this file says why, and
  where the protection is still missing.

Every threat below names its defence and the test of the spec's list (spec §9) that must show it. A defence with no
test in that list is a gap, not a defence. Until a listed test runs in CI, its defence is only intended.

## 1. What is protected

- **The user's device**: files, other apps, the platform's APIs.
- **The user's accounts and identity**: credentials, cookies, the IP address, what they search and play.
- **The app**: it keeps running and stays responsive whatever a plugin does.
- **Other plugins**: one plugin cannot read or change another's calls.

## 2. Who attacks

| Attacker | Reaches the user through | Example |
|---|---|---|
| A plugin author | A plugin the user installs | A lyrics plugin that tries to read files or send the user's queries elsewhere |
| A feed operator, or whoever stole a feed's key | A plugin update the user did not ask for | A trusted plugin replaced by a malicious version |
| A network attacker | Traffic between a plugin and its services, or between the app and a feed | An old feed replayed to keep a vulnerable plugin installed |
| A content provider | The answers a plugin parses and returns | A response crafted to exceed limits or break the contract |
| A contributor or a dependency | This repository and its CI | A compromised crate or workflow |

## 3. Threats and defences

| Threat | Defence | Test (spec §9) |
|---|---|---|
| A plugin reads or writes files, opens sockets, reads the environment | No WASI; only `http`, `log` and `types` are importable ([ADR 0007](adr/0007-plugin-host.md), spec §5) | Spec §9, must-never 2 |
| A plugin reaches a host it did not declare, plain HTTP, an IP address or another port | Every request goes through the host and is checked against the stored manifest (spec §5) | Must-never 1 |
| A plugin escapes through redirects | The host follows no redirect; the player edge checks every hop of a stream (spec §5) | Must-never 6, 7, 9 |
| A plugin hangs, loops or exhausts memory, requests or output | Fuel, epoch deadline, memory and table ceilings, HTTP and output budgets (spec §6) | Must-happen 2 |
| A plugin crashes the app, or corrupts the next call | Traps become a typed failure; a fresh store and instance per call (spec §3, §7) | Must-happen 3, must-never 4 |
| A plugin keeps user state between calls to track the user | Nothing survives a call (spec §3) | Must-never 4 |
| A plugin is so large that compiling it exhausts the device | Size ceiling checked before parsing (spec §3, §6) | Must-never 8 |
| A plugin under the size ceiling still makes the compiler exhaust time or memory | Compilation on a host thread; fuel and store limits do not apply to it, so only the size ceiling bounds it, set from recorded compile time and memory (spec §3, §9) | None yet: gap G6 |
| A plugin's manifest lies about what it provides | Manifest checked against the component's exports (spec §4) | Must-never 3 |
| A plugin changes its declared hosts after the user agreed | The stored manifest, not a later `describe`, is enforced (spec §3) | None yet: gap G5 |
| A plugin's log text is shown to the user as the app's own words | Logs are bounded, tagged, never shown as app text (spec §5) | None yet: gap G5 |
| A bug in Wasmtime lets a plugin escape | Wasmtime LTS with security backports, `cargo deny` fails on a known advisory ([ADR 0007](adr/0007-plugin-host.md)) | CI `deny` job |

## 4. Accepted risks

These follow from what a plugin is for; the user should know them, not be protected from them.

- **A plugin sees what it is asked.** A search plugin receives the user's query and sends it to its declared hosts,
  with the user's IP address. The declared hosts are shown before the plugin is enabled (spec §3); a privacy document
  will say what each published plugin sends.
- **A plugin may lie in its answers.** Wrong metadata, a stream of the wrong track. The core's user corrections and
  priority lists handle it; the sandbox does not.
- **A declared host may be malicious.** Declaring a host is consent to talk to it, not proof that it is honest.

## 5. Gaps

| Gap | Why it matters | Next step |
|---|---|---|
| G1. Feeds and signatures are not specified yet (spec, scope) | Without them, the threats from feed operators and network attackers in section 2 have no defence: rollback to a vulnerable version, a frozen feed, a stolen signing key | The feed spec, with an ADR choosing The Update Framework or its minimal protections (increasing versions, expiry, key rotation and revocation) |
| G2. No fuzzing of what the host reads from a plugin | Manifests, results and component bytes are the least trusted input of the app; property tests cover the matcher and the manifest only (spec §9) | An ADR on fuzzing, with a fixed number of runs in CI so that no test depends on time |
| G3. Side channels between plugins | Two plugins in one process could in principle observe each other through timing | Not measured; calls are sequential per store (spec §1). Review when plugins run in parallel |
| G4. Supply chain of the plugins repositories | A plugin is only as safe as its own CI | Same rules as here for `vehicoule-plugins` and `vehicoule-plugins-extra`: pinned actions, locked dependencies, `cargo deny` |
| G5. Two defences of section 3 have no test in the spec | A defence with no test is not shown to hold (AGENTS.md: CI is the only truth) | Add the tests to the spec with the edge that stores manifests and shows logs |
| G6. Compiling an accepted plugin has no measured bound | Must-never 8 only shows that bytes over the ceiling are refused; the compile time and memory of bytes under it are not measured yet, so the ceiling is not shown to bound them | Record compile time and memory per plugin (spec §9), set the ceiling from them in an ADR, and test a plugin at the ceiling against that bound |

## 6. Keeping this file true

A change to the sandbox, the contract or the budgets updates this file in the same pull request. A new threat comes
with its defence and its test, or as a gap.
