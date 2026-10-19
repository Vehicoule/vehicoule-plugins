//! Deezer's JSON to contract values. Every body goes through `deezer_error` first: Deezer
//! answers HTTP 200 with `{"error": …}` for its quota and for unknown ids.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use vehicoule_sdk::types::{Album, AlbumRef, Artist, ArtistRef, Error, Image, Playlist, Track};

/// A page of results and the cursor of the next one: Deezer's `index`, in decimal.
pub type Page<T> = (Vec<T>, Option<String>);

/// `{"error":{"code":4}}` (quota) → `rate-limited`, 800 (no data) → `not-found`.
pub fn deezer_error(body: &[u8]) -> Option<Error> {
    #[derive(Deserialize)]
    struct Body {
        error: Option<Code>,
    }
    #[derive(Deserialize)]
    struct Code {
        #[serde(default)]
        code: i64,
    }
    let error = serde_json::from_slice::<Body>(body).ok()?.error?;
    Some(match error.code {
        4 => Error::RateLimited,
        800 => Error::NotFound,
        code => Error::Internal(format!("deezer {code}")),
    })
}

pub fn parse_tracks(body: &[u8]) -> Result<Page<Track>, Error> {
    page(body, track)
}

pub fn parse_albums(body: &[u8]) -> Result<Page<Album>, Error> {
    page(body, album)
}

pub fn parse_artists(body: &[u8]) -> Result<Page<Artist>, Error> {
    page(body, artist)
}

pub fn parse_playlists(body: &[u8]) -> Result<Page<Playlist>, Error> {
    page(body, playlist)
}

pub fn parse_track(body: &[u8]) -> Result<Track, Error> {
    parse(body).map(track)
}

pub fn parse_album(body: &[u8]) -> Result<Album, Error> {
    parse(body).map(album)
}

pub fn parse_artist(body: &[u8]) -> Result<Artist, Error> {
    parse(body).map(artist)
}

pub fn parse_playlist(body: &[u8]) -> Result<Playlist, Error> {
    parse(body).map(playlist)
}

fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T, Error> {
    if let Some(e) = deezer_error(body) {
        return Err(e);
    }
    serde_json::from_slice(body).map_err(|e| Error::Internal(e.to_string()))
}

#[derive(Deserialize)]
struct List<T> {
    data: Vec<T>,
    /// The URL of the next page, absent on the last one.
    next: Option<String>,
}

fn page<D: DeserializeOwned, T>(body: &[u8], map: fn(D) -> T) -> Result<Page<T>, Error> {
    let list: List<D> = parse(body)?;
    Ok((list.data.into_iter().map(map).collect(), list.next.as_deref().and_then(next_index)))
}

/// `…?limit=100&index=100` → `100`.
fn next_index(next: &str) -> Option<String> {
    let (_, query) = next.split_once('?')?;
    let index = query.split('&').find_map(|p| p.strip_prefix("index="))?;
    crate::links::is_id(index).then(|| index.to_string())
}

// Only the fields in use. A reference (`track.artist`, `track.album`) is the same object
// with fewer fields.

#[derive(Deserialize)]
struct DzTrack {
    id: i64,
    title: String,
    /// Seconds.
    duration: Option<u32>,
    isrc: Option<String>,
    #[serde(default)]
    explicit_lyrics: bool,
    artist: DzArtist,
    album: Option<DzAlbum>,
}

#[derive(Deserialize)]
struct DzArtist {
    id: i64,
    name: String,
    picture_xl: Option<String>,
    picture_medium: Option<String>,
}

#[derive(Deserialize)]
struct DzAlbum {
    id: i64,
    title: String,
    artist: Option<DzArtist>,
    /// `YYYY-MM-DD`.
    release_date: Option<String>,
    nb_tracks: Option<u32>,
    cover_xl: Option<String>,
    cover_medium: Option<String>,
}

#[derive(Deserialize)]
struct DzPlaylist {
    id: i64,
    title: String,
    /// `user` in search results.
    #[serde(alias = "user")]
    creator: Option<Named>,
    description: Option<String>,
    nb_tracks: Option<u32>,
    picture_xl: Option<String>,
    picture_medium: Option<String>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

/// 1000 px then 250 px; empty URLs are skipped.
fn artwork(xl: Option<String>, medium: Option<String>) -> Vec<Image> {
    [(xl, 1000), (medium, 250)]
        .into_iter()
        .filter_map(|(url, px)| url.filter(|u| !u.is_empty()).map(|url| Image { url, width: Some(px), height: Some(px) }))
        .collect()
}

fn artist_ref(a: DzArtist) -> ArtistRef {
    ArtistRef { id: Some(a.id.to_string()), name: a.name }
}

/// The artwork is the album's cover, when the album is given.
fn track(t: DzTrack) -> Track {
    let (album, artwork) = match t.album {
        Some(a) => (Some(AlbumRef { id: Some(a.id.to_string()), title: a.title }), artwork(a.cover_xl, a.cover_medium)),
        None => (None, vec![]),
    };
    Track {
        id: t.id.to_string(),
        title: t.title,
        artists: vec![artist_ref(t.artist)],
        album,
        duration_ms: t.duration.filter(|&s| s > 0).map(|s| s.saturating_mul(1000)),
        isrc: t.isrc.filter(|i| !i.is_empty()),
        explicit: t.explicit_lyrics,
        artwork,
    }
}

fn album(a: DzAlbum) -> Album {
    Album {
        id: a.id.to_string(),
        title: a.title,
        artists: a.artist.map(artist_ref).into_iter().collect(),
        year: a.release_date.and_then(|d| d.get(..4)?.parse().ok()).filter(|&y| y > 0),
        track_count: a.nb_tracks,
        artwork: artwork(a.cover_xl, a.cover_medium),
    }
}

fn artist(a: DzArtist) -> Artist {
    Artist { id: a.id.to_string(), name: a.name, artwork: artwork(a.picture_xl, a.picture_medium) }
}

fn playlist(p: DzPlaylist) -> Playlist {
    Playlist {
        id: p.id.to_string(),
        title: p.title,
        owner: p.creator.map(|c| c.name),
        description: p.description.filter(|d| !d.is_empty()),
        track_count: p.nb_tracks,
        artwork: artwork(p.picture_xl, p.picture_medium),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vehicoule_sdk::types::{AlbumRef, ArtistRef, Image};

    fn dbg<T: std::fmt::Debug>(v: T) -> String {
        format!("{v:?}")
    }

    macro_rules! fixture {
        ($path:literal) => {
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/deezer/", $path))
        };
    }

    const COVER: &str = "https://cdn-images.dzcdn.net/images/cover/311bba0fc112d15f72c8b5a65f0456c1";

    #[test]
    fn search_results_parse_with_cursor() {
        let (tracks, next) = parse_tracks(fixture!("search/search_track.json")).unwrap();
        assert_eq!((tracks.len(), next.as_deref()), (2, Some("2")));
        let t = &tracks[0];
        assert_eq!(t.id, "67238732");
        assert_eq!(t.title, "Instant Crush (feat. Julian Casablancas)");
        assert_eq!(dbg(&t.artists), dbg([ArtistRef { id: Some("27".into()), name: "Daft Punk".into() }]));
        assert_eq!(
            dbg(&t.album),
            dbg(Some(AlbumRef { id: Some("6575789".into()), title: "Random Access Memories".into() }))
        );
        assert_eq!(t.duration_ms, Some(337_000));
        assert_eq!(t.isrc.as_deref(), Some("USQX91300105"));
        assert!(!t.explicit);
        assert_eq!(
            dbg(&t.artwork),
            dbg([
                Image { url: format!("{COVER}/1000x1000-000000-80-0-0.jpg"), width: Some(1000), height: Some(1000) },
                Image { url: format!("{COVER}/250x250-000000-80-0-0.jpg"), width: Some(250), height: Some(250) },
            ])
        );

        let (albums, next) = parse_albums(fixture!("search/search_album.json")).unwrap();
        assert_eq!((albums.len(), next.as_deref()), (2, Some("2")));
        let a = &albums[0];
        assert_eq!((a.id.as_str(), a.title.as_str(), a.track_count), ("6575789", "Random Access Memories", Some(13)));
        assert_eq!(a.artists[0].name, "Daft Punk");

        let (artists, next) = parse_artists(fixture!("search/search_artist.json")).unwrap();
        assert_eq!((artists.len(), next.as_deref()), (2, Some("2")));
        assert_eq!((artists[0].id.as_str(), artists[0].name.as_str()), ("27", "Daft Punk"));
        assert_eq!(artists[0].artwork[0].width, Some(1000));

        let (playlists, next) = parse_playlists(fixture!("search/search_playlist.json")).unwrap();
        assert_eq!((playlists.len(), next.as_deref()), (2, Some("2")));
        let p = &playlists[0];
        assert_eq!((p.id.as_str(), p.title.as_str(), p.track_count), ("6423436964", "French Touch Essentials", Some(50)));
        assert_eq!(p.owner.as_deref(), Some("Laeti - Deezer Dance & EDM Editor"));
    }

    #[test]
    fn last_page_has_no_cursor() {
        let (tracks, next) = parse_tracks(fixture!("playlist/tracks_100.json")).unwrap();
        assert_eq!((tracks.len(), next), (2, None));
        let (tracks, next) = parse_tracks(fixture!("empty/search_track.json")).unwrap();
        assert_eq!((tracks.len(), next), (0, None));
    }

    #[test]
    fn album_tracks_carry_isrc_when_present() {
        let (tracks, _) = parse_tracks(fixture!("entity/album_tracks.json")).unwrap();
        assert_eq!(tracks.len(), 14);
        assert_eq!((tracks[0].title.as_str(), tracks[0].isrc.as_deref()), ("One More Time", Some("GBDUW0000053")));
        // No `album` object in an album's own track list: the caller fills it in.
        assert!(tracks[0].album.is_none() && tracks[0].artwork.is_empty());
        // Radio tracks come without an ISRC.
        let (radio, _) = parse_tracks(fixture!("radio/artist_radio.json")).unwrap();
        assert!(radio.iter().all(|t| t.isrc.is_none()), "{radio:?}");
    }

    #[test]
    fn deezer_json_errors_are_typed() {
        let not_found = fixture!("entity/track_unknown.json");
        assert_eq!(dbg(deezer_error(not_found)), dbg(Some(Error::NotFound)));
        let quota = br#"{"error":{"type":"Exception","message":"Quota limit exceeded","code":4}}"#;
        assert_eq!(dbg(deezer_error(quota)), dbg(Some(Error::RateLimited)));
        let other = br#"{"error":{"type":"ParameterException","message":"Wrong parameter","code":500}}"#;
        assert_eq!(dbg(deezer_error(other)), dbg(Some(Error::Internal("deezer 500".into()))));
        assert!(deezer_error(fixture!("search/search_track.json")).is_none());
        // Checked before the data is parsed, by every parser.
        assert_eq!(dbg(parse_tracks(quota)), dbg(Err::<(), _>(Error::RateLimited)));
        assert_eq!(dbg(parse_track(not_found)), dbg(Err::<(), _>(Error::NotFound)));
        assert_eq!(dbg(parse_playlist(not_found)), dbg(Err::<(), _>(Error::NotFound)));
    }

    #[test]
    fn single_entities_parse() {
        let a = parse_album(fixture!("entity/album.json")).unwrap();
        assert_eq!((a.id.as_str(), a.title.as_str(), a.year, a.track_count), ("302127", "Discovery", Some(2001), Some(14)));
        let p = parse_playlist(fixture!("playlist/playlist.json")).unwrap();
        assert_eq!((p.title.as_str(), p.owner.as_deref(), p.track_count), ("Musique française années 80", Some("Digster France"), Some(102)));
        assert!(p.description.as_deref().unwrap().starts_with("Le meilleur de la pop française"));
        let t = parse_track(fixture!("radio/track.json")).unwrap();
        assert_eq!((t.id.as_str(), t.artists[0].id.as_deref()), ("3135556", Some("27")));
    }

    #[test]
    fn artwork_is_xl_then_medium_and_skips_empty() {
        let body = br#"{"data":[{"id":1,"name":"A","picture_xl":"","picture_medium":"https://e.dzcdn.net/m.jpg"},
            {"id":2,"name":"B","picture_xl":null}]}"#;
        let (artists, _) = parse_artists(body).unwrap();
        assert_eq!(dbg(&artists[0].artwork), dbg([Image { url: "https://e.dzcdn.net/m.jpg".into(), width: Some(250), height: Some(250) }]));
        assert!(artists[1].artwork.is_empty());
    }
}
