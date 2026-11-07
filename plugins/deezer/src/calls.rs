//! The contract's functions, in the plugin's own types: each builds its requests ([`crate::request`]), sends them
//! with the `fetch` it is given, one after the other, and reads the answers ([`crate::read`]). The first failure ends
//! the call: no request after it, no retry, no other endpoint.

use crate::link;
use crate::model::{AlbumPage, ArtistPage, Artwork, Error, Item, ItemKind, Page, PlaylistPage};
use crate::read::{self, Response, Wanted};
use crate::request::{self, Request, SearchPlan};

/// What an artwork call is asked about, taken from the contract's `item`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum ArtworkOf {
    /// A track, by its names and its ISRC if it has one.
    Track {
        wanted: Wanted,
        isrc: Option<String>,
    },
    /// An album, by its names.
    Album(Wanted),
    /// An artist, by name.
    Artist(Wanted),
    /// A playlist, which only its own service can picture.
    Playlist,
}

pub(crate) fn search<F>(
    query: &str,
    kinds: &[ItemKind],
    cursor: Option<&str>,
    mut fetch: F,
) -> Result<Page, Error>
where
    F: FnMut(&Request) -> Result<Response, Error>,
{
    let index = request::cursor(cursor)?;
    let plan = request::search_plan(kinds);
    let response = fetch(&request::search(query, &plan, index)?)?;
    match &plan {
        SearchPlan::One(kind) => read::read_search(*kind, index, &response),
        SearchPlan::Several(kinds) => read::read_autocomplete(kinds, index, &response),
    }
}

pub(crate) fn suggest<F>(prefix: &str, mut fetch: F) -> Result<Vec<String>, Error>
where
    F: FnMut(&Request) -> Result<Response, Error>,
{
    read::read_suggestions(&fetch(&request::suggest(prefix)?)?)
}

pub(crate) fn album<F>(id: &str, mut fetch: F) -> Result<AlbumPage, Error>
where
    F: FnMut(&Request) -> Result<Response, Error>,
{
    let [album, tracks] = request::album(id)?;
    let album = fetch(&album)?;
    read::check(&album)?;
    let tracks = fetch(&tracks)?;
    read::read_album(&album, &tracks)
}

pub(crate) fn artist<F>(id: &str, mut fetch: F) -> Result<ArtistPage, Error>
where
    F: FnMut(&Request) -> Result<Response, Error>,
{
    let [artist, top, albums] = request::artist(id)?;
    let artist = fetch(&artist)?;
    read::check(&artist)?;
    let top = fetch(&top)?;
    read::check(&top)?;
    let albums = fetch(&albums)?;
    read::read_artist(&artist, &top, &albums)
}

pub(crate) fn playlist<F>(
    id: &str,
    cursor: Option<&str>,
    mut fetch: F,
) -> Result<PlaylistPage, Error>
where
    F: FnMut(&Request) -> Result<Response, Error>,
{
    let index = request::cursor(cursor)?;
    read::read_playlist(index, &fetch(&request::playlist(id, index)?)?)
}

pub(crate) fn artwork<F>(of: &ArtworkOf, mut fetch: F) -> Result<Vec<Artwork>, Error>
where
    F: FnMut(&Request) -> Result<Response, Error>,
{
    let (kind, wanted) = match of {
        ArtworkOf::Track {
            isrc: Some(isrc), ..
        } if request::is_isrc(isrc) => {
            return read::read_artwork_by_isrc(&fetch(&request::track_by_isrc(isrc))?);
        }
        ArtworkOf::Track { wanted, .. } => (ItemKind::Track, wanted),
        ArtworkOf::Album(wanted) => (ItemKind::Album, wanted),
        ArtworkOf::Artist(wanted) => (ItemKind::Artist, wanted),
        ArtworkOf::Playlist => return Err(Error::Unsupported),
    };
    if wanted.title.trim().is_empty() || (kind != ItemKind::Artist && wanted.artists.is_empty()) {
        return Err(Error::InvalidInput(String::from(
            "the item lacks a title or an artist to look it up by",
        )));
    }
    let query = match wanted.artists.first() {
        Some(artist) if kind != ItemKind::Artist => format!("{artist} {}", wanted.title),
        _ => wanted.title.clone(),
    };
    let response = fetch(&request::search(&query, &SearchPlan::One(kind), 0)?)?;
    read::read_artwork_search(kind, wanted, &response)
}

pub(crate) fn import_link<F>(url: &str, mut fetch: F) -> Result<Item, Error>
where
    F: FnMut(&Request) -> Result<Response, Error>,
{
    let link = link::parse(url)?;
    read::read_item(link.kind, &fetch(&request::item(link.kind, &link.id)?)?)
}

#[cfg(test)]
mod tests;
