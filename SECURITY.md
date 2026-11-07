# Security policy

## Reporting a vulnerability

Report it privately, through GitHub's private vulnerability reporting:
[report a vulnerability](https://github.com/Vehicoule/vehicoule-plugins/security/advisories/new). Do not open a public
issue, pull request or discussion about it.

Say what is affected (the plugin and the commit), how to reproduce it, and what an attacker gains. A service response
that makes a plugin misbehave is the best reproduction.

## Supported versions

No plugin is released yet. Only the latest commit of `main` is supported.

## Scope

In scope:

- **A plugin**: a service response or a user's input that makes it panic, loop, grow its memory, or return a value
  that breaks the contract; a request it sends to a host it did not mean to (a URL built from untrusted text); data it
  sends to its service beyond what the call needs.
- **The supply chain of this repository**: its CI workflows, its pinned dependencies, its copy of the contract and
  its vendored sources.

Out of scope: the plugin sandbox and the plugin host, which are Vehicoule's; report those to
[Vehicoule](https://github.com/Vehicoule/Vehicoule/security/advisories/new). Bugs in a content provider's service;
report those to the provider.
