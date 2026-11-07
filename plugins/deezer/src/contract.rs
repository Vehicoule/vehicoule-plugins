//! Between the contract's types and the plugin's own, field for field.

use std::time::Duration;

use crate::bindings::exports::vehicoule::plugin::metadata as m;
use crate::bindings::exports::vehicoule::plugin::search as s;
use crate::bindings::vehicoule::plugin::http;
use crate::bindings::vehicoule::plugin::types as t;
use crate::calls::ArtworkOf;
use crate::model::{
    Album, AlbumPage, AlbumRef, Artist, ArtistPage, ArtistRef, Artwork, Error, Item, ItemKind,
    Page, Playlist, PlaylistPage, Track,
};
use crate::read::{Response, Wanted};
use crate::request::Request;

/// Sends `request` with the host's `fetch` (or a test's), as a `GET` with no header and no body.
pub(crate) fn send<F>(request: &Request, fetch: F) -> Result<Response, Error>
where
    F: FnOnce(&http::Request) -> Result<http::Response, http::HttpError>,
{
    let response = fetch(&http::Request {
        method: http::Method::Get,
        url: request.url.clone(),
        headers: Vec::new(),
        body: None,
    })
    .map_err(|e| {
        let what = match e {
            http::HttpError::RefusedHost => String::from("refused host"),
            http::HttpError::RefusedScheme => String::from("refused scheme"),
            http::HttpError::BudgetExceeded => String::from("request budget exceeded"),
            http::HttpError::Timeout => String::from("timed out"),
            http::HttpError::Transport(message) => format!("transport: {message}"),
        };
        Error::Upstream(format!("the request did not complete: {what}"))
    })?;
    Ok(Response::new(
        response.status,
        response.headers,
        response.body,
    ))
}

pub(crate) fn kind(kind: t::ItemKind) -> ItemKind {
    match kind {
        t::ItemKind::Track => ItemKind::Track,
        t::ItemKind::Album => ItemKind::Album,
        t::ItemKind::Artist => ItemKind::Artist,
        t::ItemKind::Playlist => ItemKind::Playlist,
    }
}

fn names(artists: &[t::ArtistRef]) -> Vec<String> {
    artists.iter().map(|a| a.name.clone()).collect()
}

pub(crate) fn artwork_of(item: &t::Item) -> ArtworkOf {
    match item {
        t::Item::Track(track) => ArtworkOf::Track {
            wanted: Wanted {
                title: track.title.clone(),
                artists: names(&track.artists),
            },
            isrc: track.isrc.clone(),
        },
        t::Item::Album(album) => ArtworkOf::Album(Wanted {
            title: album.title.clone(),
            artists: names(&album.artists),
        }),
        t::Item::Artist(artist) => ArtworkOf::Artist(Wanted {
            title: artist.name.clone(),
            artists: Vec::new(),
        }),
        t::Item::Playlist(_) => ArtworkOf::Playlist,
    }
}

pub(crate) fn error(error: Error) -> t::Error {
    match error {
        Error::NotFound => t::Error::NotFound,
        Error::Unsupported => t::Error::Unsupported,
        // A delay the contract's `u64` of milliseconds cannot carry is not reported, rather than reported wrong.
        Error::RateLimited(after) => {
            t::Error::RateLimited(after.and_then(|d| u64::try_from(d.as_millis()).ok()))
        }
        Error::InvalidInput(message) => t::Error::InvalidInput(message),
        Error::Upstream(message) => t::Error::Upstream(message),
    }
}

pub(crate) fn artwork(artwork: Vec<Artwork>) -> Vec<t::Artwork> {
    artwork
        .into_iter()
        .map(|a| t::Artwork {
            url: a.url,
            width: Some(a.size),
            height: Some(a.size),
        })
        .collect()
}

fn artist_refs(artists: Vec<ArtistRef>) -> Vec<t::ArtistRef> {
    artists
        .into_iter()
        .map(|a| t::ArtistRef {
            id: a.id,
            name: a.name,
        })
        .collect()
}

/// A duration in the contract's milliseconds; one past what a `u64` of milliseconds holds is left out.
fn millis(duration: Duration) -> Option<u64> {
    u64::try_from(duration.as_millis()).ok()
}

pub(crate) fn track(track: Track) -> t::Track {
    t::Track {
        id: track.id,
        title: track.title,
        artists: artist_refs(track.artists),
        album: track
            .album
            .map(|AlbumRef { id, title }| t::AlbumRef { id, title }),
        duration_ms: track.duration.and_then(millis),
        isrc: track.isrc,
        artwork: artwork(track.artwork),
    }
}

pub(crate) fn album(album: Album) -> t::Album {
    t::Album {
        id: album.id,
        title: album.title,
        artists: artist_refs(album.artists),
        year: album.year,
        artwork: artwork(album.artwork),
    }
}

pub(crate) fn artist(artist: Artist) -> t::Artist {
    t::Artist {
        id: artist.id,
        name: artist.name,
        artwork: artwork(artist.artwork),
    }
}

pub(crate) fn playlist(playlist: Playlist) -> t::Playlist {
    t::Playlist {
        id: playlist.id,
        title: playlist.title,
        owner: playlist.owner,
        artwork: artwork(playlist.artwork),
    }
}

pub(crate) fn item(item: Item) -> t::Item {
    match item {
        Item::Track(x) => t::Item::Track(track(x)),
        Item::Album(x) => t::Item::Album(album(x)),
        Item::Artist(x) => t::Item::Artist(artist(x)),
        Item::Playlist(x) => t::Item::Playlist(playlist(x)),
    }
}

pub(crate) fn page(page: Page) -> s::Page {
    s::Page {
        items: page.items.into_iter().map(item).collect(),
        next: page.next,
    }
}

pub(crate) fn album_page(page: AlbumPage) -> m::AlbumPage {
    m::AlbumPage {
        album: album(page.album),
        tracks: page.tracks.into_iter().map(track).collect(),
    }
}

pub(crate) fn artist_page(page: ArtistPage) -> m::ArtistPage {
    m::ArtistPage {
        artist: artist(page.artist),
        top_tracks: page.top_tracks.into_iter().map(track).collect(),
        albums: page.albums.into_iter().map(album).collect(),
    }
}

pub(crate) fn playlist_page(page: PlaylistPage) -> m::PlaylistPage {
    m::PlaylistPage {
        playlist: playlist(page.playlist),
        tracks: page.tracks.into_iter().map(track).collect(),
        next: page.next,
    }
}

#[cfg(test)]
mod tests;
