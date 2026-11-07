//! The request for a track's lyrics: `GET https://lrclib.net/api/get` with the track's signature in the query string
//! (crate documentation, "Service").

use std::time::Duration;

use crate::Error;

/// The only host the plugin reaches, declared in its manifest.
pub const HOST: &str = "lrclib.net";

/// The shortest and longest durations LRCLIB accepts, in seconds (crate documentation, "Service").
const DURATION_SECONDS: std::ops::RangeInclusive<u128> = 1..=3600;

/// What the plugin knows of the track whose lyrics it looks for, taken from the contract's `track`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Query {
    /// The track's title.
    pub title: String,
    /// The names of the track's artists, in the track's order.
    pub artists: Vec<String>,
    /// The title of the track's album, if known.
    pub album: Option<String>,
    /// The track's duration, if known.
    pub duration: Option<Duration>,
}

/// A request ready to send: a `GET` with no body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Request {
    /// The full URL, `https`, on [`HOST`].
    pub url: String,
    /// The request's headers, as name and value.
    pub headers: Vec<(String, String)>,
}

/// Builds the request for `query`.
///
/// The title and the artists are required by LRCLIB: a query whose title, or every artist name, is empty or blank is
/// refused with [`Error::InvalidInput`], and no request is made. The artists are joined with `, `, as LRCLIB names
/// tracks of several artists. The duration is sent in whole seconds, rounded to the nearest, and left out when it is
/// outside what LRCLIB accepts (1 to 3600 seconds) rather than sent for LRCLIB to refuse. Every value is
/// percent-encoded ([`percent_encode`]).
///
/// # Errors
///
/// [`Error::InvalidInput`] when the title or the artists are missing.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use vehicoule_plugin_lrclib::request::{get, Query};
///
/// let mut query = Query::default();
/// query.title = String::from("Blue Hour");
/// query.artists = vec![String::from("Ana")];
/// query.duration = Some(Duration::from_millis(181_400));
/// let request = get(&query).unwrap();
/// assert_eq!(request.url, "https://lrclib.net/api/get?track_name=Blue%20Hour&artist_name=Ana&duration=181");
/// ```
pub fn get(query: &Query) -> Result<Request, Error> {
    if query.title.trim().is_empty() {
        return Err(Error::InvalidInput(String::from("the track has no title")));
    }
    let artists: Vec<&str> = query
        .artists
        .iter()
        .map(String::as_str)
        .filter(|name| !name.trim().is_empty())
        .collect();
    if artists.is_empty() {
        return Err(Error::InvalidInput(String::from("the track has no artist")));
    }
    let mut url = format!("https://{HOST}/api/get?track_name=");
    percent_encode(query.title.as_bytes(), &mut url);
    url.push_str("&artist_name=");
    percent_encode(artists.join(", ").as_bytes(), &mut url);
    if let Some(album) = query.album.as_deref().filter(|a| !a.trim().is_empty()) {
        url.push_str("&album_name=");
        percent_encode(album.as_bytes(), &mut url);
    }
    if let Some(seconds) = query.duration.and_then(rounded_seconds) {
        url.push_str("&duration=");
        url.push_str(&seconds.to_string());
    }
    Ok(Request {
        url,
        headers: vec![(String::from("User-Agent"), user_agent())],
    })
}

/// `duration` in whole seconds, rounded to the nearest (half a second up), if LRCLIB accepts it.
fn rounded_seconds(duration: Duration) -> Option<u128> {
    let seconds = duration.as_millis().checked_add(500)?.checked_div(1000)?;
    DURATION_SECONDS.contains(&seconds).then_some(seconds)
}

/// The client's name, version and project page, as LRCLIB requires (crate documentation, "Service").
fn user_agent() -> String {
    format!(
        "vehicoule-lrclib/{} (https://github.com/Vehicoule/vehicoule-plugins)",
        env!("CARGO_PKG_VERSION")
    )
}

/// Appends `bytes` to `out`, percent-encoded for a URL's query string, one byte at a time ([`encode_byte`]). The
/// result holds nothing but ASCII, so no value can end the parameter or the query early.
///
/// # Examples
///
/// ```
/// use vehicoule_plugin_lrclib::request::percent_encode;
///
/// let mut out = String::new();
/// percent_encode("Mae & Co, é".as_bytes(), &mut out);
/// assert_eq!(out, "Mae%20%26%20Co%2C%20%C3%A9");
/// ```
pub fn percent_encode(bytes: &[u8], out: &mut String) {
    for &byte in bytes {
        match encode_byte(byte) {
            EncodedByte::Kept(kept) => out.push(char::from(kept)),
            EncodedByte::Escaped(escape) => out.extend(escape.map(char::from)),
        }
    }
}

/// How [`encode_byte`] writes one byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodedByte {
    /// The byte itself, an unreserved character.
    Kept(u8),
    /// `%` and the byte's two hexadecimal digits, in uppercase.
    Escaped([u8; 3]),
}

/// How one byte is written in a URL's query string: the unreserved characters of RFC 3986, section 2.3 (ASCII
/// letters, digits, `-`, `.`, `_`, `~`), are kept, and every other byte is written `%XX`, in uppercase hexadecimal.
///
/// # Examples
///
/// ```
/// use vehicoule_plugin_lrclib::request::{EncodedByte, encode_byte};
///
/// assert_eq!(encode_byte(b'a'), EncodedByte::Kept(b'a'));
/// assert_eq!(encode_byte(b' '), EncodedByte::Escaped(*b"%20"));
/// ```
#[must_use]
pub fn encode_byte(byte: u8) -> EncodedByte {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
        return EncodedByte::Kept(byte);
    }
    // `>> 4` of a `u8` and `& 0xF` are both below 16, `HEX`'s length, so neither `get` misses.
    let digit = |nibble: u8| HEX.get(usize::from(nibble)).copied().unwrap_or(b'0');
    EncodedByte::Escaped([b'%', digit(byte >> 4), digit(byte & 0xF)])
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    fn query() -> Query {
        Query {
            title: String::from("Night Train"),
            artists: vec![String::from("The Owls")],
            album: None,
            duration: None,
        }
    }

    #[test]
    fn keeps_unreserved_characters_and_encodes_every_other_byte() {
        let all: Vec<u8> = (0..=u8::MAX).collect();
        let mut out = String::new();
        percent_encode(&all, &mut out);
        let mut expected = String::new();
        for byte in all {
            if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
                expected.push(char::from(byte));
            } else {
                write!(expected, "%{byte:02X}").unwrap();
            }
        }
        assert_eq!(out, expected);
    }

    #[test]
    fn appends_to_what_is_already_written() {
        let mut out = String::from("q=");
        percent_encode(b"a b", &mut out);
        assert_eq!(out, "q=a%20b");
    }

    #[test]
    fn blank_artists_are_left_out_and_the_rest_joined() {
        let mut query = query();
        query.artists = vec![
            String::from(" "),
            String::from("A"),
            String::new(),
            String::from("B"),
        ];
        let request = get(&query).unwrap();
        assert_eq!(
            request.url,
            "https://lrclib.net/api/get?track_name=Night%20Train&artist_name=A%2C%20B"
        );
    }

    #[test]
    fn only_blank_artists_is_no_artist() {
        let mut query = query();
        query.artists = vec![String::from(" "), String::new()];
        assert_eq!(
            get(&query),
            Err(Error::InvalidInput(String::from("the track has no artist")))
        );
    }

    #[test]
    fn an_empty_title_is_refused() {
        let mut query = query();
        query.title = String::new();
        assert_eq!(
            get(&query),
            Err(Error::InvalidInput(String::from("the track has no title")))
        );
    }

    #[test]
    fn a_blank_album_is_left_out() {
        let mut query = query();
        query.album = Some(String::from("  "));
        assert_eq!(
            get(&query).unwrap().url,
            "https://lrclib.net/api/get?track_name=Night%20Train&artist_name=The%20Owls"
        );
    }

    #[test]
    fn the_duration_is_rounded_to_the_nearest_second() {
        assert_eq!(rounded_seconds(Duration::from_millis(1_499)), Some(1));
        assert_eq!(rounded_seconds(Duration::from_millis(1_500)), Some(2));
        assert_eq!(rounded_seconds(Duration::from_millis(500)), Some(1));
        assert_eq!(
            rounded_seconds(Duration::from_millis(3_600_499)),
            Some(3600)
        );
    }

    #[test]
    fn a_duration_lrclib_refuses_is_left_out() {
        assert_eq!(rounded_seconds(Duration::ZERO), None);
        assert_eq!(rounded_seconds(Duration::from_millis(499)), None);
        assert_eq!(rounded_seconds(Duration::from_millis(3_600_500)), None);
        assert_eq!(rounded_seconds(Duration::MAX), None);
        let mut query = query();
        query.duration = Some(Duration::from_secs(4_000));
        assert_eq!(
            get(&query).unwrap().url,
            "https://lrclib.net/api/get?track_name=Night%20Train&artist_name=The%20Owls"
        );
    }

    #[test]
    fn names_the_client_with_its_version_and_page() {
        assert_eq!(
            get(&query()).unwrap().headers,
            vec![(
                String::from("User-Agent"),
                format!(
                    "vehicoule-lrclib/{} (https://github.com/Vehicoule/vehicoule-plugins)",
                    env!("CARGO_PKG_VERSION")
                )
            )]
        );
    }
}

#[cfg(kani)]
mod proofs;
