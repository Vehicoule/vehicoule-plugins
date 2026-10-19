use std::process::Command;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

/// The vendored contract must be the one the SDK was built against: `UPDATE_WIT=1` rewrites it.
#[test]
fn wit_deps_match_sdk() {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
    for (path, text) in [("wit/host/host.wit", vehicoule_sdk::HOST_WIT), ("wit/music/music.wit", vehicoule_sdk::MUSIC_WIT)] {
        let path = root.join(path);
        if std::env::var_os("UPDATE_WIT").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text, "{} differs from the SDK: UPDATE_WIT=1", path.display());
    }
}

/// `scripts/publish.sh`, run against a fake (empty) plugin set in a temp copy of the repo
/// root so the real `feed/sequence` is never touched, produces a `feed.json` that
/// `plugin_host::verify_feed` accepts under the key `vehicoule-feed keygen` just printed.
#[test]
fn published_feed_verifies() {
    let repo = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
    let root = tempfile::tempdir().expect("tempdir for fake repo root");

    std::fs::create_dir_all(root.path().join("scripts")).unwrap();
    std::fs::copy(repo.join("scripts/publish.sh"), root.path().join("scripts/publish.sh")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path().join("scripts/publish.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::create_dir_all(root.path().join("plugins")).unwrap(); // no plugins today: zero to build
    std::fs::create_dir_all(root.path().join("feed")).unwrap();
    std::fs::write(root.path().join("feed/sequence"), "0\n").unwrap();

    let key_dir = tempfile::tempdir().expect("tempdir for test key");
    let key_path = key_dir.path().join("test.key");
    let keygen = Command::new("vehicoule-feed").arg("keygen").arg(&key_path).output().unwrap_or_else(|e| {
        panic!(
            "running `vehicoule-feed keygen`: {e}; install it with \
             `cargo install --git file:///Users/btw/Documents/repo/Vehicoule --tag plugin-host-v1.0.0 plugin-host --bin vehicoule-feed`"
        )
    });
    assert!(keygen.status.success(), "vehicoule-feed keygen failed: {}", String::from_utf8_lossy(&keygen.stderr));
    let stdout = String::from_utf8(keygen.stdout).unwrap();
    let public_key_b64 = stdout.lines().find_map(|l| l.strip_prefix("public key: ")).expect("no public key in keygen output");
    let public_key: [u8; 32] = BASE64.decode(public_key_b64).unwrap().try_into().unwrap();

    let out_dir = root.path().join("out");
    let publish = Command::new(root.path().join("scripts/publish.sh"))
        .arg(&out_dir)
        .env("VEHICOULE_FEED_KEY", &key_path)
        .output()
        .expect("running publish.sh");
    assert!(publish.status.success(), "publish.sh failed: {}", String::from_utf8_lossy(&publish.stderr));

    let feed_bytes = std::fs::read(out_dir.join("feed.json")).expect("publish.sh should write feed.json");
    let feed = plugin_host::verify_feed(&feed_bytes, &[public_key], None).expect("feed.json should verify");
    assert_eq!(feed.name, "official");
    assert_eq!(feed.sequence, 1);
    assert!(feed.plugins.is_empty());
}
