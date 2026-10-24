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
        "vehicoule:music/types@1.1.0": vehicoule_sdk::types,
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
/// iTunes' cap on a search's `limit`: no result past the 200th can ever be reached.
const SEARCH_CAP: u32 = 200;

struct Itunes;

fn get(url: String) -> Result<Vec<u8>, Error> {
    let req = Request {
        method: Method::Get,
        url,
        headers: vec![],
        body: None,
    };
    let resp = host_http::send(&req).map_err(vehicoule_sdk::from_http_error)?;
    match api::status_error(resp.status) {
        Some(e) => Err(e),
        None => Ok(resp.body),
    }
}

fn search_url(term: &str, entity: &str, country: &str, limit: &str) -> String {
    format!(
        "{API}/search?{}",
        query(&[
            ("term", term),
            ("entity", entity),
            ("country", country),
            ("limit", limit)
        ])
    )
}

fn lookup_url(id: &str, entity: &str, country: &str, limit: Option<&str>) -> String {
    let mut pairs = vec![("id", id), ("entity", entity), ("country", country)];
    if let Some(limit) = limit {
        pairs.push(("limit", limit));
    }
    format!("{API}/lookup?{}", query(&pairs))
}

/// `0` without a cursor (the first page). Parsed to `u32` once here, since both the next-page
/// math and the URL need a number: a cursor that isn't a plain decimal is refused outright,
/// one that's all digits but past `u32::MAX` is treated as a page that doesn't exist.
fn offset(cursor: Option<&str>) -> Result<u32, Error> {
    match cursor {
        None => Ok(0),
        Some(c) if vehicoule_sdk::is_decimal_id(c) => c.parse().map_err(|_| Error::NotFound),
        Some(c) => Err(Error::Internal(format!("bad cursor {c:?}"))),
    }
}

impl search::Guest for Itunes {
    /// One request per kind requested (song/album/musicArtist), run together; iTunes has no
    /// playlists. iTunes ignores `offset`, so a page asks for the first `offset + limit`
    /// results (up to `SEARCH_CAP`) and drops the first `offset`: a kind that came back full
    /// below the cap means more may follow.
    fn search(req: search::SearchRequest) -> Result<search::SearchResults, Error> {
        use EntityKind::*;
        let kinds: Vec<EntityKind> = if req.kinds.is_empty() {
            vec![Track, Album, Artist]
        } else {
            req.kinds.into_iter().filter(|k| *k != Playlist).collect()
        };
        let mut out = search::SearchResults {
            tracks: vec![],
            albums: vec![],
            artists: vec![],
            playlists: vec![],
            next: None,
        };
        if kinds.is_empty() {
            return Ok(out); // Only playlists were asked for; iTunes has none.
        }
        let entity = |k: &EntityKind| match k {
            Track => "song",
            Album => "album",
            Artist => "musicArtist",
            Playlist => unreachable!("filtered out above"),
        };
        let offset = offset(req.cursor.as_deref())?;
        let end = offset.saturating_add(req.limit).min(SEARCH_CAP);
        let limit = end.to_string();
        let reqs: Vec<Request> = kinds
            .iter()
            .map(|k| Request {
                method: Method::Get,
                url: search_url(&req.query, entity(k), &req.region, &limit),
                headers: vec![],
                body: None,
            })
            .collect();
        let bodies: Vec<Vec<u8>> = vehicoule_sdk::send_batch_bodies(&reqs, api::status_error)
            .into_iter()
            .collect::<Result<_, Error>>()?;
        let mut full_page = false;
        for body in &bodies {
            let parts = api::parse_results(body)?;
            let skip = offset as usize;
            out.tracks.extend(parts.tracks.into_iter().skip(skip));
            out.albums.extend(parts.albums.into_iter().skip(skip));
            out.artists.extend(parts.artists.into_iter().skip(skip));
            full_page |= parts.total == end;
        }
        if full_page && end < SEARCH_CAP {
            out.next = Some(end.to_string());
        }
        Ok(out)
    }
}

impl entity::Guest for Itunes {
    /// The region is iTunes' own `country`, taken as given (ISO 3166-1 alpha-2, uppercase).
    /// `lookup` has no cursor to page through more, so a page is always the whole thing (up
    /// to `LOOKUP_LIMIT`) and `next` is always `none`.
    fn fetch(
        target: EntityRef,
        region: String,
        _cursor: Option<String>,
    ) -> Result<EntityPage, Error> {
        if !vehicoule_sdk::is_decimal_id(&target.id) {
            return Err(Error::NotFound);
        }
        let page = |entity, tracks, albums| EntityPage {
            entity,
            tracks,
            albums,
            next: None,
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dbg<T: std::fmt::Debug>(v: T) -> String {
        format!("{v:?}")
    }

    #[test]
    fn offset_parses_once_and_bounds_overflow() {
        assert_eq!(dbg(offset(None)), dbg(Ok::<u32, Error>(0)));
        assert_eq!(dbg(offset(Some("5"))), dbg(Ok::<u32, Error>(5)));
        assert_eq!(
            dbg(offset(Some("4294967295"))),
            dbg(Ok::<u32, Error>(u32::MAX))
        );
        // Not a plain decimal: refused rather than sent on.
        assert!(matches!(offset(Some("-1")), Err(Error::Internal(_))));
        assert!(matches!(offset(Some("abc")), Err(Error::Internal(_))));
        // All digits but past `u32::MAX`: no such page, not an internal error.
        assert_eq!(
            dbg(offset(Some("4294967296"))),
            dbg(Err::<u32, _>(Error::NotFound))
        );
        assert_eq!(
            dbg(offset(Some("99999999999999999999"))),
            dbg(Err::<u32, _>(Error::NotFound))
        );
    }
}
