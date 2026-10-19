//! LRCLIB (`lrclib.net`) request building and JSON parsing. `GET /api/get` is an exact match
//! on track/artist/album/duration; a 404 there falls back to `GET /api/search` (track/artist
//! only), picking the first result within 2s of the query's duration (or just the first, when
//! the query carries no duration).

use serde::Deserialize;
use vehicoule_sdk::host_http::{self, Method, Request};
use vehicoule_sdk::query;
use vehicoule_sdk::types::{Error, TrackQuery};

use crate::exports::vehicoule::music::lyrics::{Line, Lyrics};
use crate::lrc::parse_lrc;

const API: &str = "https://lrclib.net";
/// A search hit counts as matching the query's duration within this many seconds either way.
const DURATION_TOLERANCE_S: i64 = 2;

/// The fields read off an LRCLIB track record, shared by `/api/get` and each `/api/search`
/// result.
#[derive(Deserialize)]
struct LrcTrack {
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    instrumental: bool,
    #[serde(rename = "plainLyrics", default)]
    plain_lyrics: Option<String>,
    #[serde(rename = "syncedLyrics", default)]
    synced_lyrics: Option<String>,
}

pub fn fetch(track: &TrackQuery) -> Result<Lyrics, Error> {
    let req = Request { method: Method::Get, url: get_url(track), headers: vec![], body: None };
    let resp = host_http::send(&req).map_err(vehicoule_sdk::from_http_error)?;
    match resp.status {
        200..=299 => to_lyrics(parse_track(&resp.body)?),
        404 => search_fallback(track),
        status => Err(vehicoule_sdk::status_error(status).unwrap_or(Error::Internal(format!("HTTP {status}")))),
    }
}

/// `GET /api/search`: picks the first result within `DURATION_TOLERANCE_S` of the query's
/// duration, or the first result outright when the query has none. No match (or an empty
/// result list) is `not-found`, same as a direct miss.
fn search_fallback(track: &TrackQuery) -> Result<Lyrics, Error> {
    let req = Request { method: Method::Get, url: search_url(track), headers: vec![], body: None };
    let resp = host_http::send(&req).map_err(vehicoule_sdk::from_http_error)?;
    if let Some(e) = vehicoule_sdk::status_error(resp.status) {
        return Err(e);
    }
    let results: Vec<LrcTrack> = serde_json::from_slice(&resp.body).map_err(|e| Error::Internal(e.to_string()))?;
    let wanted_s = track.duration_ms.map(|ms| (ms as f64 / 1000.0).round() as i64);
    let picked = match wanted_s {
        None => results.into_iter().next(),
        Some(wanted) => results.into_iter().find(|r| r.duration.is_some_and(|d| (d.round() as i64 - wanted).abs() <= DURATION_TOLERANCE_S)),
    };
    to_lyrics(picked.ok_or(Error::NotFound)?)
}

/// `instrumental: true`, or neither plain nor synced text, is `not-found`: nothing to show.
fn to_lyrics(t: LrcTrack) -> Result<Lyrics, Error> {
    if t.instrumental {
        return Err(Error::NotFound);
    }
    let synced: Vec<Line> = t
        .synced_lyrics
        .as_deref()
        .map(parse_lrc)
        .unwrap_or_default()
        .into_iter()
        .map(|(start_ms, text)| Line { start_ms, text })
        .collect();
    let plain = t.plain_lyrics.filter(|s| !s.is_empty());
    if plain.is_none() && synced.is_empty() {
        return Err(Error::NotFound);
    }
    Ok(Lyrics { plain, synced })
}

fn parse_track(body: &[u8]) -> Result<LrcTrack, Error> {
    serde_json::from_slice(body).map_err(|e| Error::Internal(e.to_string()))
}

/// `track_name`, `artist_name` (the query's first artist), `album_name` (omitted when the
/// query has none), `duration` in whole seconds (omitted when the query has none).
fn get_url(track: &TrackQuery) -> String {
    let artist = track.artists.first().map(String::as_str).unwrap_or("");
    let duration = track.duration_ms.map(|ms| ((ms as f64 / 1000.0).round() as u32).to_string());
    let mut pairs: Vec<(&str, &str)> = vec![("track_name", &track.title), ("artist_name", artist)];
    if let Some(album) = &track.album {
        pairs.push(("album_name", album));
    }
    if let Some(d) = &duration {
        pairs.push(("duration", d));
    }
    format!("{API}/api/get?{}", query(&pairs))
}

/// `track_name`, `artist_name` only: the search fallback carries no album or duration.
fn search_url(track: &TrackQuery) -> String {
    let artist = track.artists.first().map(String::as_str).unwrap_or("");
    format!("{API}/api/search?{}", query(&[("track_name", &track.title), ("artist_name", artist)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! fixture {
        ($path:literal) => {
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/lrclib/", $path))
        };
    }

    fn track(title: &str, artist: &str, album: Option<&str>, duration_ms: Option<u32>) -> TrackQuery {
        TrackQuery { id: None, title: title.into(), artists: vec![artist.into()], album: album.map(Into::into), duration_ms, isrc: None }
    }

    #[test]
    fn get_url_omits_absent_album_and_duration() {
        let full = get_url(&track("Yesterday", "The Beatles", Some("Help!"), Some(126_000)));
        assert_eq!(full, "https://lrclib.net/api/get?track_name=Yesterday&artist_name=The+Beatles&album_name=Help%21&duration=126");
        let bare = get_url(&track("Yesterday", "The Beatles", None, None));
        assert_eq!(bare, "https://lrclib.net/api/get?track_name=Yesterday&artist_name=The+Beatles");
    }

    #[test]
    fn search_url_carries_no_album_or_duration() {
        let url = search_url(&track("Yesterday", "The Beatles", Some("Help!"), Some(126_000)));
        assert_eq!(url, "https://lrclib.net/api/search?track_name=Yesterday&artist_name=The+Beatles");
    }

    #[test]
    fn instrumental_record_has_no_lyrics() {
        let t: LrcTrack = serde_json::from_slice(fixture!("instrumental/get.json")).unwrap();
        assert!(to_lyrics(t).is_err());
    }

    #[test]
    fn synced_text_parses_into_sorted_lines() {
        let t: LrcTrack = serde_json::from_slice(fixture!("get/get.json")).unwrap();
        let lyrics = to_lyrics(t).unwrap();
        assert_eq!(lyrics.synced[0].start_ms, 5_060);
        assert_eq!(lyrics.synced[0].text, "Yesterday, all my troubles seemed so far away");
        assert!(lyrics.synced.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));
    }
}
