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
        "vehicoule:music/types@1.0.0": vehicoule_sdk::types,
    },
});

mod api;
mod links;

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

/// GETs every URL in one host batch; the bodies in order, or the first error. Deezer's own
/// JSON errors are left to `api`.
fn get_all(urls: Vec<String>) -> Result<Vec<Vec<u8>>, Error> {
    let reqs: Vec<Request> = urls.into_iter().map(get_request).collect();
    host_http::send_batch(&reqs)
        .into_iter()
        .map(|r| {
            let resp = r.map_err(from_http_error)?;
            status_error(resp.status).map_or(Ok(resp.body), Err)
        })
        .collect()
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
                let b = get_all(vec![
                    format!("{API}/album/{id}"),
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
                let b = get_all(vec![
                    format!("{API}/playlist/{id}"),
                    format!("{API}/playlist/{id}/tracks?index={index}&limit={TRACKS_PAGE}"),
                ])?;
                let playlist = api::parse_playlist(&b[0])?;
                let (tracks, next) = api::parse_tracks(&b[1])?;
                Ok(page(Entity::Playlist(playlist), tracks, vec![], next))
            }
        }
    }

    /// A share link is followed (the host does the redirects) and its final URL parsed; one
    /// that leads nowhere (`/deezer-links-404`) is `not-found`.
    fn parse_link(url: String) -> Result<Option<EntityRef>, Error> {
        if let Some(target) = links::parse_link(&url) {
            return Ok(Some(target));
        }
        if !links::is_short_link(&url) {
            return Ok(None);
        }
        let url = match url.strip_prefix("http://") {
            Some(rest) => format!("https://{rest}"),
            None => url,
        };
        let resp = host_http::send(&get_request(url)).map_err(from_http_error)?;
        links::parse_link(&resp.url).map(Some).ok_or(Error::NotFound)
    }
}

impl radio::Guest for Deezer {
    /// The radio of the seed's artist: the seed by its id, else the first search match.
    fn radio(seed: TrackQuery, _region: String, limit: u32) -> Result<Vec<Track>, Error> {
        let track = match seed.id.filter(|id| links::is_id(id)) {
            Some(id) => api::parse_track(&get(format!("{API}/track/{id}"))?)?,
            None => {
                let q = format!("{} {}", seed.artists.first().map_or("", String::as_str), seed.title);
                let found = api::parse_tracks(&get(search_url("track", q.trim(), "1", "0"))?)?.0;
                found.into_iter().next().ok_or(Error::NotFound)?
            }
        };
        let artist = track.artists.first().and_then(|a| a.id.clone()).ok_or(Error::NotFound)?;
        let (mut tracks, _) = api::parse_tracks(&get(format!("{API}/artist/{artist}/radio?limit={limit}"))?)?;
        tracks.truncate(limit as usize);
        Ok(tracks)
    }
}

export!(Deezer);
