//! iTunes Search/Lookup JSON to contract values. Every request answers with an HTTP status
//! (no `{"error":…}` envelope on 200 like Deezer); the status is checked by `status_error`
//! before any body reaches these parsers. Search and lookup share one envelope shape:
//! `{"resultCount": n, "results": [...]}`, with each item tagged by `wrapperType`
//! (`"track"`, `"collection"`, `"artist"`).

use serde::Deserialize;
use vehicoule_sdk::types::{Album, AlbumRef, Artist, ArtistRef, EntityKind, EntityRef, Error, Image, Track};

/// A search response split by kind; `total` is iTunes' own `resultCount`, needed to tell a
/// full page (possibly more to fetch) from the last one.
#[derive(Debug, Default)]
pub struct SearchParts {
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
    pub total: u32,
}

/// iTunes signals its own throttling with 403 (not 429); anything else maps through the SDK.
pub fn status_error(status: u16) -> Option<Error> {
    if status == 403 {
        return Some(Error::RateLimited);
    }
    vehicoule_sdk::status_error(status)
}

/// `…/100x100bb.jpg` → `…/1000x1000bb.jpg` (1000 px); the 100 px URL is kept too by the caller.
pub fn big_artwork(url100: &str) -> String {
    url100.replace("100x100bb", "1000x1000bb")
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(rename = "resultCount")]
    result_count: u32,
    results: Vec<serde_json::Value>,
}

fn envelope(body: &[u8]) -> Result<Envelope, Error> {
    serde_json::from_slice(body).map_err(|e| Error::Internal(e.to_string()))
}

// Only the fields in use; absent/malformed optional fields degrade gracefully. A missing
// mandatory one (the id or name field below) fails that one item's `from_value`, so
// `parse_results` and friends skip just that item rather than the whole response.

#[derive(Deserialize)]
struct ItTrack {
    #[serde(rename = "trackId")]
    track_id: i64,
    #[serde(rename = "trackName")]
    track_name: String,
    #[serde(rename = "artistId")]
    artist_id: Option<i64>,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    #[serde(rename = "collectionId")]
    collection_id: Option<i64>,
    #[serde(rename = "collectionName")]
    collection_name: Option<String>,
    #[serde(rename = "trackTimeMillis")]
    track_time_millis: Option<u32>,
    #[serde(rename = "trackExplicitness")]
    track_explicitness: Option<String>,
    #[serde(rename = "artworkUrl100")]
    artwork_url_100: Option<String>,
    #[serde(rename = "discNumber")]
    disc_number: Option<u32>,
    #[serde(rename = "trackNumber")]
    track_number: Option<u32>,
}

#[derive(Deserialize)]
struct ItCollection {
    #[serde(rename = "collectionId")]
    collection_id: i64,
    #[serde(rename = "collectionName")]
    collection_name: String,
    #[serde(rename = "artistId")]
    artist_id: Option<i64>,
    #[serde(rename = "artistName")]
    artist_name: Option<String>,
    #[serde(rename = "releaseDate")]
    release_date: Option<String>,
    #[serde(rename = "trackCount")]
    track_count: Option<u32>,
    #[serde(rename = "artworkUrl100")]
    artwork_url_100: Option<String>,
}

#[derive(Deserialize)]
struct ItArtist {
    #[serde(rename = "artistId")]
    artist_id: i64,
    #[serde(rename = "artistName")]
    artist_name: String,
}

fn artwork(url100: Option<String>) -> Vec<Image> {
    match url100.filter(|u| !u.is_empty()) {
        Some(u) => vec![
            Image { url: big_artwork(&u), width: Some(1000), height: Some(1000) },
            Image { url: u, width: Some(100), height: Some(100) },
        ],
        None => vec![],
    }
}

fn track_artists(id: Option<i64>, name: Option<String>) -> Vec<ArtistRef> {
    name.into_iter().map(|name| ArtistRef { id: id.map(|i| i.to_string()), name }).collect()
}

fn track(t: ItTrack) -> Track {
    Track {
        id: t.track_id.to_string(),
        title: t.track_name,
        artists: track_artists(t.artist_id, t.artist_name),
        album: t
            .collection_name
            .map(|title| AlbumRef { id: t.collection_id.map(|i| i.to_string()), title }),
        duration_ms: t.track_time_millis,
        // The search/lookup API never carries an ISRC.
        isrc: None,
        explicit: t.track_explicitness.as_deref() == Some("explicit"),
        artwork: artwork(t.artwork_url_100),
    }
}

fn album(a: ItCollection) -> Album {
    Album {
        id: a.collection_id.to_string(),
        title: a.collection_name,
        artists: track_artists(a.artist_id, a.artist_name),
        year: a.release_date.and_then(|d| d.get(..4)?.parse().ok()).filter(|&y| y > 0),
        track_count: a.track_count,
        artwork: artwork(a.artwork_url_100),
    }
}

fn artist(a: ItArtist) -> Artist {
    // The artist search/lookup payload carries no artwork of its own.
    Artist { id: a.artist_id.to_string(), name: a.artist_name, artwork: vec![] }
}

/// Splits one response's `results` by `wrapperType`; an item that fails to deserialise into
/// its own shape is skipped, not the whole response.
pub fn parse_results(body: &[u8]) -> Result<SearchParts, Error> {
    let env = envelope(body)?;
    let mut parts = SearchParts { total: env.result_count, ..SearchParts::default() };
    for item in env.results {
        match item.get("wrapperType").and_then(|v| v.as_str()) {
            Some("track") => {
                if let Ok(t) = serde_json::from_value::<ItTrack>(item) {
                    parts.tracks.push(track(t));
                }
            }
            Some("collection") => {
                if let Ok(a) = serde_json::from_value::<ItCollection>(item) {
                    parts.albums.push(album(a));
                }
            }
            Some("artist") => {
                if let Ok(a) = serde_json::from_value::<ItArtist>(item) {
                    parts.artists.push(artist(a));
                }
            }
            _ => {}
        }
    }
    Ok(parts)
}

/// A single track lookup (`lookup?id=<id>&entity=song`): the first (only) `"track"` item.
pub fn parse_track(body: &[u8]) -> Result<Track, Error> {
    let env = envelope(body)?;
    env.results
        .into_iter()
        .find(|v| v.get("wrapperType").and_then(|v| v.as_str()) == Some("track"))
        .and_then(|v| serde_json::from_value::<ItTrack>(v).ok())
        .map(track)
        .ok_or(Error::NotFound)
}

/// An album lookup (`lookup?id=<id>&entity=song`): the one `"collection"` item is the album,
/// every `"track"` item after it is a track, ordered by disc then track number so a
/// multi-disc album doesn't interleave its discs.
pub fn parse_album_tracks(body: &[u8]) -> Result<(Album, Vec<Track>), Error> {
    let env = envelope(body)?;
    let mut found_album = None;
    let mut tracks: Vec<(Option<u32>, Option<u32>, Track)> = vec![];
    for item in env.results {
        match item.get("wrapperType").and_then(|v| v.as_str()) {
            Some("collection") if found_album.is_none() => {
                if let Ok(a) = serde_json::from_value::<ItCollection>(item) {
                    found_album = Some(album(a));
                }
            }
            Some("track") => {
                if let Ok(t) = serde_json::from_value::<ItTrack>(item) {
                    tracks.push((t.disc_number, t.track_number, track(t)));
                }
            }
            _ => {}
        }
    }
    tracks.sort_by_key(|(disc_number, track_number, _)| (*disc_number, *track_number));
    Ok((found_album.ok_or(Error::NotFound)?, tracks.into_iter().map(|(_, _, t)| t).collect()))
}

/// An artist lookup (`lookup?id=<id>&entity=album`): the one `"artist"` item, followed by its
/// albums as `"collection"` items, in the order iTunes returns them.
pub fn parse_artist_albums(body: &[u8]) -> Result<(Artist, Vec<Album>), Error> {
    let env = envelope(body)?;
    let mut found_artist = None;
    let mut albums = vec![];
    for item in env.results {
        match item.get("wrapperType").and_then(|v| v.as_str()) {
            Some("artist") if found_artist.is_none() => {
                if let Ok(a) = serde_json::from_value::<ItArtist>(item) {
                    found_artist = Some(artist(a));
                }
            }
            Some("collection") => {
                if let Ok(a) = serde_json::from_value::<ItCollection>(item) {
                    albums.push(album(a));
                }
            }
            _ => {}
        }
    }
    Ok((found_artist.ok_or(Error::NotFound)?, albums))
}

/// `music.apple.com/<cc>/album/<slug>/<id>` (`?i=<trackId>` names that track instead),
/// `/artist/<slug>/<id>`, `/song/<slug>/<id>`; `itunes.apple.com` legacy links (and the
/// region-redirected `geo.music.apple.com`) in the same shape. Scheme and host are matched in
/// any case; a playlist link (and anything else iTunes does not serve) is `None`, not an
/// error.
pub fn parse_link(url: &str) -> Option<EntityRef> {
    let (host, rest) = split(url)?;
    if host != "music.apple.com" && host != "itunes.apple.com" && host != "geo.music.apple.com" {
        return None;
    }
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    // A fragment can trail either the bare path (no `?`) or the query string; strip it from
    // whichever one actually has it, before the query is read for `i=`.
    let query = query.split('#').next().unwrap_or(query);
    let path = path.split('#').next().unwrap_or(path).trim_end_matches('/').to_ascii_lowercase();
    let parts: Vec<&str> = path.split('/').collect();
    let [cc, kind, _slug, raw_id] = parts[..] else { return None };
    if cc.len() != 2 || !cc.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    // Legacy `itunes.apple.com` links carry `id<digits>`; current `music.apple.com` ones don't.
    let id = raw_id.strip_prefix("id").filter(|d| is_id(d)).unwrap_or(raw_id);
    if !is_id(id) {
        return None;
    }
    match kind {
        "song" => Some(EntityRef { kind: EntityKind::Track, id: id.to_string() }),
        "album" => match query.split('&').find_map(|p| p.strip_prefix("i=")) {
            Some(track_id) => is_id(track_id).then(|| EntityRef { kind: EntityKind::Track, id: track_id.to_string() }),
            None => Some(EntityRef { kind: EntityKind::Album, id: id.to_string() }),
        },
        "artist" => Some(EntityRef { kind: EntityKind::Artist, id: id.to_string() }),
        // Playlists (and anything else) are not served by this API.
        _ => None,
    }
}

/// iTunes ids are decimal: anything else never reaches a URL.
pub fn is_id(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// An http(s) URL's host, lowercased, and its path as given (without the leading `/`).
fn split(url: &str) -> Option<(String, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return None;
    }
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    Some((host.to_ascii_lowercase(), path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dbg<T: std::fmt::Debug>(v: T) -> String {
        format!("{v:?}")
    }

    macro_rules! fixture {
        ($path:literal) => {
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/itunes/", $path))
        };
    }

    #[test]
    fn songs_albums_artists_are_split() {
        let songs = parse_results(fixture!("search/search_song.json")).unwrap();
        assert_eq!((songs.tracks.len(), songs.albums.len(), songs.artists.len(), songs.total), (2, 0, 0, 2));
        assert_eq!(songs.tracks[0].title, "Instant Crush");
        assert_eq!(songs.tracks[0].duration_ms, Some(337_562));
        assert_eq!(songs.tracks[0].artists[0].name, "Daft Punk & Julian Casablancas");
        assert_eq!(
            dbg(&songs.tracks[0].album),
            dbg(Some(AlbumRef { id: Some("617154241".into()), title: "Random Access Memories".into() }))
        );

        let albums = parse_results(fixture!("search/search_album.json")).unwrap();
        assert_eq!((albums.tracks.len(), albums.albums.len(), albums.artists.len()), (0, 2, 0));
        assert_eq!((albums.albums[0].title.as_str(), albums.albums[0].year, albums.albums[0].track_count), ("Random Access Memories", Some(2013), Some(14)));
        assert_eq!(albums.albums[0].artists[0].name, "Daft Punk");

        let artists = parse_results(fixture!("search/search_artist.json")).unwrap();
        assert_eq!((artists.tracks.len(), artists.albums.len(), artists.artists.len()), (0, 0, 2));
        assert_eq!(artists.artists[0].id.as_str(), "5468295");
        assert_eq!(artists.artists[0].name, "Daft Punk");
    }

    /// A hand-made, deliberately shuffled two-disc lookup body. The old `(track_number,
    /// disc_number)` sort key groups by track number first, so it interleaves discs
    /// (d1t1, d2t1, d1t2, d2t2); the correct `(disc_number, track_number)` key keeps each
    /// disc together (d1t1, d1t2, d2t1, d2t2).
    #[test]
    fn album_tracks_sort_by_disc_then_track_number() {
        let body = br#"{"resultCount":5,"results":[
            {"wrapperType":"collection","collectionId":1,"collectionName":"Two Discs"},
            {"wrapperType":"track","trackId":10,"trackName":"D1T2","discNumber":1,"trackNumber":2},
            {"wrapperType":"track","trackId":11,"trackName":"D2T1","discNumber":2,"trackNumber":1},
            {"wrapperType":"track","trackId":12,"trackName":"D1T1","discNumber":1,"trackNumber":1},
            {"wrapperType":"track","trackId":13,"trackName":"D2T2","discNumber":2,"trackNumber":2}
        ]}"#;
        let (_album, tracks) = parse_album_tracks(body).unwrap();
        let names: Vec<&str> = tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(names, ["D1T1", "D1T2", "D2T1", "D2T2"]);
    }

    #[test]
    fn explicit_track_is_flagged() {
        let body = br#"{"resultCount":2,"results":[
            {"wrapperType":"track","trackId":1,"trackName":"Clean","trackExplicitness":"notExplicit"},
            {"wrapperType":"track","trackId":2,"trackName":"Dirty","trackExplicitness":"explicit"}
        ]}"#;
        let parts = parse_results(body).unwrap();
        assert!(!parts.tracks[0].explicit, "{:?}", parts.tracks[0]);
        assert!(parts.tracks[1].explicit, "{:?}", parts.tracks[1]);
    }

    #[test]
    fn one_bad_item_does_not_fail_the_page() {
        let body = br#"{"resultCount":3,"results":[
            {"wrapperType":"track","trackId":1,"trackName":"Good"},
            {"wrapperType":"track","trackName":"Missing id"},
            {"wrapperType":"artist","artistId":2,"artistName":"A"}
        ]}"#;
        let parts = parse_results(body).unwrap();
        assert_eq!(parts.tracks.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["Good"]);
        assert_eq!(parts.artists.len(), 1);
        assert_eq!(parts.total, 3);
    }

    #[test]
    fn artwork_is_upscaled_to_1000() {
        assert_eq!(
            big_artwork("https://is1-ssl.mzstatic.com/image/thumb/x/100x100bb.jpg"),
            "https://is1-ssl.mzstatic.com/image/thumb/x/1000x1000bb.jpg"
        );
        let songs = parse_results(fixture!("search/search_song.json")).unwrap();
        let art = &songs.tracks[0].artwork;
        assert_eq!(art.len(), 2);
        assert!(art[0].url.ends_with("1000x1000bb.jpg"));
        assert_eq!((art[0].width, art[0].height), (Some(1000), Some(1000)));
        assert!(art[1].url.ends_with("100x100bb.jpg"));
        assert_eq!((art[1].width, art[1].height), (Some(100), Some(100)));
    }

    #[test]
    fn itunes_403_is_rate_limited() {
        assert_eq!(dbg(status_error(403)), dbg(Some(Error::RateLimited)));
        // Everything else still goes through the SDK's own mapping.
        assert_eq!(dbg(status_error(404)), dbg(Some(Error::NotFound)));
        assert_eq!(dbg(status_error(429)), dbg(Some(Error::RateLimited)));
        assert!(status_error(200).is_none());
    }

    #[test]
    fn album_and_track_links_are_recognised() {
        fn link(url: &str) -> Option<(EntityKind, String)> {
            parse_link(url).map(|r| (r.kind, r.id))
        }
        assert_eq!(
            link("https://music.apple.com/fr/album/random-access-memories/617154241"),
            Some((EntityKind::Album, "617154241".into()))
        );
        // `?i=<trackId>` on an album link names that track, not the album.
        assert_eq!(
            link("https://music.apple.com/fr/album/instant-crush/617154241?i=617154362&uo=4"),
            Some((EntityKind::Track, "617154362".into()))
        );
        assert_eq!(
            link("https://music.apple.com/us/artist/daft-punk/5468295?uo=4"),
            Some((EntityKind::Artist, "5468295".into()))
        );
        // Any case in the scheme and the host.
        assert_eq!(
            link("HTTPS://Music.Apple.COM/FR/album/random-access-memories/617154241"),
            Some((EntityKind::Album, "617154241".into()))
        );
        // Legacy host, `id`-prefixed numeric segment.
        assert_eq!(
            link("https://itunes.apple.com/us/album/random-access-memories/id617154241"),
            Some((EntityKind::Album, "617154241".into()))
        );
        assert_eq!(
            link("https://itunes.apple.com/us/artist/daft-punk/id5468295"),
            Some((EntityKind::Artist, "5468295".into()))
        );
        // A direct song link (no album context at all).
        assert_eq!(link("https://music.apple.com/us/song/instant-crush/617154362"), Some((EntityKind::Track, "617154362".into())));
        // The region-redirected host, same shape.
        assert_eq!(
            link("https://geo.music.apple.com/fr/album/random-access-memories/617154241"),
            Some((EntityKind::Album, "617154241".into()))
        );
        // A fragment after the `i=` query is stripped before the track id is read.
        assert_eq!(
            link("https://music.apple.com/fr/album/instant-crush/617154241?i=617154362#t"),
            Some((EntityKind::Track, "617154362".into()))
        );
        // Playlists are not served by this API; another domain is not ours.
        assert_eq!(link("https://music.apple.com/fr/playlist/french-touch/pl.abc123"), None);
        assert_eq!(link("https://open.spotify.com/album/617154241"), None);
        assert_eq!(link("https://music.apple.com.evil.com/fr/album/x/617154241"), None);
        assert_eq!(link("https://music.apple.com/fr/album/x/not-a-number"), None);
    }
}
