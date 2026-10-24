//! LRCLIB (`lrclib.net`) request building and JSON parsing. `GET /api/get` is an exact match
//! on track/artist/album/duration; a 404 there falls back to `GET /api/search` (track/artist
//! only), picking the first result within 2s of the query's duration (or just the first, when
//! the query carries no duration).

use serde::Deserialize;
use vehicoule_sdk::host_http::{self, Method, Request};
use vehicoule_sdk::types::{Error, TrackQuery};
use vehicoule_sdk::{json, query};

use crate::exports::vehicoule::music::lyrics::{Line, Lyrics};
use crate::lrc::parse_lrc;

const API: &str = "https://lrclib.net";
/// A search hit counts as matching the query's duration within this many seconds either way.
const DURATION_TOLERANCE_S: f64 = 2.0;

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
    let req = Request {
        method: Method::Get,
        url: get_url(track),
        headers: vec![],
        body: None,
    };
    let resp = host_http::send(&req).map_err(vehicoule_sdk::from_http_error)?;
    match resp.status {
        200..=299 => to_lyrics(json(&resp.body)?),
        404 => search_fallback(track),
        status => Err(vehicoule_sdk::status_error(status)
            .unwrap_or(Error::Internal(format!("HTTP {status}")))),
    }
}

/// `GET /api/search`: picks the first result within `DURATION_TOLERANCE_S` of the query's
/// duration, or the first result outright when the query has none. No match (or an empty
/// result list) is `not-found`, same as a direct miss.
fn search_fallback(track: &TrackQuery) -> Result<Lyrics, Error> {
    let req = Request {
        method: Method::Get,
        url: search_url(track),
        headers: vec![],
        body: None,
    };
    let resp = host_http::send(&req).map_err(vehicoule_sdk::from_http_error)?;
    if let Some(e) = vehicoule_sdk::status_error(resp.status) {
        return Err(e);
    }
    let results: Vec<LrcTrack> = json(&resp.body)?;
    to_lyrics(pick(results, track.duration_ms).ok_or(Error::NotFound)?)
}

/// The first result within `DURATION_TOLERANCE_S` of `duration_ms`, or the first outright
/// when there is no duration to match. The remote duration is compared in `f64`: a huge one
/// cannot overflow, and a NaN one never matches.
fn pick(results: Vec<LrcTrack>, duration_ms: Option<u32>) -> Option<LrcTrack> {
    let wanted_s = duration_ms.map(|ms| (f64::from(ms) / 1000.0).round());
    match wanted_s {
        None => results.into_iter().next(),
        Some(wanted) => results.into_iter().find(|r| {
            r.duration
                .is_some_and(|d| (d.round() - wanted).abs() <= DURATION_TOLERANCE_S)
        }),
    }
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

/// `track_name`, `artist_name` (the query's first artist), `album_name` (omitted when the
/// query has none), `duration` in whole seconds (omitted when the query has none).
fn get_url(track: &TrackQuery) -> String {
    let artist = track.artists.first().map(String::as_str).unwrap_or("");
    let duration = track
        .duration_ms
        .map(|ms| ((ms as f64 / 1000.0).round() as u32).to_string());
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
    format!(
        "{API}/api/search?{}",
        query(&[("track_name", &track.title), ("artist_name", artist)])
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! fixture {
        ($path:literal) => {
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/lrclib/",
                $path
            ))
        };
    }

    fn track(
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration_ms: Option<u32>,
    ) -> TrackQuery {
        TrackQuery {
            id: None,
            title: title.into(),
            artists: vec![artist.into()],
            album: album.map(Into::into),
            duration_ms,
            isrc: None,
        }
    }

    #[test]
    fn get_url_omits_absent_album_and_duration() {
        let full = get_url(&track(
            "Yesterday",
            "The Beatles",
            Some("Help!"),
            Some(126_000),
        ));
        assert_eq!(
            full,
            "https://lrclib.net/api/get?track_name=Yesterday&artist_name=The+Beatles&album_name=Help%21&duration=126"
        );
        let bare = get_url(&track("Yesterday", "The Beatles", None, None));
        assert_eq!(
            bare,
            "https://lrclib.net/api/get?track_name=Yesterday&artist_name=The+Beatles"
        );
    }

    #[test]
    fn search_url_carries_no_album_or_duration() {
        let url = search_url(&track(
            "Yesterday",
            "The Beatles",
            Some("Help!"),
            Some(126_000),
        ));
        assert_eq!(
            url,
            "https://lrclib.net/api/search?track_name=Yesterday&artist_name=The+Beatles"
        );
    }

    fn record(duration: Option<f64>, text: &str) -> LrcTrack {
        LrcTrack {
            duration,
            instrumental: false,
            plain_lyrics: Some(text.into()),
            synced_lyrics: None,
        }
    }

    #[test]
    fn pick_takes_the_first_result_within_two_seconds() {
        let results = vec![
            record(Some(130.0), "far"),
            record(Some(124.4), "near"),
            record(Some(126.0), "exact"),
        ];
        assert_eq!(
            pick(results, Some(126_000))
                .and_then(|r| r.plain_lyrics)
                .as_deref(),
            Some("near")
        );
        let results = vec![record(None, "first"), record(Some(1.0), "second")];
        assert_eq!(
            pick(results, None).and_then(|r| r.plain_lyrics).as_deref(),
            Some("first")
        );
    }

    /// LRCLIB's `duration` is remote input: a huge, infinite or NaN value must neither panic
    /// (no integer overflow) nor pass as a match.
    #[test]
    fn pick_survives_absurd_remote_durations() {
        for d in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            1e300,
            -1e300,
            f64::MAX,
            f64::MIN,
        ] {
            for ms in [0, 1_000, 126_000, u32::MAX] {
                let results = vec![
                    record(Some(d), "absurd"),
                    record(Some((ms / 1000) as f64), "real"),
                ];
                let picked = pick(results, Some(ms)).and_then(|r| r.plain_lyrics);
                assert_eq!(picked.as_deref(), Some("real"), "duration {d} vs {ms} ms");
            }
        }
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
        assert_eq!(
            lyrics.synced[0].text,
            "Yesterday, all my troubles seemed so far away"
        );
        assert!(
            lyrics
                .synced
                .windows(2)
                .all(|w| w[0].start_ms <= w[1].start_ms)
        );
    }
}
