# vehicoule-plugins

Plugins officiels de Vehicoule (deezer, itunes, lrclib) et outillage de publication du feed. Licence : MIT OR Apache-2.0.

Style : guide Rust de Google, appliqué par rustfmt et clippy (voir Cargo.toml).

Installer l'outil de signature avant `scripts/publish.sh` :

```sh
cargo install --git file:///Users/btw/Documents/repo/Vehicoule --tag plugin-host-v1.0.0 plugin-host --bin vehicoule-feed
```
