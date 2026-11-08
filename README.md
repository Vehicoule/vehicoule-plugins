# vehicoule-plugins

Content plugins for [Vehicoule](https://github.com/Vehicoule/Vehicoule): WebAssembly components, written in Rust, that
the app runs in a sandbox to search, describe and illustrate music from public services. Its plugins are
`deezer` and `lrclib`; `itunes` is withdrawn until the video work (Vehicoule's ADR 0018).

Licensed under the [GNU General Public License v3.0](LICENSE) ([ADR 0004](docs/adr/0004-license-the-plugins-under-gpl-3.0.md)).

Start with [`AGENTS.md`](AGENTS.md): the rules every contributor, human or agent, follows. How a plugin is written is
in [`docs/plugin-crates.md`](docs/plugin-crates.md), decisions in [`docs/adr/`](docs/adr/), sources in
[`docs/references.md`](docs/references.md).
