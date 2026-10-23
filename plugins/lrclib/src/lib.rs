//! LRCLIB as a Vehicoule plugin: the free, keyless lyrics API (`lrclib.net`) behind the
//! `vehicoule:music` contract's `lyrics` capability, over the host's `http`.

wit_bindgen::generate!({
    path: ["../../wit/host", "../../wit/music", "wit"],
    world: "vehicoule:lrclib/lrclib",
    with: {
        "vehicoule:host/http@1.0.0": vehicoule_sdk::host_http,
        "vehicoule:host/kv@1.0.0": vehicoule_sdk::kv,
        "vehicoule:host/clock@1.0.0": vehicoule_sdk::clock,
        "vehicoule:host/log@1.0.0": vehicoule_sdk::log,
        "vehicoule:music/types@1.1.0": vehicoule_sdk::types,
    },
});

mod api;
mod lrc;

use exports::vehicoule::music::lyrics::{Guest, Lyrics};
use vehicoule_sdk::types::{Error, TrackQuery};

struct Lrclib;

impl Guest for Lrclib {
    fn fetch(track: TrackQuery) -> Result<Lyrics, Error> {
        api::fetch(&track)
    }
}

export!(Lrclib);

#[cfg(test)]
mod tests {
    /// Reads `manifest.json` directly: the host sets the descriptive User-Agent itself when
    /// this flag is set, so only the flag is this plugin's concern (the header itself is a
    /// host-side test).
    #[test]
    fn manifest_asks_for_descriptive_user_agent() {
        let manifest: serde_json::Value = serde_json::from_str(include_str!("../manifest.json")).unwrap();
        assert_eq!(manifest["descriptive_user_agent"], true);
    }
}
