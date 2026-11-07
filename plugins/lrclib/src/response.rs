//! Reads LRCLIB's answer to [`crate::request::get`] (crate documentation, "Service").

use std::time::Duration;

use serde::Deserialize;

use crate::{Error, Lyrics, lrc};

/// The fields of a lyrics record the plugin reads; serde ignores the others. A field that is missing or of another
/// type makes the record unreadable.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    instrumental: bool,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
}

/// Reads the answer to a lyrics request: its status, headers and body.
///
/// - `200` with a JSON `Content-Type`: the record's synced lyrics if it has any that are not blank, read by
///   [`lrc::parse`], else its plain lyrics if they are not blank, else [`Error::NotFound`]. An instrumental record is
///   [`Error::NotFound`]: it has no lyrics.
/// - `404`: [`Error::NotFound`].
/// - `429` and `503`: [`Error::RateLimited`], with the delay of `Retry-After` if it is a number of seconds.
/// - Any other status, a body that is not a record, or synced lyrics that are not LRC: [`Error::Upstream`].
///
/// # Errors
///
/// As above: every answer that holds no lyrics is an error.
///
/// # Examples
///
/// ```
/// use vehicoule_plugin_lrclib::{Error, Lyrics, response::read_get};
///
/// let json = vec![(String::from("content-type"), String::from("application/json"))];
/// let body = br#"{"instrumental":false,"plainLyrics":"la la","syncedLyrics":null}"#;
/// assert_eq!(read_get(200, &json, body), Ok(Lyrics::Plain(String::from("la la"))));
/// assert_eq!(read_get(404, &json, b"{}"), Err(Error::NotFound));
/// ```
pub fn read_get(status: u16, headers: &[(String, String)], body: &[u8]) -> Result<Lyrics, Error> {
    match status {
        200 => {}
        404 => return Err(Error::NotFound),
        429 | 503 => return Err(Error::RateLimited(retry_after(headers))),
        _ => {
            return Err(Error::Upstream(format!(
                "LRCLIB answered with status {status}"
            )));
        }
    }
    let content_type = header(headers, "content-type").unwrap_or_default();
    if !is_json(content_type) {
        return Err(Error::Upstream(format!(
            "LRCLIB answered with Content-Type {content_type:?}, not JSON"
        )));
    }
    let record: Record = serde_json::from_slice(body)
        .map_err(|e| Error::Upstream(format!("LRCLIB's record is unreadable: {e}")))?;
    if record.instrumental {
        return Err(Error::NotFound);
    }
    if let Some(synced) = record.synced_lyrics.filter(|s| !s.trim().is_empty()) {
        return lrc::parse(&synced).map(Lyrics::Synced);
    }
    match record.plain_lyrics.filter(|p| !p.trim().is_empty()) {
        Some(plain) => Ok(Lyrics::Plain(plain)),
        None => Err(Error::NotFound),
    }
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

/// The delay of `Retry-After`, when it is a number of seconds that the contract's milliseconds can carry. The other
/// form the header may take, an HTTP date, needs a clock to read, which a plugin does not have: it gives `None`.
#[must_use]
pub fn retry_after(headers: &[(String, String)]) -> Option<Duration> {
    let seconds: u64 = header(headers, "retry-after")?.trim().parse().ok()?;
    let ms = seconds.checked_mul(1_000)?;
    Some(Duration::from_millis(ms))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TimedLine;

    fn json() -> Vec<(String, String)> {
        vec![(
            String::from("Content-Type"),
            String::from("application/json; charset=utf-8"),
        )]
    }

    fn with_retry_after(value: &str) -> Vec<(String, String)> {
        vec![(String::from("retry-after"), String::from(value))]
    }

    fn upstream(result: Result<Lyrics, Error>) -> String {
        match result {
            Err(Error::Upstream(message)) => message,
            other => panic!("expected an upstream error, got {other:?}"),
        }
    }

    #[test]
    fn prefers_synced_lyrics() {
        let body = br#"{"id":1,"trackName":"t","instrumental":false,"plainLyrics":"a","syncedLyrics":"[00:02.00] a"}"#;
        assert_eq!(
            read_get(200, &json(), body),
            Ok(Lyrics::Synced(vec![TimedLine {
                start: Duration::from_secs(2),
                text: String::from("a"),
            }]))
        );
    }

    #[test]
    fn blank_synced_lyrics_give_the_plain_ones() {
        let body = br#"{"instrumental":false,"plainLyrics":"a\n","syncedLyrics":" \n"}"#;
        assert_eq!(
            read_get(200, &json(), body),
            Ok(Lyrics::Plain(String::from("a\n")))
        );
    }

    #[test]
    fn a_record_without_lyrics_is_not_found() {
        for body in [
            br#"{"instrumental":true,"plainLyrics":null,"syncedLyrics":null}"#.as_slice(),
            br#"{"instrumental":true,"plainLyrics":"a","syncedLyrics":"[00:01.00] a"}"#,
            br#"{"instrumental":false,"plainLyrics":null,"syncedLyrics":null}"#,
            br#"{"instrumental":false,"plainLyrics":"  ","syncedLyrics":""}"#,
        ] {
            assert_eq!(
                read_get(200, &json(), body),
                Err(Error::NotFound),
                "{}",
                String::from_utf8_lossy(body)
            );
        }
    }

    #[test]
    fn synced_lyrics_that_are_not_lrc_are_upstream() {
        let body = br#"{"instrumental":false,"plainLyrics":"a","syncedLyrics":"just words"}"#;
        assert_eq!(
            upstream(read_get(200, &json(), body)),
            "synced lyrics, line 1: not LRC"
        );
    }

    #[test]
    fn a_body_that_is_not_a_record_is_upstream() {
        for body in [
            b"".as_slice(),
            b"not json",
            b"[]",
            b"null",
            br#"{"plainLyrics":"a","syncedLyrics":null}"#,
            br#"{"instrumental":"no","plainLyrics":"a","syncedLyrics":null}"#,
            br#"{"instrumental":false,"plainLyrics":7,"syncedLyrics":null}"#,
            br#"{"instrumental":false,"plainLyrics":"a","syncedLyrics":["x"]}"#,
            b"{\"instrumental\":false,\"plainLyrics\":\"\xff\",\"syncedLyrics\":null}",
        ] {
            let message = upstream(read_get(200, &json(), body));
            assert!(
                message.starts_with("LRCLIB's record is unreadable: "),
                "{message}"
            );
        }
    }

    #[test]
    fn a_200_that_is_not_json_is_upstream() {
        let body = br#"{"instrumental":false,"plainLyrics":"a","syncedLyrics":null}"#;
        let html = vec![(String::from("content-type"), String::from("text/html"))];
        assert_eq!(
            upstream(read_get(200, &html, body)),
            "LRCLIB answered with Content-Type \"text/html\", not JSON"
        );
        assert_eq!(
            upstream(read_get(200, &[], body)),
            "LRCLIB answered with Content-Type \"\", not JSON"
        );
    }

    #[test]
    fn json_is_recognized_with_any_case_and_parameters() {
        assert!(is_json("application/json"));
        assert!(is_json("Application/JSON ; charset=utf-8"));
        assert!(!is_json("application/jsonp"));
        assert!(!is_json("text/plain"));
        assert!(!is_json(""));
    }

    #[test]
    fn not_found_whatever_the_body() {
        for body in [b"".as_slice(), b"not json", br#"{"name":"TrackNotFound"}"#] {
            assert_eq!(read_get(404, &[], body), Err(Error::NotFound));
        }
    }

    #[test]
    fn rate_limits_carry_the_delay_when_it_is_seconds() {
        for status in [429, 503] {
            assert_eq!(
                read_get(status, &with_retry_after(" 3 "), b""),
                Err(Error::RateLimited(Some(Duration::from_secs(3))))
            );
            assert_eq!(
                read_get(status, &[], b"not json"),
                Err(Error::RateLimited(None))
            );
        }
    }

    #[test]
    fn a_retry_after_that_is_not_seconds_gives_no_delay() {
        for value in [
            "Sat, 07 Nov 2026 15:00:00 GMT",
            "-1",
            "1.5",
            "",
            "18446744073709552",
        ] {
            assert_eq!(retry_after(&with_retry_after(value)), None, "{value}");
        }
        assert_eq!(
            retry_after(&with_retry_after("18446744073709551")),
            Some(Duration::from_secs(18_446_744_073_709_551))
        );
    }

    #[test]
    fn the_first_retry_after_counts() {
        let headers = vec![
            (String::from("Retry-After"), String::from("2")),
            (String::from("retry-after"), String::from("9")),
        ];
        assert_eq!(retry_after(&headers), Some(Duration::from_secs(2)));
    }

    #[test]
    fn any_other_status_is_upstream() {
        for status in [0, 100, 201, 204, 301, 400, 401, 403, 500, 502, u16::MAX] {
            assert_eq!(
                upstream(read_get(status, &json(), b"{}")),
                format!("LRCLIB answered with status {status}")
            );
        }
    }
}
