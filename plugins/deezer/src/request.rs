//! The requests the plugin sends, all `GET` to `https://api.deezer.com`, built from a call's arguments (crate
//! documentation, "Service"). Every value from outside is checked or percent-encoded before it enters a URL: an id
//! is decimal digits, a cursor is a decimal index, a text is percent-encoded.

use crate::model::{Error, ItemKind};

/// The only host the plugin reaches, declared in its manifest.
pub const HOST: &str = "api.deezer.com";

/// Results asked per search page: Deezer's own default page size (crate documentation, "Service").
pub const SEARCH_LIMIT: u32 = 25;

/// Tracks asked per playlist page.
pub const PLAYLIST_LIMIT: u32 = 100;

/// Tracks asked for an album, which has no pages in the contract: the most Deezer was seen to return in one answer
/// (crate documentation, "Service"). An album with more is refused, not cut.
pub const ALBUM_TRACKS_LIMIT: u32 = 1000;

/// Top tracks asked for an artist page.
pub const TOP_TRACKS_LIMIT: u32 = 10;

/// Albums asked for an artist page; the page holds the first ones, in Deezer's order.
pub const ARTIST_ALBUMS_LIMIT: u32 = 50;

/// A request ready to send: a `GET` with no header and no body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Request {
    /// The full URL, `https`, on [`HOST`].
    pub url: String,
}

impl Request {
    fn new(path_and_query: &str) -> Self {
        Self {
            url: format!("https://{HOST}/{path_and_query}"),
        }
    }
}

/// How a search is asked: one endpoint per kind, or Deezer's autocomplete for several kinds at once.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SearchPlan {
    /// One kind, from its search endpoint.
    One(ItemKind),
    /// Several kinds, in this order, from the autocomplete endpoint.
    Several(Vec<ItemKind>),
}

/// Which kinds a search asks for: `kinds` without repeats, in its order, or every kind when it is empty.
#[must_use]
pub fn search_plan(kinds: &[ItemKind]) -> SearchPlan {
    let mut wanted: Vec<ItemKind> = Vec::new();
    for &kind in kinds {
        if !wanted.contains(&kind) {
            wanted.push(kind);
        }
    }
    match wanted.as_slice() {
        [one] => SearchPlan::One(*one),
        [] => SearchPlan::Several(vec![
            ItemKind::Track,
            ItemKind::Album,
            ItemKind::Artist,
            ItemKind::Playlist,
        ]),
        _ => SearchPlan::Several(wanted),
    }
}

/// The request for a page of search results.
///
/// # Errors
///
/// [`Error::InvalidInput`] for a blank query.
///
/// # Examples
///
/// ```
/// use vehicoule_plugin_deezer::model::ItemKind;
/// use vehicoule_plugin_deezer::request::{search, search_plan};
///
/// let request = search("Blue hour", &search_plan(&[ItemKind::Album]), 25).unwrap();
/// assert_eq!(request.url, "https://api.deezer.com/search/album?q=Blue%20hour&index=25&limit=25");
/// ```
pub fn search(query: &str, plan: &SearchPlan, index: u32) -> Result<Request, Error> {
    if query.trim().is_empty() {
        return Err(Error::InvalidInput(String::from("the query is blank")));
    }
    let path = match plan {
        SearchPlan::One(ItemKind::Track) => "search",
        SearchPlan::One(ItemKind::Album) => "search/album",
        SearchPlan::One(ItemKind::Artist) => "search/artist",
        SearchPlan::One(ItemKind::Playlist) => "search/playlist",
        SearchPlan::Several(_) => "search/autocomplete",
    };
    let mut url = format!("{path}?q=");
    percent_encode(query.trim().as_bytes(), &mut url);
    Ok(Request::new(&format!(
        "{url}&index={index}&limit={SEARCH_LIMIT}"
    )))
}

/// The request for suggestions: the first page of the autocomplete endpoint.
///
/// # Errors
///
/// [`Error::InvalidInput`] for a blank prefix.
pub fn suggest(prefix: &str) -> Result<Request, Error> {
    search(prefix, &SearchPlan::Several(Vec::new()), 0)
}

/// The two requests of an album page: the album, with one embedded track to keep the answer small, then all its
/// tracks, which carry their ISRCs where the embedded ones do not.
///
/// # Errors
///
/// [`Error::InvalidInput`] for an id that is not Deezer's ([`id`]).
pub fn album(album_id: &str) -> Result<[Request; 2], Error> {
    let album_id = id(album_id)?;
    Ok([
        Request::new(&format!("album/{album_id}?limit=1")),
        Request::new(&format!(
            "album/{album_id}/tracks?limit={ALBUM_TRACKS_LIMIT}"
        )),
    ])
}

/// The three requests of an artist page: the artist, their top tracks, their albums.
///
/// # Errors
///
/// [`Error::InvalidInput`] for an id that is not Deezer's ([`id`]).
pub fn artist(artist_id: &str) -> Result<[Request; 3], Error> {
    let artist_id = id(artist_id)?;
    Ok([
        Request::new(&format!("artist/{artist_id}")),
        Request::new(&format!("artist/{artist_id}/top?limit={TOP_TRACKS_LIMIT}")),
        Request::new(&format!(
            "artist/{artist_id}/albums?limit={ARTIST_ALBUMS_LIMIT}"
        )),
    ])
}

/// The request of a playlist page: the playlist with the tracks from `index`, in one answer.
///
/// # Errors
///
/// [`Error::InvalidInput`] for an id that is not Deezer's ([`id`]).
pub fn playlist(playlist_id: &str, index: u32) -> Result<Request, Error> {
    let playlist_id = id(playlist_id)?;
    Ok(Request::new(&format!(
        "playlist/{playlist_id}?index={index}&limit={PLAYLIST_LIMIT}"
    )))
}

/// The request for one item by kind and id, for an imported link. An album or a playlist is asked with one embedded
/// track, since only the item itself is returned.
///
/// # Errors
///
/// [`Error::InvalidInput`] for an id that is not Deezer's ([`id`]).
pub fn item(kind: ItemKind, item_id: &str) -> Result<Request, Error> {
    let item_id = id(item_id)?;
    Ok(Request::new(&match kind {
        ItemKind::Track => format!("track/{item_id}"),
        ItemKind::Album => format!("album/{item_id}?limit=1"),
        ItemKind::Artist => format!("artist/{item_id}"),
        ItemKind::Playlist => format!("playlist/{item_id}?limit=1"),
    }))
}

/// The request for the track with this ISRC, which [`is_isrc`] accepts.
#[must_use]
pub fn track_by_isrc(isrc: &str) -> Request {
    let mut path = String::from("track/isrc:");
    percent_encode(isrc.as_bytes(), &mut path);
    Request::new(&path)
}

/// Whether `isrc` has an ISRC's form: 12 ASCII letters and digits (ISO 3901).
#[must_use]
pub fn is_isrc(isrc: &str) -> bool {
    isrc.len() == 12 && isrc.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// The id of a Deezer item, as the plugin hands it out: decimal digits whose value fits a `u64`, as every id Deezer
/// returned was (crate documentation, "Service"). Checking it keeps a path from being written into a URL by whoever
/// gave the id.
///
/// # Errors
///
/// [`Error::InvalidInput`] for anything else.
///
/// # Examples
///
/// ```
/// use vehicoule_plugin_deezer::request::id;
///
/// assert_eq!(id("302127"), Ok(302127));
/// assert!(id("302127/tracks").is_err());
/// ```
pub fn id(id: &str) -> Result<u64, Error> {
    decimal(id).ok_or_else(|| Error::InvalidInput(format!("not a Deezer id: {id:?}")))
}

/// The value of `text` if it is nothing but ASCII decimal digits (no sign, no space) and fits `T`.
fn decimal<T: std::str::FromStr>(text: &str) -> Option<T> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// The index a cursor stands for: the plugin's cursors are the decimal index of the page's first result, and no
/// cursor is the first page.
///
/// # Errors
///
/// [`Error::InvalidInput`] for a cursor the plugin did not hand out: anything but decimal digits whose value fits a
/// `u32`.
pub fn cursor(cursor: Option<&str>) -> Result<u32, Error> {
    match cursor {
        None => Ok(0),
        Some(cursor) => {
            decimal(cursor).ok_or_else(|| Error::InvalidInput(format!("not a cursor: {cursor:?}")))
        }
    }
}

/// Appends `bytes` to `out`, percent-encoded for a URL's query string, one byte at a time ([`encode_byte`]). The
/// result holds nothing but ASCII, so no value can end the parameter or the query early.
///
/// # Examples
///
/// ```
/// use vehicoule_plugin_deezer::request::percent_encode;
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
/// use vehicoule_plugin_deezer::request::{EncodedByte, encode_byte};
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
mod tests;

#[cfg(kani)]
mod proofs;
