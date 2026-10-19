//! `plugins/itunes` inside the real `plugin-host`, on responses recorded from iTunes.

use plugin_host::music::entity::Entity;
use plugin_host::music::search::SearchRequest;
use plugin_host::types::{EntityKind, EntityRef};
use plugin_host::{CallError, Capability};
use tests::load;

fn dbg<T: std::fmt::Debug>(v: T) -> String {
    format!("{v:?}")
}

fn target(kind: EntityKind, id: &str) -> EntityRef {
    EntityRef { kind, id: id.into() }
}

#[tokio::test]
async fn manifest_and_exports_match() {
    let (_host, plugin, _) = load("itunes", "empty");
    assert_eq!(plugin.id(), "itunes");
    assert_eq!(plugin.capabilities(), [Capability::Search, Capability::Entity]);
    let m = plugin.manifest();
    assert_eq!(m.domains, ["itunes.apple.com", "music.apple.com", "*.mzstatic.com"]);
    let rl = m.rate_limit.unwrap();
    assert_eq!((rl.requests, rl.per_seconds), (20, 60));
    assert!(!m.descriptive_user_agent);
}

#[tokio::test]
async fn search_every_kind() {
    let (host, plugin, _) = load("itunes", "search");
    let req = SearchRequest { query: "daft punk".into(), kinds: vec![], region: "FR".into(), limit: 2, cursor: None };
    let found = host.search(&plugin, req).await.unwrap();
    assert_eq!((found.tracks.len(), found.albums.len(), found.artists.len(), found.playlists.len()), (2, 2, 2, 0));
    assert_eq!(found.tracks[0].title, "Instant Crush");
    assert_eq!(found.albums[0].title, "Random Access Memories");
    assert_eq!(found.artists[0].name, "Daft Punk");
    // Every kind came back a full page (resultCount == limit): next continues from offset 2.
    assert_eq!(found.next.as_deref(), Some("2"));
}

#[tokio::test]
async fn empty_search_is_empty_not_error() {
    let (host, plugin, _) = load("itunes", "empty");
    let req = SearchRequest { query: "zzqxjvkwplmnbv".into(), kinds: vec![EntityKind::Track], region: "FR".into(), limit: 5, cursor: None };
    let found = host.search(&plugin, req).await.unwrap();
    assert!(found.tracks.is_empty() && found.albums.is_empty() && found.artists.is_empty() && found.playlists.is_empty());
    assert_eq!(found.next, None);
}

/// The one failing test from the brief: a `lookup` by album id lists its tracks, ordered by
/// track number then disc number (the album here is single-disc, so this also matches the
/// API's own order).
#[tokio::test]
async fn album_entity_lists_tracks_in_order() {
    let (host, plugin, _) = load("itunes", "entity");
    let album = host.fetch_entity(&plugin, target(EntityKind::Album, "617154241"), "FR", None).await.unwrap();
    let Entity::Album(a) = &album.entity else { panic!("{:?}", album.entity) };
    assert_eq!((a.title.as_str(), a.year, a.track_count), ("Random Access Memories", Some(2013), Some(14)));
    assert_eq!(album.tracks.len(), 13);
    let titles: Vec<&str> = album.tracks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Give Life Back to Music",
            "The Game of Love",
            "Giorgio by Moroder",
            "Within",
            "Instant Crush",
            "Lose Yourself to Dance",
            "Touch",
            "Get Lucky",
            "Beyond",
            "Motherboard",
            "Fragments of Time",
            "Doin' it Right",
            "Contact",
        ]
    );
    assert_eq!(album.next, None);
    assert_eq!(dbg(&album.tracks[0].artwork), dbg(&a.artwork));
}

#[tokio::test]
async fn artist_entity_lists_albums() {
    let (host, plugin, _) = load("itunes", "entity");
    let artist = host.fetch_entity(&plugin, target(EntityKind::Artist, "5468295"), "FR", None).await.unwrap();
    let Entity::Artist(ar) = &artist.entity else { panic!("{:?}", artist.entity) };
    assert_eq!(ar.name, "Daft Punk");
    assert_eq!(artist.albums.len(), 38);
    assert_eq!(artist.albums[0].title, "Random Access Memories");
    assert!(artist.tracks.is_empty() && artist.next.is_none());
}

#[tokio::test]
async fn track_entity_and_not_found() {
    let (host, plugin, _) = load("itunes", "entity");
    let track = host.fetch_entity(&plugin, target(EntityKind::Track, "617154362"), "FR", None).await.unwrap();
    let Entity::Track(t) = &track.entity else { panic!("{:?}", track.entity) };
    assert_eq!(t.title, "Instant Crush");

    // iTunes answers an unknown id with HTTP 200 and an empty `results`.
    let unknown = host.fetch_entity(&plugin, target(EntityKind::Track, "0"), "FR", None).await;
    assert_eq!(unknown.unwrap_err(), CallError::NotFound);
    let bogus = host.fetch_entity(&plugin, target(EntityKind::Track, "../user/1"), "FR", None).await;
    assert_eq!(bogus.unwrap_err(), CallError::NotFound);
    // Not served by this API: no request is even recorded for it.
    let playlist = host.fetch_entity(&plugin, target(EntityKind::Playlist, "1"), "FR", None).await;
    assert_eq!(playlist.unwrap_err(), CallError::NotFound);
}

#[tokio::test]
async fn links_need_no_request() {
    let (host, plugin, _) = load("itunes", "empty");
    let album = host.parse_link(&plugin, "https://music.apple.com/fr/album/random-access-memories/617154241").await.unwrap();
    assert_eq!(dbg(album), dbg(Some(target(EntityKind::Album, "617154241"))));
    let track = host.parse_link(&plugin, "https://music.apple.com/fr/album/instant-crush/617154241?i=617154362&uo=4").await.unwrap();
    assert_eq!(dbg(track), dbg(Some(target(EntityKind::Track, "617154362"))));
    assert!(host.parse_link(&plugin, "https://music.apple.com/fr/playlist/french-touch/pl.abc123").await.unwrap().is_none());
    assert!(host.parse_link(&plugin, "https://example.com/album/617154241").await.unwrap().is_none());
}
