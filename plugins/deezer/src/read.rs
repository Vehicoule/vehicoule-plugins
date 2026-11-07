//! Reads Deezer's answers (crate documentation, "Service").
//!
//! Every answer goes through [`check`] first: the status, the `Content-Type`, and the error object Deezer sends with
//! status `200`. Then each reader parses the JSON into structs that name only the fields the plugin uses, and maps
//! them to [`crate::model`]. A field the plugin needs that is missing or of another type makes the answer unreadable;
//! a field it can do without (a cover, an ISRC, a release date) that is missing, empty or unreadable is left out.

use std::time::Duration;

use serde::Deserialize;

use crate::model::{
    Album, AlbumPage, AlbumRef, Artist, ArtistPage, ArtistRef, Artwork, Error, Item, ItemKind,
    Page, Playlist, PlaylistPage, Track,
};
use crate::request::{ALBUM_TRACKS_LIMIT, SEARCH_LIMIT};

/// The answer to one request, as the host returns it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Response {
    /// The HTTP status.
    pub status: u16,
    /// The headers, as name and value.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Vec<u8>,
}

impl Response {
    /// An answer with this status, headers and body.
    #[must_use]
    pub fn new(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }
}

/// The error object Deezer answers with status `200` (crate documentation, "Service").
#[derive(Deserialize)]
struct ErrorEnvelope {
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ApiError {
    #[serde(rename = "type")]
    kind: Option<String>,
    message: Option<String>,
    code: Option<i64>,
}

/// Deezer's error code for "no data": the item does not exist (crate documentation, "Service").
const NO_DATA: i64 = 800;

/// Checks what every answer must be before its body is read: status `200`, a JSON `Content-Type`, and no error
/// object. An error object with code 800 is [`Error::NotFound`]; any other is [`Error::Upstream`], with its code,
/// type and message. Status `429` is [`Error::RateLimited`], with the delay of `Retry-After` in seconds if given.
///
/// # Errors
///
/// As above.
pub fn check(response: &Response) -> Result<(), Error> {
    match response.status {
        200 => {}
        429 => return Err(Error::RateLimited(retry_after(&response.headers))),
        status => {
            return Err(Error::Upstream(format!(
                "Deezer answered with status {status}"
            )));
        }
    }
    let content_type = header(&response.headers, "content-type").unwrap_or_default();
    if !is_json(content_type) {
        return Err(Error::Upstream(format!(
            "Deezer answered with Content-Type {content_type:?}, not JSON"
        )));
    }
    // An answer that is not an object with an `error` field is read by the caller, which says what is wrong with it.
    if let Ok(ErrorEnvelope { error: Some(error) }) = serde_json::from_slice(&response.body) {
        if error.code == Some(NO_DATA) {
            return Err(Error::NotFound);
        }
        return Err(Error::Upstream(format!(
            "Deezer answered with error {}: {} ({})",
            error
                .code
                .map_or_else(|| String::from("without code"), |c| c.to_string()),
            error.message.unwrap_or_default(),
            error.kind.unwrap_or_default(),
        )));
    }
    Ok(())
}

/// The value of the first header named `name`, which is lowercase; header names are compared without case.
fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// Whether a `Content-Type` value is JSON's media type, with or without parameters (`; charset=utf-8`).
fn is_json(content_type: &str) -> bool {
    content_type
        .split(';')
        .next()
        .is_some_and(|media| media.trim().eq_ignore_ascii_case("application/json"))
}

/// The delay of `Retry-After`, when it is a number of seconds whose milliseconds fit a `u64`; an HTTP date needs a
/// clock to read, which a plugin does not have.
fn retry_after(headers: &[(String, String)]) -> Option<Duration> {
    let seconds: u64 = header(headers, "retry-after")?.trim().parse().ok()?;
    Some(Duration::from_millis(seconds.checked_mul(1_000)?))
}

/// Checks `response` and parses its body as `T`.
fn parse<T: for<'de> Deserialize<'de>>(response: &Response, what: &str) -> Result<T, Error> {
    check(response)?;
    serde_json::from_slice(&response.body)
        .map_err(|e| Error::Upstream(format!("Deezer's {what} is unreadable: {e}")))
}

#[derive(Deserialize)]
struct List<T> {
    data: Vec<T>,
    next: Option<String>,
}

#[derive(Deserialize)]
struct RawArtistRef {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
struct RawAlbumRef {
    id: u64,
    title: String,
    cover_small: Option<String>,
    cover_medium: Option<String>,
    cover_big: Option<String>,
    cover_xl: Option<String>,
}

#[derive(Deserialize)]
struct RawTrack {
    id: u64,
    title: String,
    title_short: Option<String>,
    duration: Option<u64>,
    isrc: Option<String>,
    artist: RawArtistRef,
    contributors: Option<Vec<RawArtistRef>>,
    album: Option<RawAlbumRef>,
}

#[derive(Deserialize)]
struct RawAlbum {
    id: u64,
    title: String,
    release_date: Option<String>,
    artist: Option<RawArtistRef>,
    contributors: Option<Vec<RawArtistRef>>,
    nb_tracks: Option<u64>,
    cover_small: Option<String>,
    cover_medium: Option<String>,
    cover_big: Option<String>,
    cover_xl: Option<String>,
}

#[derive(Deserialize)]
struct RawArtist {
    id: u64,
    name: String,
    picture_small: Option<String>,
    picture_medium: Option<String>,
    picture_big: Option<String>,
    picture_xl: Option<String>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

#[derive(Deserialize)]
struct RawPlaylist {
    id: u64,
    title: String,
    nb_tracks: Option<u64>,
    creator: Option<Named>,
    user: Option<Named>,
    picture_small: Option<String>,
    picture_medium: Option<String>,
    picture_big: Option<String>,
    picture_xl: Option<String>,
    tracks: Option<List<RawTrack>>,
}

#[derive(Deserialize)]
struct Autocomplete {
    tracks: List<RawTrack>,
    albums: List<RawAlbum>,
    artists: List<RawArtist>,
    playlists: List<RawPlaylist>,
    next: Option<String>,
}

/// The pictures Deezer gives in four sizes, from the smallest; each URL that is not `https` is left out.
fn pictures(urls: [&Option<String>; 4]) -> Vec<Artwork> {
    // The sizes in pixels of `*_small`, `*_medium`, `*_big` and `*_xl` (crate documentation, "Service").
    const SIZES: [u32; 4] = [56, 250, 500, 1000];
    urls.into_iter()
        .zip(SIZES)
        .filter_map(|(url, size)| {
            let url = url.as_deref()?;
            url.starts_with("https://").then(|| Artwork {
                url: url.to_owned(),
                size,
            })
        })
        .collect()
}

fn artist_ref(raw: RawArtistRef) -> ArtistRef {
    ArtistRef {
        id: raw.id.to_string(),
        name: raw.name,
    }
}

/// The contributors when Deezer lists any, else the main artist.
fn artists(main: Option<RawArtistRef>, contributors: Option<Vec<RawArtistRef>>) -> Vec<ArtistRef> {
    match contributors.filter(|c| !c.is_empty()) {
        Some(contributors) => contributors.into_iter().map(artist_ref).collect(),
        None => main.into_iter().map(artist_ref).collect(),
    }
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|t| !t.trim().is_empty())
}

/// A track; `album` stands in for the track's own album when the answer does not name one (an album's tracks).
fn track(raw: RawTrack, album: Option<&RawAlbumRef>) -> Track {
    let album = raw.album.as_ref().or(album);
    Track {
        id: raw.id.to_string(),
        title: raw.title,
        artists: artists(Some(raw.artist), raw.contributors),
        album: album.map(|a| AlbumRef {
            id: a.id.to_string(),
            title: a.title.clone(),
        }),
        duration: raw.duration.map(Duration::from_secs),
        isrc: non_empty(raw.isrc),
        artwork: album.map_or_else(Vec::new, |a| {
            pictures([&a.cover_small, &a.cover_medium, &a.cover_big, &a.cover_xl])
        }),
    }
}

/// The year of a release date `YYYY-MM-DD`; Deezer writes an unknown date `0000-00-00`, which gives none.
fn year(release_date: Option<&str>) -> Option<u16> {
    let year = release_date?.get(..4)?;
    if !year.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    year.parse().ok().filter(|&y| y != 0)
}

/// An album; `artist` stands in for its artist when the answer does not name one (an artist's albums).
fn album(raw: RawAlbum, artist: Option<&ArtistRef>) -> Album {
    let mut artists = artists(raw.artist, raw.contributors);
    if artists.is_empty() {
        artists.extend(artist.cloned());
    }
    Album {
        id: raw.id.to_string(),
        title: raw.title,
        artists,
        year: year(raw.release_date.as_deref()),
        artwork: pictures([
            &raw.cover_small,
            &raw.cover_medium,
            &raw.cover_big,
            &raw.cover_xl,
        ]),
    }
}

fn artist(raw: RawArtist) -> Artist {
    Artist {
        id: raw.id.to_string(),
        name: raw.name,
        artwork: pictures([
            &raw.picture_small,
            &raw.picture_medium,
            &raw.picture_big,
            &raw.picture_xl,
        ]),
    }
}

fn playlist(raw: &RawPlaylist) -> Playlist {
    Playlist {
        id: raw.id.to_string(),
        title: raw.title.clone(),
        owner: raw
            .creator
            .as_ref()
            .or(raw.user.as_ref())
            .map(|n| n.name.clone()),
        artwork: pictures([
            &raw.picture_small,
            &raw.picture_medium,
            &raw.picture_big,
            &raw.picture_xl,
        ]),
    }
}

/// The cursor after a page from `index`, in decimal ([`next_index`]).
fn next_cursor(index: u32, count: u32, has_next: bool) -> Option<String> {
    next_index(index, count, has_next).map(|next| next.to_string())
}

/// The index after a page from `index`: the one after the `count` results returned, if Deezer says there is a next
/// page and this one was not empty. A next page past `u32::MAX` is not offered, since its cursor could not be read
/// back.
fn next_index(index: u32, count: u32, has_next: bool) -> Option<u32> {
    if !has_next || count == 0 {
        return None;
    }
    index.checked_add(count)
}

/// The number of results in a list, which fits a `u32` in any answer that fits in memory; a longer one is unreadable.
fn count<T>(list: &[T]) -> Result<u32, Error> {
    u32::try_from(list.len())
        .map_err(|_| Error::Upstream(String::from("Deezer answered too many results")))
}

/// Reads a page of one kind's search endpoint, asked from `index`.
///
/// # Errors
///
/// Those of [`check`], and [`Error::Upstream`] for an answer that is not a list of that kind.
pub fn read_search(kind: ItemKind, index: u32, response: &Response) -> Result<Page, Error> {
    let (items, count, has_next) = match kind {
        ItemKind::Track => {
            let list: List<RawTrack> = parse(response, "track search")?;
            let count = count(&list.data)?;
            let items = list
                .data
                .into_iter()
                .map(|t| Item::Track(track(t, None)))
                .collect();
            (items, count, list.next.is_some())
        }
        ItemKind::Album => {
            let list: List<RawAlbum> = parse(response, "album search")?;
            let count = count(&list.data)?;
            let items = list
                .data
                .into_iter()
                .map(|a| Item::Album(album(a, None)))
                .collect();
            (items, count, list.next.is_some())
        }
        ItemKind::Artist => {
            let list: List<RawArtist> = parse(response, "artist search")?;
            let count = count(&list.data)?;
            let items = list
                .data
                .into_iter()
                .map(|a| Item::Artist(artist(a)))
                .collect();
            (items, count, list.next.is_some())
        }
        ItemKind::Playlist => {
            let list: List<RawPlaylist> = parse(response, "playlist search")?;
            let count = count(&list.data)?;
            let items = list
                .data
                .iter()
                .map(|p| Item::Playlist(playlist(p)))
                .collect();
            (items, count, list.next.is_some())
        }
    };
    Ok(Page {
        items,
        next: next_cursor(index, count, has_next),
    })
}

/// Reads a page of the autocomplete endpoint, asked from `index`, keeping the `kinds` asked for, grouped in their
/// order. The endpoint pages every kind from the same index, [`SEARCH_LIMIT`] at a time, so the next page starts
/// [`SEARCH_LIMIT`] further when Deezer says there is one.
///
/// # Errors
///
/// Those of [`check`], and [`Error::Upstream`] for an answer that is not the autocomplete's object.
pub fn read_autocomplete(
    kinds: &[ItemKind],
    index: u32,
    response: &Response,
) -> Result<Page, Error> {
    let found: Autocomplete = parse(response, "autocomplete")?;
    let returned = [
        found.tracks.data.len(),
        found.albums.data.len(),
        found.artists.data.len(),
        found.playlists.data.len(),
    ]
    .into_iter()
    .sum::<usize>();
    let mut tracks = Some(found.tracks.data);
    let mut albums = Some(found.albums.data);
    let mut artists = Some(found.artists.data);
    let mut playlists = Some(found.playlists.data);
    let mut items = Vec::new();
    for kind in kinds {
        match kind {
            ItemKind::Track => items.extend(
                tracks
                    .take()
                    .into_iter()
                    .flatten()
                    .map(|t| Item::Track(track(t, None))),
            ),
            ItemKind::Album => items.extend(
                albums
                    .take()
                    .into_iter()
                    .flatten()
                    .map(|a| Item::Album(album(a, None))),
            ),
            ItemKind::Artist => items.extend(
                artists
                    .take()
                    .into_iter()
                    .flatten()
                    .map(|a| Item::Artist(artist(a))),
            ),
            ItemKind::Playlist => {
                items.extend(
                    playlists
                        .take()
                        .into_iter()
                        .flatten()
                        .map(|p| Item::Playlist(playlist(&p))),
                );
            }
        }
    }
    let has_next = found.next.is_some() && returned > 0;
    Ok(Page {
        items,
        next: next_cursor(index, SEARCH_LIMIT, has_next),
    })
}

/// The most suggestions returned.
pub const MAX_SUGGESTIONS: usize = 10;

/// Reads the autocomplete's first page as suggestions: the names of the artists, then the titles of the tracks, then
/// the titles of the albums, each once whatever its case, at most [`MAX_SUGGESTIONS`].
///
/// # Errors
///
/// Those of [`check`], and [`Error::Upstream`] for an answer that is not the autocomplete's object.
pub fn read_suggestions(response: &Response) -> Result<Vec<String>, Error> {
    let found: Autocomplete = parse(response, "autocomplete")?;
    let candidates = found
        .artists
        .data
        .into_iter()
        .map(|a| a.name)
        .chain(
            found
                .tracks
                .data
                .into_iter()
                .map(|t| t.title_short.unwrap_or(t.title)),
        )
        .chain(found.albums.data.into_iter().map(|a| a.title));
    let mut suggestions: Vec<String> = Vec::new();
    for candidate in candidates {
        let candidate = candidate.trim();
        if candidate.is_empty()
            || suggestions
                .iter()
                .any(|s| s.to_lowercase() == candidate.to_lowercase())
        {
            continue;
        }
        suggestions.push(candidate.to_owned());
        if suggestions.len() >= MAX_SUGGESTIONS {
            break;
        }
    }
    Ok(suggestions)
}

/// Reads an album page from the answers to [`crate::request::album`]: the album, then its tracks.
///
/// # Errors
///
/// Those of [`check`] for either answer, and [`Error::Upstream`] for an answer that is not an album or a list of
/// tracks, or for an album with more tracks than [`ALBUM_TRACKS_LIMIT`], which would come back cut, whether the
/// album's count, a next page or the list itself says so.
pub fn read_album(
    album_response: &Response,
    tracks_response: &Response,
) -> Result<AlbumPage, Error> {
    let raw: RawAlbum = parse(album_response, "album")?;
    let list: List<RawTrack> = parse(tracks_response, "album's tracks")?;
    let too_many = |n: u64| n > u64::from(ALBUM_TRACKS_LIMIT);
    if list.next.is_some()
        || raw.nb_tracks.is_some_and(too_many)
        || u64::try_from(list.data.len()).map_or(true, too_many)
    {
        return Err(Error::Upstream(format!(
            "the album has more than {ALBUM_TRACKS_LIMIT} tracks, more than one answer holds"
        )));
    }
    let album_ref = RawAlbumRef {
        id: raw.id,
        title: raw.title.clone(),
        cover_small: raw.cover_small.clone(),
        cover_medium: raw.cover_medium.clone(),
        cover_big: raw.cover_big.clone(),
        cover_xl: raw.cover_xl.clone(),
    };
    let tracks = list
        .data
        .into_iter()
        .map(|t| track(t, Some(&album_ref)))
        .collect();
    Ok(AlbumPage {
        album: album(raw, None),
        tracks,
    })
}

/// Reads an artist page from the answers to [`crate::request::artist`]: the artist, their top tracks, their albums.
///
/// # Errors
///
/// Those of [`check`] for any answer, and [`Error::Upstream`] for an answer that is not what it should be.
pub fn read_artist(
    artist_response: &Response,
    top_response: &Response,
    albums_response: &Response,
) -> Result<ArtistPage, Error> {
    let raw: RawArtist = parse(artist_response, "artist")?;
    let top: List<RawTrack> = parse(top_response, "artist's top tracks")?;
    let albums: List<RawAlbum> = parse(albums_response, "artist's albums")?;
    let artist = artist(raw);
    let artist_ref = ArtistRef {
        id: artist.id.clone(),
        name: artist.name.clone(),
    };
    Ok(ArtistPage {
        top_tracks: top.data.into_iter().map(|t| track(t, None)).collect(),
        albums: albums
            .data
            .into_iter()
            .map(|a| album(a, Some(&artist_ref)))
            .collect(),
        artist,
    })
}

/// Reads a playlist page from the answer to [`crate::request::playlist`], asked from `index`. The next page starts
/// after the tracks returned, while the playlist has more.
///
/// # Errors
///
/// Those of [`check`], and [`Error::Upstream`] for an answer that is not a playlist with its tracks and its track
/// count, without which the plugin cannot tell whether a page follows and would cut the playlist short.
pub fn read_playlist(index: u32, response: &Response) -> Result<PlaylistPage, Error> {
    let raw: RawPlaylist = parse(response, "playlist")?;
    let header = playlist(&raw);
    let tracks = raw
        .tracks
        .ok_or_else(|| Error::Upstream(String::from("Deezer's playlist has no tracks field")))?
        .data;
    let total = raw
        .nb_tracks
        .ok_or_else(|| Error::Upstream(String::from("Deezer's playlist has no track count")))?;
    let count = count(&tracks)?;
    let has_next = u64::from(index).saturating_add(u64::from(count)) < total;
    Ok(PlaylistPage {
        playlist: header,
        tracks: tracks.into_iter().map(|t| track(t, None)).collect(),
        next: next_cursor(index, count, has_next),
    })
}

/// Reads one item of `kind`, the answer to [`crate::request::item`] or [`crate::request::track_by_isrc`].
///
/// # Errors
///
/// Those of [`check`], and [`Error::Upstream`] for an answer that is not an item of that kind.
pub fn read_item(kind: ItemKind, response: &Response) -> Result<Item, Error> {
    Ok(match kind {
        ItemKind::Track => Item::Track(track(parse(response, "track")?, None)),
        ItemKind::Album => Item::Album(album(parse(response, "album")?, None)),
        ItemKind::Artist => Item::Artist(artist(parse(response, "artist")?)),
        ItemKind::Playlist => Item::Playlist(playlist(&parse(response, "playlist")?)),
    })
}

/// What an artwork search looks for: an item of another provider, by its names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Wanted {
    /// The track's or album's title, or the artist's name.
    pub title: String,
    /// The names of the item's artists; empty for an artist.
    pub artists: Vec<String>,
}

/// Whether two names are the same once trimmed and lowercased.
fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// Reads an artwork search: the artwork of the first result whose title (or a track's short title, without its
/// version) is `wanted`'s and that names one of `wanted`'s artists. An artist matches on its name alone.
///
/// # Errors
///
/// Those of [`check`]; [`Error::NotFound`] when no result matches or the match has no artwork; [`Error::Upstream`]
/// for an answer that is not a list of that kind.
pub fn read_artwork_search(
    kind: ItemKind,
    wanted: &Wanted,
    response: &Response,
) -> Result<Vec<Artwork>, Error> {
    let names_one = |names: &[ArtistRef]| {
        names
            .iter()
            .any(|n| wanted.artists.iter().any(|w| same(&n.name, w)))
    };
    let artwork = match kind {
        ItemKind::Track => {
            let list: List<RawTrack> = parse(response, "track search")?;
            list.data
                .into_iter()
                .filter(|t| {
                    same(&t.title, &wanted.title)
                        || t.title_short
                            .as_deref()
                            .is_some_and(|s| same(s, &wanted.title))
                })
                .map(|t| track(t, None))
                .find(|t| names_one(&t.artists))
                .map(|t| t.artwork)
        }
        ItemKind::Album => {
            let list: List<RawAlbum> = parse(response, "album search")?;
            list.data
                .into_iter()
                .filter(|a| same(&a.title, &wanted.title))
                .map(|a| album(a, None))
                .find(|a| names_one(&a.artists))
                .map(|a| a.artwork)
        }
        ItemKind::Artist => {
            let list: List<RawArtist> = parse(response, "artist search")?;
            list.data
                .into_iter()
                .find(|a| same(&a.name, &wanted.title))
                .map(|a| artist(a).artwork)
        }
        ItemKind::Playlist => return Err(Error::Unsupported),
    };
    artwork.filter(|a| !a.is_empty()).ok_or(Error::NotFound)
}

/// Reads the track found by its ISRC: its album's covers.
///
/// # Errors
///
/// Those of [`check`] ([`Error::NotFound`] when Deezer has no track with that ISRC); [`Error::NotFound`] when the track
/// has no cover; [`Error::Upstream`] for an answer that is not a track.
pub fn read_artwork_by_isrc(response: &Response) -> Result<Vec<Artwork>, Error> {
    let raw: RawTrack = parse(response, "track")?;
    let artwork = track(raw, None).artwork;
    if artwork.is_empty() {
        return Err(Error::NotFound);
    }
    Ok(artwork)
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod proofs;
