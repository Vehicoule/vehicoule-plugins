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
    assert_eq!(
        plugin.capabilities(),
        [
            Capability::Search,
            Capability::Entity,
            Capability::Radio,
            Capability::Browse
        ]
    );
    let m = plugin.manifest();
    assert_eq!(m.domains, ["api.deezer.com", "www.deezer.com", "link.deezer.com", "*.dzcdn.net"]);
    let rl = m.rate_limit.unwrap();
    assert_eq!((rl.requests, rl.per_seconds), (45, 5));
    assert!(!m.descriptive_user_agent);
    assert_eq!((m.version.as_str(), m.contract.as_str()), ("0.2.0", "1.1"));
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

fn is_iso_date(d: &str) -> bool {
    d.len() == 10
        && d.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

#[tokio::test]
async fn browse_categories_lists_featured_then_genres() {
    let (host, plugin, _) = load("deezer", "browse_genres");
    let cats = host.browse_categories(&plugin, "FR").await.unwrap();
    let head: Vec<_> = cats[..2]
        .iter()
        .map(|c| (c.id.as_str(), c.title.as_str(), c.featured))
        .collect();
    assert_eq!(
        head,
        [
            ("chart:0", "Top 50", true),
            ("selection:0", "Sélection", true)
        ]
    );
    // The covers of the chart's first track and of the selection's first album.
    let cover = |c: &plugin_host::music::browse::Category| {
        c.artwork.first().map(|i| i.url.clone()).unwrap_or_default()
    };
    assert!(
        cover(&cats[0]).contains("/a7f9bae0243c512059298a68d09f45a1/1000x1000"),
        "{:?}",
        cats[0]
    );
    assert!(
        cover(&cats[1]).contains("/ea1408a80efbe37b26ee99a62c323525/1000x1000"),
        "{:?}",
        cats[1]
    );
    // Then the genres, without "Tous" (id 0).
    assert_eq!(cats.len(), 2 + 25);
    assert!(cats[2..].iter().all(|c| !c.featured
        && c.id.starts_with("genre:")
        && c.id != "genre:0"
        && !c.artwork.is_empty()));
    assert_eq!(
        (cats[2].id.as_str(), cats[2].title.as_str()),
        ("genre:132", "Pop")
    );
}

#[tokio::test]
async fn browse_chart_category_has_track_shelf() {
    let (host, plugin, _) = load("deezer", "browse_chart");
    let page = host
        .browse_category(&plugin, "chart:0", "FR", None)
        .await
        .unwrap();
    assert_eq!(
        (page.title.as_str(), page.next.as_deref()),
        ("Top 50", None)
    );
    let titles: Vec<_> = page
        .shelves
        .iter()
        .map(|s| (s.title.as_str(), s.items.len()))
        .collect();
    assert_eq!(
        titles,
        [
            ("tracks", 50),
            ("albums", 49),
            ("artists", 50),
            ("playlists", 50)
        ]
    );
    let Entity::Track(t) = &page.shelves[0].items[0] else {
        panic!("{:?}", page.shelves[0].items[0])
    };
    assert_eq!(
        (t.title.as_str(), t.artwork[0].width),
        ("Dai Dai", Some(1000))
    );
    // Unknown ids: `not-found` without a request (none is recorded for them).
    for id in ["foo:1", "chart:x", "genre:"] {
        assert_eq!(
            host.browse_category(&plugin, id, "FR", None)
                .await
                .unwrap_err(),
            CallError::NotFound,
            "{id}"
        );
    }
}

#[tokio::test]
async fn browse_selection_is_one_album_shelf() {
    let (host, plugin, _) = load("deezer", "browse_selection");
    let page = host
        .browse_category(&plugin, "selection:0", "FR", None)
        .await
        .unwrap();
    assert_eq!(
        (page.title.as_str(), page.next.as_deref()),
        ("Sélection", None)
    );
    let titles: Vec<_> = page
        .shelves
        .iter()
        .map(|s| (s.title.as_str(), s.items.len()))
        .collect();
    assert_eq!(titles, [("albums", 10)]);
}

#[tokio::test]
async fn browse_releases_has_dates() {
    let (host, plugin, _) = load("deezer", "browse_releases");
    let releases = host.browse_releases(&plugin, "27", "FR").await.unwrap();
    assert_eq!(releases.len(), 39);
    assert!(
        releases.iter().all(|r| is_iso_date(&r.date)),
        "{releases:?}"
    );
    assert!(releases.windows(2).all(|w| w[0].date >= w[1].date));
    assert_eq!(
        (releases[0].date.as_str(), releases[0].album.id.as_str()),
        ("2023-11-17", "494309801")
    );
    let bogus = host.browse_releases(&plugin, "../user/1", "FR").await;
    assert_eq!(bogus.unwrap_err(), CallError::NotFound);
}
