//! `plugins/lrclib` inside the real `plugin-host`, on responses recorded from lrclib.net.

use plugin_host::types::TrackQuery;
use plugin_host::{CallError, Capability};
use tests::load;

fn query(title: &str, artist: &str, album: Option<&str>, duration_ms: Option<u32>) -> TrackQuery {
    TrackQuery { id: None, title: title.into(), artists: vec![artist.into()], album: album.map(Into::into), duration_ms, isrc: None }
}

#[tokio::test]
async fn manifest_and_exports_match() {
    let (_host, plugin, _) = load("lrclib", "get");
    assert_eq!(plugin.id(), "lrclib");
    assert_eq!(plugin.capabilities(), [Capability::Lyrics]);
    let m = plugin.manifest();
    assert_eq!(m.domains, ["lrclib.net"]);
    assert!(m.rate_limit.is_none());
    assert!(m.descriptive_user_agent);
}

/// `GET /api/get` hits on the first try: plain and synced lyrics both come back, the synced
/// side parsed from LRC text into timed lines, sorted and in order.
#[tokio::test]
async fn get_returns_plain_and_synced() {
    let (host, plugin, _) = load("lrclib", "get");
    let track = query("Yesterday", "The Beatles", Some("Help!"), Some(126_000));
    let lyrics = host.lyrics(&plugin, track).await.unwrap();
    assert!(lyrics.plain.unwrap().starts_with("Yesterday, all my troubles seemed so far away"));
    assert_eq!(lyrics.synced[0].start_ms, 5_060);
    assert_eq!(lyrics.synced[0].text, "Yesterday, all my troubles seemed so far away");
    // Sorted by time; every entry before the next starts no later than it.
    assert!(lyrics.synced.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));
}

/// `GET /api/get` 404s (the recorded album doesn't match this exact track), so the plugin
/// falls back to `GET /api/search` and picks the first result within 2s of the query's
/// duration.
#[tokio::test]
async fn search_fallback_matches_duration() {
    let (host, plugin, _) = load("lrclib", "search");
    let track = query("Yesterday", "The Beatles", Some("WrongAlbumName"), Some(126_000));
    let lyrics = host.lyrics(&plugin, track).await.unwrap();
    assert!(lyrics.plain.unwrap().starts_with("Yesterday, all my troubles seemed so far away"));
    assert_eq!(lyrics.synced[0].start_ms, 5_340);
}

/// `instrumental: true` on the matched track (even though it fetched fine) is `not-found`, not
/// empty lyrics.
#[tokio::test]
async fn instrumental_is_not_found() {
    let (host, plugin, _) = load("lrclib", "instrumental");
    let track = query("Rebel Rouser", "Duane Eddy", Some("Rebel Rouser"), Some(142_000));
    let result = host.lyrics(&plugin, track).await;
    assert_eq!(result.unwrap_err(), CallError::NotFound);
}

