//! iTunes as a Vehicoule plugin: the free Search/Lookup API (`itunes.apple.com`, no account)
//! behind the `vehicoule:music` contract, over the host's `http`. A fallback catalogue and
//! artwork source — no playlists, no radio.

wit_bindgen::generate!({
    path: ["../../wit/host", "../../wit/music", "wit"],
    world: "vehicoule:itunes/itunes",
    with: {
        "vehicoule:host/http@1.0.0": vehicoule_sdk::host_http,
        "vehicoule:host/kv@1.0.0": vehicoule_sdk::kv,
        "vehicoule:host/clock@1.0.0": vehicoule_sdk::clock,
        "vehicoule:host/log@1.0.0": vehicoule_sdk::log,
        "vehicoule:music/types@1.0.0": vehicoule_sdk::types,
    },
});

mod api;

use exports::vehicoule::music::entity::{self, Entity, EntityPage};
use exports::vehicoule::music::search;
use vehicoule_sdk::host_http::{self, Method, Request};
use vehicoule_sdk::query;
use vehicoule_sdk::types::{EntityKind, EntityRef, Error};

const API: &str = "https://itunes.apple.com";
/// iTunes' own cap for `lookup`: enough to get a whole album or an artist's discography in
/// one request, since `lookup` has no `offset` to page through more.
const LOOKUP_LIMIT: &str = "200";

struct Itunes;

fn get(url: String) -> Result<Vec<u8>, Error> {
    let req = Request { method: Method::Get, url, headers: vec![], body: None };
    let resp = host_http::send(&req).map_err(vehicoule_sdk::from_http_error)?;
    match api::status_error(resp.status) {
        Some(e) => Err(e),
        None => Ok(resp.body),
    }
}

fn search_url(term: &str, entity: &str, country: &str, limit: &str, offset: &str) -> String {
    format!("{API}/search?{}", query(&[("term", term), ("entity", entity), ("country", country), ("limit", limit), ("offset", offset)]))
}

fn lookup_url(id: &str, entity: &str, country: &str, limit: Option<&str>) -> String {
    let mut pairs = vec![("id", id), ("entity", entity), ("country", country)];
    if let Some(limit) = limit {
        pairs.push(("limit", limit));
    }
    format!("{API}/lookup?{}", query(&pairs))
}

/// `None` without a cursor (the first page); a non-digit cursor is refused rather than sent on.
fn offset(cursor: Option<&str>) -> Result<&str, Error> {
    match cursor {
        None => Ok("0"),
        Some(c) if api::is_id(c) => Ok(c),
        Some(c) => Err(Error::Internal(format!("bad cursor {c:?}"))),
    }
}

impl search::Guest for Itunes {
    /// One request per kind requested (song/album/musicArtist), run together; iTunes has no
    /// playlists. Every kind pages by the same `limit` from the same `offset`: iTunes' own
    /// `resultCount` tells a full page (more may follow) from the last one.
    fn search(req: search::SearchRequest) -> Result<search::SearchResults, Error> {
        use EntityKind::*;
        let kinds: Vec<EntityKind> =
            if req.kinds.is_empty() { vec![Track, Album, Artist] } else { req.kinds.into_iter().filter(|k| *k != Playlist).collect() };
        let mut out = search::SearchResults { tracks: vec![], albums: vec![], artists: vec![], playlists: vec![], next: None };
        if kinds.is_empty() {
            return Ok(out); // Only playlists were asked for; iTunes has none.
        }
        let entity = |k: &EntityKind| match k {
            Track => "song",
            Album => "album",
            Artist => "musicArtist",
            Playlist => unreachable!("filtered out above"),
        };
        let (limit, offset) = (req.limit.to_string(), offset(req.cursor.as_deref())?);
        let reqs: Vec<Request> = kinds
            .iter()
            .map(|k| Request { method: Method::Get, url: search_url(&req.query, entity(k), &req.region, &limit, offset), headers: vec![], body: None })
            .collect();
        let bodies: Vec<Vec<u8>> = host_http::send_batch(&reqs)
            .into_iter()
            .map(|r| {
                let resp = r.map_err(vehicoule_sdk::from_http_error)?;
                api::status_error(resp.status).map_or(Ok(resp.body), Err)
            })
            .collect::<Result<_, Error>>()?;
        let mut full_page = false;
        for body in &bodies {
            let parts = api::parse_results(body)?;
            out.tracks.extend(parts.tracks);
            out.albums.extend(parts.albums);
            out.artists.extend(parts.artists);
            full_page |= parts.total == req.limit;
        }
        if full_page {
            let next = offset.parse::<u32>().unwrap_or(0).saturating_add(req.limit);
            out.next = Some(next.to_string());
        }
        Ok(out)
    }
}

impl entity::Guest for Itunes {
    /// The region is iTunes' own `country`, taken as given (ISO 3166-1 alpha-2, uppercase).
    /// `lookup` has no cursor to page through more, so a page is always the whole thing (up
    /// to `LOOKUP_LIMIT`) and `next` is always `none`.
    fn fetch(target: EntityRef, region: String, _cursor: Option<String>) -> Result<EntityPage, Error> {
        if !api::is_id(&target.id) {
            return Err(Error::NotFound);
        }
        let page = |entity, tracks, albums| EntityPage { entity, tracks, albums, next: None };
        match target.kind {
            EntityKind::Track => {
                let track = api::parse_track(&get(lookup_url(&target.id, "song", &region, None))?)?;
                Ok(page(Entity::Track(track), vec![], vec![]))
            }
            EntityKind::Album => {
                let body = get(lookup_url(&target.id, "song", &region, Some(LOOKUP_LIMIT)))?;
                let (album, tracks) = api::parse_album_tracks(&body)?;
                Ok(page(Entity::Album(album), tracks, vec![]))
            }
            EntityKind::Artist => {
                let body = get(lookup_url(&target.id, "album", &region, Some(LOOKUP_LIMIT)))?;
                let (artist, albums) = api::parse_artist_albums(&body)?;
                Ok(page(Entity::Artist(artist), vec![], albums))
            }
            // Not served by this API.
            EntityKind::Playlist => Err(Error::NotFound),
        }
    }

    fn parse_link(url: String) -> Result<Option<EntityRef>, Error> {
        Ok(api::parse_link(&url))
    }
}

export!(Itunes);
