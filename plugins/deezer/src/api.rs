//! Deezer's JSON to contract values. Every body goes through `deezer_error` first: Deezer
//! answers HTTP 200 with `{"error": …}` for its quota and for unknown ids.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use vehicoule_sdk::types::{Album, AlbumRef, Artist, ArtistRef, Error, Image, Playlist, Track};

use crate::exports::vehicoule::music::browse::{Category, Release, Shelf};
use crate::exports::vehicoule::music::entity::Entity;

/// A page of results and the cursor of the next one: Deezer's `index`, in decimal.
pub type Page<T> = (Vec<T>, Option<String>);

/// `{"error":{"code":4}}` (quota) and 700 (service busy) → `rate-limited`, 800 (no data) →
/// `not-found`.
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
        4 | 700 => Error::RateLimited,
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

/// A browse category id: `chart:<genre>`, `genre:<genre>` or `selection:<genre>`.
#[derive(Debug)]
pub enum CategoryId<'a> {
    Chart(&'a str),
    Genre(&'a str),
    Selection(&'a str),
}

/// Anything else, or a genre that is not decimal, is `not-found` before any request.
pub fn category_id(id: &str) -> Result<CategoryId<'_>, Error> {
    match id.split_once(':') {
        Some((kind, g)) if crate::links::is_id(g) => match kind {
            "chart" => Ok(CategoryId::Chart(g)),
            "genre" => Ok(CategoryId::Genre(g)),
            "selection" => Ok(CategoryId::Selection(g)),
            _ => Err(Error::NotFound),
        },
        _ => Err(Error::NotFound),
    }
}

/// `/genre` as the browse grid, without "all" (id 0): that one is the featured chart.
pub fn parse_genres(body: &[u8]) -> Result<Vec<Category>, Error> {
    #[derive(Deserialize)]
    struct DzGenre {
        id: i64,
        name: String,
        picture_xl: Option<String>,
        picture_medium: Option<String>,
    }
    let list: List = parse(body)?;
    Ok(items(list.data, |g: DzGenre| g)
        .filter(|g| g.id != 0)
        .map(|g| Category {
            id: format!("genre:{}", g.id),
            title: g.name,
            artwork: artwork(g.picture_xl, g.picture_medium),
            featured: false,
        })
        .collect())
}

/// `/chart/<genre>` as shelves: tracks, albums, artists, playlists (podcasts are left out).
pub fn parse_chart(body: &[u8]) -> Result<Vec<Shelf>, Error> {
    #[derive(Deserialize)]
    struct Chart {
        #[serde(default)]
        tracks: List,
        #[serde(default)]
        albums: List,
        #[serde(default)]
        artists: List,
        #[serde(default)]
        playlists: List,
    }
    let c: Chart = parse(body)?;
    Ok(shelves([
        (
            "tracks",
            items(c.tracks.data, track).map(Entity::Track).collect(),
        ),
        (
            "albums",
            items(c.albums.data, album).map(Entity::Album).collect(),
        ),
        (
            "artists",
            items(c.artists.data, artist).map(Entity::Artist).collect(),
        ),
        (
            "playlists",
            items(c.playlists.data, playlist)
                .map(Entity::Playlist)
                .collect(),
        ),
    ]))
}

/// `/editorial/<genre>/selection`: one shelf of albums.
pub fn parse_selection(body: &[u8]) -> Result<Vec<Shelf>, Error> {
    let (albums, _) = parse_albums(body)?;
    Ok(shelves([(
        "albums",
        albums.into_iter().map(Entity::Album).collect(),
    )]))
}

/// An artist's albums, newest first; those without a full `YYYY-MM-DD` date are dropped.
pub fn parse_releases(body: &[u8]) -> Result<Vec<Release>, Error> {
    let list: List = parse(body)?;
    let mut releases: Vec<Release> = items(list.data, |a: DzAlbum| a)
        .filter_map(|a| {
            let date = a.release_date.clone().filter(|d| is_date(d))?;
            Some(Release {
                album: album(a),
                date,
            })
        })
        .collect();
    releases.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(releases)
}

/// `YYYY-MM-DD`, year not zero (Deezer's `0000-00-00`).
fn is_date(d: &str) -> bool {
    d.len() == 10
        && !d.starts_with("0000")
        && d.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

/// Empty shelves are left out.
fn shelves<const N: usize>(shelves: [(&str, Vec<Entity>); N]) -> Vec<Shelf> {
    shelves
        .into_iter()
        .filter(|(_, items)| !items.is_empty())
        .map(|(title, items)| Shelf {
            title: title.into(),
            items,
        })
        .collect()
}

fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T, Error> {
    if let Some(e) = deezer_error(body) {
        return Err(e);
    }
    serde_json::from_slice(body).map_err(|e| Error::Internal(e.to_string()))
}

#[derive(Deserialize, Default)]
struct List {
    data: Vec<serde_json::Value>,
    /// The URL of the next page, absent on the last one.
    next: Option<String>,
}

fn page<D: DeserializeOwned, T>(body: &[u8], map: fn(D) -> T) -> Result<Page<T>, Error> {
    let list: List = parse(body)?;
    Ok((
        items(list.data, map).collect(),
        list.next.as_deref().and_then(next_index),
    ))
}

/// An item Deezer sends malformed (a null title…) is skipped, not the whole list.
fn items<D: DeserializeOwned, T>(
    data: Vec<serde_json::Value>,
    map: fn(D) -> T,
) -> impl Iterator<Item = T> {
    data.into_iter()
        .filter_map(|item| serde_json::from_value(item).ok())
        .map(map)
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
        let busy = br#"{"error":{"type":"Exception","message":"Service busy","code":700}}"#;
        assert_eq!(dbg(deezer_error(busy)), dbg(Some(Error::RateLimited)));
        let other = br#"{"error":{"type":"ParameterException","message":"Wrong parameter","code":500}}"#;
        assert_eq!(dbg(deezer_error(other)), dbg(Some(Error::Internal("deezer 500".into()))));
        assert!(deezer_error(fixture!("search/search_track.json")).is_none());
        // Checked before the data is parsed, by every parser.
        assert_eq!(dbg(parse_tracks(quota)), dbg(Err::<(), _>(Error::RateLimited)));
        assert_eq!(dbg(parse_track(not_found)), dbg(Err::<(), _>(Error::NotFound)));
        assert_eq!(dbg(parse_playlist(not_found)), dbg(Err::<(), _>(Error::NotFound)));
    }

    #[test]
    fn one_bad_item_does_not_fail_the_page() {
        let body = br#"{"data":[{"id":1,"title":"Good","artist":{"id":2,"name":"A"}},
            {"id":3,"title":null,"artist":{"id":2,"name":"A"}},{"id":"x"}],
            "next":"https://api.deezer.com/search/track?q=a&limit=3&index=3"}"#;
        let (tracks, next) = parse_tracks(body).unwrap();
        assert_eq!(tracks.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["Good"]);
        assert_eq!(next.as_deref(), Some("3"));
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

    #[test]
    fn genres_parse_without_all() {
        let genres = parse_genres(fixture!("browse_genres/genre.json")).unwrap();
        assert_eq!(genres.len(), 25);
        assert!(
            genres
                .iter()
                .all(|c| !c.featured && c.id != "genre:0" && c.id.starts_with("genre:")),
            "{genres:?}"
        );
        let pop = &genres[0];
        assert_eq!((pop.id.as_str(), pop.title.as_str()), ("genre:132", "Pop"));
        let pic = "https://cdn-images.dzcdn.net/images/misc/db7a604d9e7634a67d45cfc86b48370a";
        assert_eq!(
            dbg(&pop.artwork),
            dbg([
                Image {
                    url: format!("{pic}/1000x1000-000000-80-0-0.jpg"),
                    width: Some(1000),
                    height: Some(1000)
                },
                Image {
                    url: format!("{pic}/250x250-000000-80-0-0.jpg"),
                    width: Some(250),
                    height: Some(250)
                },
            ])
        );
    }

    fn shelf_sizes(shelves: &[Shelf]) -> Vec<(&str, usize)> {
        shelves
            .iter()
            .map(|s| (s.title.as_str(), s.items.len()))
            .collect()
    }

    #[test]
    fn chart_parses_into_four_ordered_shelves() {
        let shelves = parse_chart(fixture!("browse_chart/chart.json")).unwrap();
        assert_eq!(
            shelf_sizes(&shelves),
            [
                ("tracks", 50),
                ("albums", 49),
                ("artists", 50),
                ("playlists", 50)
            ]
        );
        let Entity::Track(t) = &shelves[0].items[0] else {
            panic!("{:?}", shelves[0].items[0])
        };
        assert_eq!((t.id.as_str(), t.title.as_str()), ("4015341011", "Dai Dai"));
        let Entity::Album(a) = &shelves[1].items[0] else {
            panic!("{:?}", shelves[1].items[0])
        };
        assert_eq!(a.title, "Dopamine Chamber");
        let Entity::Artist(ar) = &shelves[2].items[0] else {
            panic!("{:?}", shelves[2].items[0])
        };
        assert_eq!(ar.name, "Jul");
        let Entity::Playlist(p) = &shelves[3].items[0] else {
            panic!("{:?}", shelves[3].items[0])
        };
        assert_eq!(
            (p.title.as_str(), p.owner.as_deref()),
            (
                "Les titres du moment",
                Some("Alexandre - Pop & Hits Editor")
            )
        );

        // An empty or missing section is no shelf; podcasts are not music.
        let body = br#"{"tracks":{"data":[]},"albums":{"data":[{"id":1,"title":"A"}]},"artists":{"data":[]},
            "podcasts":{"data":[{"id":9,"title":"P"}]}}"#;
        assert_eq!(shelf_sizes(&parse_chart(body).unwrap()), [("albums", 1)]);

        let selection = parse_selection(fixture!("browse_selection/selection.json")).unwrap();
        assert_eq!(shelf_sizes(&selection), [("albums", 10)]);
        let Entity::Album(a) = &selection[0].items[0] else {
            panic!("{:?}", selection[0].items[0])
        };
        assert_eq!(
            (a.title.as_str(), a.artists[0].name.as_str()),
            ("650", "Wyatt McCubbin")
        );
    }

    #[test]
    fn releases_sorted_newest_first_undated_dropped() {
        let releases = parse_releases(fixture!("browse_releases/artist_albums.json")).unwrap();
        assert_eq!(releases.len(), 39);
        assert!(
            releases.windows(2).all(|w| w[0].date >= w[1].date),
            "{releases:?}"
        );
        assert_eq!(
            (releases[0].date.as_str(), releases[0].album.title.as_str()),
            ("2023-11-17", "Random Access Memories (Drumless Edition)")
        );
        assert_eq!(releases[38].date, "1997-01-16");

        let body = br#"{"data":[{"id":1,"title":"Old","release_date":"2001-03-07"},{"id":2,"title":"Undated"},
            {"id":3,"title":"Zero","release_date":"0000-00-00"},{"id":4,"title":"New","release_date":"2026-10-02"},
            {"id":5,"title":"Odd","release_date":"2026"}]}"#;
        let releases = parse_releases(body).unwrap();
        let got: Vec<_> = releases
            .iter()
            .map(|r| (r.date.as_str(), r.album.title.as_str()))
            .collect();
        assert_eq!(got, [("2026-10-02", "New"), ("2001-03-07", "Old")]);
    }

    #[test]
    fn unknown_category_id_is_not_found() {
        assert_eq!(
            dbg(category_id("chart:0")),
            dbg(Ok::<_, Error>(CategoryId::Chart("0")))
        );
        assert_eq!(
            dbg(category_id("genre:132")),
            dbg(Ok::<_, Error>(CategoryId::Genre("132")))
        );
        assert_eq!(
            dbg(category_id("selection:0")),
            dbg(Ok::<_, Error>(CategoryId::Selection("0")))
        );
        for id in [
            "foo:1",
            "chart",
            "chart:",
            "genre:pop",
            "chart:0/../user/1",
            "",
        ] {
            assert_eq!(
                dbg(category_id(id)),
                dbg(Err::<(), _>(Error::NotFound)),
                "{id}"
            );
        }
    }
}
