//! `plugins/deezer` inside the real `plugin-host`, on responses recorded from Deezer.

use plugin_host::music::entity::Entity;
use plugin_host::music::search::SearchRequest;
use plugin_host::types::{EntityKind, EntityRef, TrackQuery};
use plugin_host::{CallError, Capability};
use tests::load;

fn dbg<T: std::fmt::Debug>(v: T) -> String {
    format!("{v:?}")
}

fn target(kind: EntityKind, id: &str) -> EntityRef {
    EntityRef { kind, id: id.into() }
}

fn seed(id: Option<&str>) -> TrackQuery {
    TrackQuery {
        id: id.map(str::to_string),
        title: "Harder Better Faster Stronger".into(),
        artists: vec!["Daft Punk".into()],
        album: None,
        duration_ms: Some(224_000),
        isrc: None,
    }
}

#[tokio::test]
async fn manifest_and_exports_match() {
    let (_host, plugin, _) = load("deezer", "empty");
    assert_eq!(plugin.id(), "deezer");
    assert_eq!(plugin.capabilities(), [Capability::Search, Capability::Entity, Capability::Radio]);
    let m = plugin.manifest();
    assert_eq!(m.domains, ["api.deezer.com", "www.deezer.com", "link.deezer.com", "*.dzcdn.net"]);
    let rl = m.rate_limit.unwrap();
    assert_eq!((rl.requests, rl.per_seconds), (45, 5));
    assert!(!m.descriptive_user_agent);
}

#[tokio::test]
async fn short_link_follows_redirect() {
    let (host, plugin, _) = load("deezer", "short_link");
    // 301 → link.deezer.com/?dest=… → www.deezer.com/playlist/… → www.deezer.com/fr/playlist/…
    let found = host.parse_link(&plugin, "https://link.deezer.com/s/34bMMRXCn45BPl52sk6ml").await.unwrap();
    assert_eq!(dbg(found), dbg(Some(target(EntityKind::Playlist, "15565783443"))));
    // A dead share link lands on /deezer-links-404.
    let dead = host.parse_link(&plugin, "https://link.deezer.com/s/31nhZMumBaLkQg77gDv3x").await;
    assert_eq!(dead.unwrap_err(), CallError::NotFound);
    // Refused before any redirect (hand-built recording): typed, not a dead link.
    let limited = host.parse_link(&plugin, "https://link.deezer.com/s/30RateLimitedByHand").await;
    assert_eq!(limited.unwrap_err(), CallError::RateLimited);
    // Full links need no request (none is recorded for them); other domains are not ours.
    let full = host.parse_link(&plugin, "https://www.deezer.com/fr/album/302127").await.unwrap();
    assert_eq!(dbg(full), dbg(Some(target(EntityKind::Album, "302127"))));
    assert!(host.parse_link(&plugin, "https://example.com/album/302127").await.unwrap().is_none());
}

#[tokio::test]
async fn playlist_import_pages_by_100() {
    let (host, plugin, _) = load("deezer", "playlist");
    let playlist = target(EntityKind::Playlist, "6458328984");
    let first = host.fetch_entity(&plugin, playlist.clone(), "FR", None).await.unwrap();
    let Entity::Playlist(p) = &first.entity else { panic!("{:?}", first.entity) };
    assert_eq!((p.id.as_str(), p.title.as_str()), ("6458328984", "Musique française années 80"));
    assert_eq!((p.owner.as_deref(), p.track_count), (Some("Digster France"), Some(102)));
    assert_eq!(first.tracks.len(), 100);
    let t = &first.tracks[0];
    assert_eq!((t.id.as_str(), t.title.as_str(), t.artists[0].name.as_str()), ("2178580", "Je te promets", "Johnny Hallyday"));
    assert_eq!(t.isrc.as_deref(), Some("FRZ808680050"));
    assert_eq!(first.next.as_deref(), Some("100"));

    let last = host.fetch_entity(&plugin, playlist, "FR", first.next).await.unwrap();
    let ids: Vec<&str> = last.tracks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["17819255", "1126389"]);
    assert_eq!(last.next, None);
}

#[tokio::test]
async fn album_and_artist_pages() {
    let (host, plugin, _) = load("deezer", "entity");
    let album = host.fetch_entity(&plugin, target(EntityKind::Album, "302127"), "FR", None).await.unwrap();
    let Entity::Album(a) = &album.entity else { panic!("{:?}", album.entity) };
    assert_eq!((a.title.as_str(), a.year, a.track_count), ("Discovery", Some(2001), Some(14)));
    assert_eq!((album.tracks.len(), album.next.as_deref()), (14, None));
    let t = &album.tracks[0];
    assert_eq!(t.album.as_ref().map(|r| (r.id.as_deref(), r.title.as_str())), Some((Some("302127"), "Discovery")));
    assert_eq!(dbg(&t.artwork), dbg(&a.artwork));

    let artist = host.fetch_entity(&plugin, target(EntityKind::Artist, "27"), "FR", None).await.unwrap();
    let Entity::Artist(ar) = &artist.entity else { panic!("{:?}", artist.entity) };
    assert_eq!(ar.name, "Daft Punk");
    assert_eq!((artist.tracks.len(), artist.albums.len(), artist.next.as_deref()), (20, 39, None));
    assert_eq!(artist.tracks[0].title, "Instant Crush (feat. Julian Casablancas)");
    assert_eq!(artist.albums[0].title, "Random Access Memories (Drumless Edition)");
    assert_eq!(artist.albums[0].year, Some(2023));
    assert_eq!(artist.albums[0].artists[0].id.as_deref(), Some("27"));

    // Deezer answers 200 with `{"error":{"code":800}}`.
    let unknown = host.fetch_entity(&plugin, target(EntityKind::Track, "0"), "FR", None).await;
    assert_eq!(unknown.unwrap_err(), CallError::NotFound);
    let bogus = host.fetch_entity(&plugin, target(EntityKind::Track, "../user/1"), "FR", None).await;
    assert_eq!(bogus.unwrap_err(), CallError::NotFound);
}

#[tokio::test]
async fn search_every_kind() {
    let (host, plugin, _) = load("deezer", "search");
    let req = SearchRequest { query: "daft punk".into(), kinds: vec![], region: "FR".into(), limit: 2, cursor: None };
    let found = host.search(&plugin, req).await.unwrap();
    assert_eq!((found.tracks.len(), found.albums.len(), found.artists.len(), found.playlists.len()), (2, 2, 2, 2));
    assert_eq!(found.tracks[0].id, "67238732");
    assert_eq!(found.artists[0].name, "Daft Punk");
    assert_eq!(found.next.as_deref(), Some("2"));
}

#[tokio::test]
async fn empty_search_is_empty_not_error() {
    let (host, plugin, _) = load("deezer", "empty");
    let req = SearchRequest {
        query: "zzqxjvkwplmnbv".into(),
        kinds: vec![EntityKind::Track],
        region: "FR".into(),
        limit: 5,
        cursor: None,
    };
    let found = host.search(&plugin, req).await.unwrap();
    assert!(found.tracks.is_empty() && found.albums.is_empty() && found.artists.is_empty() && found.playlists.is_empty());
    assert_eq!(found.next, None);
}

#[tokio::test]
async fn radio_from_seed_track() {
    let (host, plugin, reports) = load("deezer", "radio");
    let http_calls = || reports.lock().unwrap().last().unwrap().http_calls;
    // `track/3135556` → artist 27 → `artist/27/radio`.
    let radio = host.radio(&plugin, seed(Some("3135556")), "FR", 5).await.unwrap();
    let ids: Vec<&str> = radio.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["3155979", "559283992", "3167848", "135373118", "67238737"]);
    assert_eq!(radio[0].artwork[0].width, Some(1000));
    assert_eq!(http_calls(), 2);
    // Again: the track comes from the host's cache, the radio (`no-cache`) from the network.
    host.radio(&plugin, seed(Some("3135556")), "FR", 5).await.unwrap();
    assert_eq!(http_calls(), 1);
    // Without an id, the first search match is the seed.
    let searched = host.radio(&plugin, seed(None), "FR", 5).await.unwrap();
    assert_eq!(dbg(searched), dbg(&radio));
    // An id Deezer no longer knows (`{"error":{"code":800}}`) falls back to the search.
    let stale = host.radio(&plugin, seed(Some("0")), "FR", 5).await.unwrap();
    assert_eq!(dbg(stale), dbg(&radio));
    // Nothing to search for: `not-found` without a request (none is recorded for it).
    let blank = TrackQuery { title: String::new(), artists: vec![], ..seed(None) };
    assert_eq!(host.radio(&plugin, blank, "FR", 5).await.unwrap_err(), CallError::NotFound);
}

/// Deezer's quota (code 4) and "busy" (700) errors come as HTTP 200, which the host caches for
/// a day: each is asked once more past the cache before `rate-limited`.
#[tokio::test]
async fn quota_errors_retry_past_the_cache() {
    let (host, plugin, reports) = load("deezer", "quota");
    let http_calls = || reports.lock().unwrap().last().unwrap().http_calls;
    let quota = target(EntityKind::Track, "1");
    let busy = target(EntityKind::Track, "2");
    assert_eq!(host.fetch_entity(&plugin, quota.clone(), "FR", None).await.unwrap_err(), CallError::RateLimited);
    assert_eq!(http_calls(), 2);
    // The first answer is now cached; the retry still reaches the network.
    assert_eq!(host.fetch_entity(&plugin, quota, "FR", None).await.unwrap_err(), CallError::RateLimited);
    assert_eq!(http_calls(), 1);
    assert_eq!(host.fetch_entity(&plugin, busy, "FR", None).await.unwrap_err(), CallError::RateLimited);
    assert_eq!(http_calls(), 2);
}
