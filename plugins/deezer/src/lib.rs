//! Deezer as a Vehicoule plugin: its public API (`api.deezer.com`, no account) behind the
//! `vehicoule:music` contract, over the host's `http`.

wit_bindgen::generate!({
    path: ["../../wit/host", "../../wit/music", "wit"],
    world: "vehicoule:deezer/deezer",
    with: {
        "vehicoule:host/http@1.0.0": vehicoule_sdk::host_http,
        "vehicoule:host/kv@1.0.0": vehicoule_sdk::kv,
        "vehicoule:host/clock@1.0.0": vehicoule_sdk::clock,
        "vehicoule:host/log@1.0.0": vehicoule_sdk::log,
        "vehicoule:music/types@1.1.0": vehicoule_sdk::types,
    },
});

mod api;
mod links;

use api::CategoryId;

use exports::vehicoule::music::browse::{self, Category, CategoryPage, Release};
use exports::vehicoule::music::entity::{self, Entity, EntityPage};
use exports::vehicoule::music::{radio, search};
use vehicoule_sdk::host_http::{self, Method, Request};
use vehicoule_sdk::types::{AlbumRef, ArtistRef, EntityKind, EntityRef, Error, Track, TrackQuery};
use vehicoule_sdk::{from_http_error, query, status_error};

const API: &str = "https://api.deezer.com";
/// Tracks per page of an album or a playlist.
const TRACKS_PAGE: u32 = 100;
/// An artist's top tracks, on the first page only.
const TOP_TRACKS: u32 = 20;
/// An artist's albums per page.
const ALBUMS_PAGE: u32 = 50;

struct Deezer;

fn get_request(url: String) -> Request {
    Request { method: Method::Get, url, headers: vec![], body: None }
}

/// Past the host's HTTP cache, both ways (no lookup, no store).
fn uncached(mut req: Request) -> Request {
    req.headers.push(("cache-control".into(), "no-cache".into()));
    req
}

/// Sends every request in one host batch; the bodies in order, or the first error. Deezer's
/// own JSON errors are left to `api`, except its quota and "busy" ones: they come as HTTP 200,
/// which the host caches for a day, so each such request is sent once more past the cache.
fn send_all(reqs: Vec<Request>) -> Result<Vec<Vec<u8>>, Error> {
    let send = |reqs: &[Request]| -> Result<Vec<Vec<u8>>, Error> {
        host_http::send_batch(reqs)
            .into_iter()
            .map(|r| {
                let resp = r.map_err(from_http_error)?;
                status_error(resp.status).map_or(Ok(resp.body), Err)
            })
            .collect()
    };
    let mut bodies = send(&reqs)?;
    let limited: Vec<usize> = (0..bodies.len()).filter(|&i| matches!(api::deezer_error(&bodies[i]), Some(Error::RateLimited))).collect();
    if !limited.is_empty() {
        let retry: Vec<Request> = limited.iter().map(|&i| uncached(reqs[i].clone())).collect();
        for (i, body) in limited.into_iter().zip(send(&retry)?) {
            bodies[i] = body;
        }
    }
    Ok(bodies)
}

/// GETs every URL in one host batch (see `send_all`).
fn get_all(urls: Vec<String>) -> Result<Vec<Vec<u8>>, Error> {
    send_all(urls.into_iter().map(get_request).collect())
}

fn get(url: String) -> Result<Vec<u8>, Error> {
    Ok(get_all(vec![url])?.remove(0))
}

/// Deezer's `index`: 0 without a cursor.
fn index(cursor: Option<&str>) -> Result<&str, Error> {
    match cursor {
        None => Ok("0"),
        Some(c) if links::is_id(c) => Ok(c),
        Some(c) => Err(Error::Internal(format!("bad cursor {c:?}"))),
    }
}

fn search_url(kind: &str, q: &str, limit: &str, index: &str) -> String {
    format!("{API}/search/{kind}?{}", query(&[("q", q), ("limit", limit), ("index", index)]))
}

impl search::Guest for Deezer {
    /// One request per kind, sent together. Every kind pages by the same `limit` from the same
    /// `index`, so their next index is shared.
    fn search(req: search::SearchRequest) -> Result<search::SearchResults, Error> {
        use EntityKind::*;
        let kinds = if req.kinds.is_empty() { vec![Track, Album, Artist, Playlist] } else { req.kinds };
        let (limit, index) = (req.limit.to_string(), index(req.cursor.as_deref())?);
        let path = |k: &EntityKind| match k {
            Track => "track",
            Album => "album",
            Artist => "artist",
            Playlist => "playlist",
        };
        let bodies = get_all(kinds.iter().map(|k| search_url(path(k), &req.query, &limit, index)).collect())?;
        let mut out = search::SearchResults { tracks: vec![], albums: vec![], artists: vec![], playlists: vec![], next: None };
        for (kind, body) in kinds.iter().zip(&bodies) {
            let next = match kind {
                Track => {
                    let (v, next) = api::parse_tracks(body)?;
                    out.tracks = v;
                    next
                }
                Album => {
                    let (v, next) = api::parse_albums(body)?;
                    out.albums = v;
                    next
                }
                Artist => {
                    let (v, next) = api::parse_artists(body)?;
                    out.artists = v;
                    next
                }
                Playlist => {
                    let (v, next) = api::parse_playlists(body)?;
                    out.playlists = v;
                    next
                }
            };
            out.next = out.next.or(next);
        }
        Ok(out)
    }
}

impl entity::Guest for Deezer {
    /// The region is not sent: Deezer locates the caller by IP.
    fn fetch(target: EntityRef, _region: String, cursor: Option<String>) -> Result<EntityPage, Error> {
        if !links::is_id(&target.id) {
            return Err(Error::NotFound);
        }
        let (id, index) = (&target.id, index(cursor.as_deref())?);
        let page = |entity, tracks, albums, next| EntityPage { entity, tracks, albums, next };
        match target.kind {
            EntityKind::Track => {
                let track = api::parse_track(&get(format!("{API}/track/{id}"))?)?;
                Ok(page(Entity::Track(track), vec![], vec![], None))
            }
            EntityKind::Album => {
                // `limit=1`: the album without its embedded track list, which is paged below.
                let b = get_all(vec![
                    format!("{API}/album/{id}?limit=1"),
                    format!("{API}/album/{id}/tracks?index={index}&limit={TRACKS_PAGE}"),
                ])?;
                let album = api::parse_album(&b[0])?;
                let (mut tracks, next) = api::parse_tracks(&b[1])?;
                // An album's own track list names neither the album nor its cover.
                for t in &mut tracks {
                    t.album.get_or_insert_with(|| AlbumRef { id: Some(album.id.clone()), title: album.title.clone() });
                    if t.artwork.is_empty() {
                        t.artwork = album.artwork.clone();
                    }
                }
                Ok(page(Entity::Album(album), tracks, vec![], next))
            }
            EntityKind::Artist => {
                let mut urls = vec![
                    format!("{API}/artist/{id}"),
                    format!("{API}/artist/{id}/albums?index={index}&limit={ALBUMS_PAGE}"),
                ];
                if cursor.is_none() {
                    urls.push(format!("{API}/artist/{id}/top?limit={TOP_TRACKS}"));
                }
                let b = get_all(urls)?;
                let artist = api::parse_artist(&b[0])?;
                let (mut albums, next) = api::parse_albums(&b[1])?;
                // An artist's album list does not repeat the artist.
                for a in albums.iter_mut().filter(|a| a.artists.is_empty()) {
                    a.artists.push(ArtistRef { id: Some(artist.id.clone()), name: artist.name.clone() });
                }
                let tracks = match b.get(2) {
                    Some(top) => api::parse_tracks(top)?.0,
                    None => vec![],
                };
                Ok(page(Entity::Artist(artist), tracks, albums, next))
            }
            EntityKind::Playlist => {
                // `limit=1`: the playlist without its embedded tracks (up to 400), paged below.
                let b = get_all(vec![
                    format!("{API}/playlist/{id}?limit=1"),
                    format!("{API}/playlist/{id}/tracks?index={index}&limit={TRACKS_PAGE}"),
                ])?;
                let playlist = api::parse_playlist(&b[0])?;
                let (tracks, next) = api::parse_tracks(&b[1])?;
                Ok(page(Entity::Playlist(playlist), tracks, vec![], next))
            }
        }
    }

    /// A share link is followed (the host does the redirects) and its final URL parsed. When
    /// that URL names nothing, a refused request keeps its typed error (429 → `rate-limited`)
    /// and a dead link (404 at `/deezer-links-404`) is `not-found`.
    fn parse_link(url: String) -> Result<Option<EntityRef>, Error> {
        if let Some(target) = links::parse_link(&url) {
            return Ok(Some(target));
        }
        if !links::is_short_link(&url) {
            return Ok(None);
        }
        let url = match url.get(..7) {
            Some(scheme) if scheme.eq_ignore_ascii_case("http://") => format!("https://{}", &url[7..]),
            _ => url,
        };
        let resp = host_http::send(&get_request(url)).map_err(from_http_error)?;
        match links::parse_link(&resp.url) {
            Some(target) => Ok(Some(target)),
            None => Err(status_error(resp.status).unwrap_or(Error::NotFound)),
        }
    }
}

impl radio::Guest for Deezer {
    /// The radio of the seed's artist: the seed by its id, else (no id, or one Deezer no
    /// longer knows) the first search match.
    fn radio(seed: TrackQuery, _region: String, limit: u32) -> Result<Vec<Track>, Error> {
        let by_id = match seed.id.as_deref().filter(|id| links::is_id(id)) {
            Some(id) => match get(format!("{API}/track/{id}")).and_then(|body| api::parse_track(&body)) {
                Err(Error::NotFound) => None,
                found => Some(found?),
            },
            None => None,
        };
        let track = match by_id {
            Some(track) => track,
            None => {
                let q = format!("{} {}", seed.artists.first().map_or("", String::as_str), seed.title);
                if q.trim().is_empty() {
                    return Err(Error::NotFound);
                }
                let found = api::parse_tracks(&get(search_url("track", q.trim(), "1", "0"))?)?.0;
                found.into_iter().next().ok_or(Error::NotFound)?
            }
        };
        let artist = track.artists.first().and_then(|a| a.id.clone()).ok_or(Error::NotFound)?;
        // A different radio on every call: never from the host's cache.
        let radio = uncached(get_request(format!("{API}/artist/{artist}/radio?limit={limit}")));
        let (mut tracks, _) = api::parse_tracks(&send_all(vec![radio])?.remove(0))?;
        tracks.truncate(limit as usize);
        Ok(tracks)
    }
}

/// The featured cards' titles: Deezer's own names, not interface text.
const TOP: &str = "Top 50";
const SELECTION: &str = "Sélection";

fn chart_url(genre: &str) -> String {
    format!("{API}/chart/{genre}?limit=50")
}

/// Deezer's charts follow the caller's IP: the region is not sent. A chart fits one page.
impl browse::Guest for Deezer {
    /// The chart and the editors' selection, each with the cover of its first item, then the
    /// genres.
    fn categories(_region: String) -> Result<Vec<Category>, Error> {
        let b = get_all(vec![
            format!("{API}/genre"),
            format!("{API}/chart/0/tracks?limit=1"),
            format!("{API}/editorial/0/selection"),
        ])?;
        let top = api::parse_tracks(&b[1])?
            .0
            .into_iter()
            .next()
            .map(|t| t.artwork)
            .unwrap_or_default();
        let pick = api::parse_albums(&b[2])?
            .0
            .into_iter()
            .next()
            .map(|a| a.artwork)
            .unwrap_or_default();
        let featured = |id: &str, title: &str, artwork| Category {
            id: id.into(),
            title: title.into(),
            artwork,
            featured: true,
        };
        let mut out = vec![
            featured("chart:0", TOP, top),
            featured("selection:0", SELECTION, pick),
        ];
        out.extend(api::parse_genres(&b[0])?);
        Ok(out)
    }

    fn fetch_category(
        id: String,
        _region: String,
        _cursor: Option<String>,
    ) -> Result<CategoryPage, Error> {
        let (title, shelves) = match api::category_id(&id)? {
            CategoryId::Chart(g) => (TOP.into(), api::parse_chart(&get(chart_url(g))?)?),
            // The genre's name from the list `categories` fetched (likely in the host's cache).
            CategoryId::Genre(g) => {
                let b = get_all(vec![format!("{API}/genre"), chart_url(g)])?;
                let genre = api::parse_genres(&b[0])?
                    .into_iter()
                    .find(|c| c.id == id)
                    .ok_or(Error::NotFound)?;
                (genre.title, api::parse_chart(&b[1])?)
            }
            CategoryId::Selection(g) => (
                SELECTION.into(),
                api::parse_selection(&get(format!("{API}/editorial/{g}/selection"))?)?,
            ),
        };
        Ok(CategoryPage {
            title,
            shelves,
            next: None,
        })
    }

    fn releases(artist_id: String, _region: String) -> Result<Vec<Release>, Error> {
        if !links::is_id(&artist_id) {
            return Err(Error::NotFound);
        }
        api::parse_releases(&get(format!(
            "{API}/artist/{artist_id}/albums?limit={ALBUMS_PAGE}"
        ))?)
    }
}

export!(Deezer);
