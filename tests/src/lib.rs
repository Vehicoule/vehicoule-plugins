//! Shared test harness for the official plugins (`plugins/deezer`, `plugins/itunes`,
//! `plugins/lrclib`): builds a plugin's `.wasm` and loads it into the real `plugin-host`
//! with its HTTP replayed from fixtures.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use plugin_host::{Host, HostConfig, Limits, MemoryStorage, NetPolicy, Plugin};

fn workspace() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
}

/// Builds `plugins/<plugin>` to `wasm32-wasip2` release once per test binary.
fn build(plugin: &str) -> Vec<u8> {
    static CACHE: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().unwrap();
    let path = cache
        .entry(plugin.to_string())
        .or_insert_with(|| {
            let target = workspace().join("target/plugin-tests");
            let status = Command::new(env!("CARGO"))
                .args(["build", "--release", "--target", "wasm32-wasip2", "-p", plugin, "--target-dir"])
                .arg(&target)
                .current_dir(workspace())
                .env_remove("CARGO_TARGET_DIR")
                .status()
                .expect("cargo build plugin");
            assert!(status.success(), "building plugins/{plugin} failed");
            target.join(format!("wasm32-wasip2/release/{}.wasm", plugin.replace('-', "_")))
        })
        .clone();
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// Loads `plugins/<plugin>`, its HTTP replayed from `tests/fixtures/<plugin>/<fixtures>`.
pub fn load(plugin: &str, fixtures: &str) -> (Host, Plugin, Arc<MemoryStorage>) {
    let wasm = build(plugin);
    let manifest_path = workspace().join("plugins").join(plugin).join("manifest.json");
    let manifest = std::fs::read(&manifest_path).unwrap_or_else(|e| panic!("reading {}: {e}", manifest_path.display()));
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(plugin).join(fixtures);

    let storage = Arc::new(MemoryStorage::default());
    let cache_dir = std::env::temp_dir().join(format!("vehicoule-plugins-tests-{}", std::process::id()));
    let host = Host::new(HostConfig {
        cache_dir,
        storage: storage.clone(),
        events: Arc::new(|_| {}),
        limits: Limits::default(),
        app_version: "0.0.0-test".into(),
        net: NetPolicy::replay(&fixtures_dir),
    })
    .unwrap();
    let plugin = host.load(&manifest, &wasm).unwrap();
    (host, plugin, storage)
}
