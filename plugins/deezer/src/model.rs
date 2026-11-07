//! What the plugin reads from Deezer, in its own types: the readers ([`crate::read`]) return them, and the glue turns
//! them into the contract's (`types` in `wit/plugin.wit`), field for field.

use std::time::Duration;

/// A picture of an item, at one size.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Artwork {
    /// The picture's URL; always `https`.
    pub url: String,
    /// Its width and height in pixels, as Deezer serves it (crate documentation, "Service").
    pub size: u32,
}

/// An artist named by an item.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ArtistRef {
    /// Deezer's id of the artist, in decimal.
    pub id: String,
    /// The artist's name.
    pub name: String,
}

/// An album named by a track.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct AlbumRef {
    /// Deezer's id of the album, in decimal.
    pub id: String,
    /// The album's title.
    pub title: String,
}

/// A track.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Track {
    /// Deezer's id of the track, in decimal.
    pub id: String,
    /// The track's full title, with its version (`Get Lucky (Radio Edit)`).
    pub title: String,
    /// The track's artists: its contributors when Deezer lists them, else its main artist.
    pub artists: Vec<ArtistRef>,
    /// The track's album, when the answer names it.
    pub album: Option<AlbumRef>,
    /// The track's duration.
    pub duration: Option<Duration>,
    /// The track's ISRC, when the answer gives it.
    pub isrc: Option<String>,
    /// The covers of the track's album.
    pub artwork: Vec<Artwork>,
}

/// An album.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Album {
    /// Deezer's id of the album, in decimal.
    pub id: String,
    /// The album's title.
    pub title: String,
    /// The album's artists: its contributors when Deezer lists them, else its main artist.
    pub artists: Vec<ArtistRef>,
    /// The year of the album's release date.
    pub year: Option<u16>,
    /// The album's covers.
    pub artwork: Vec<Artwork>,
}

/// An artist.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Artist {
    /// Deezer's id of the artist, in decimal.
    pub id: String,
    /// The artist's name.
    pub name: String,
    /// The artist's pictures.
    pub artwork: Vec<Artwork>,
}

/// A playlist.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Playlist {
    /// Deezer's id of the playlist, in decimal.
    pub id: String,
    /// The playlist's title.
    pub title: String,
    /// The name of the playlist's creator.
    pub owner: Option<String>,
    /// The playlist's pictures.
    pub artwork: Vec<Artwork>,
}

/// One search result, or an imported link.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Item {
    /// A track.
    Track(Track),
    /// An album.
    Album(Album),
    /// An artist.
    Artist(Artist),
    /// A playlist.
    Playlist(Playlist),
}

/// The kinds of item a search can ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ItemKind {
    /// Tracks.
    Track,
    /// Albums.
    Album,
    /// Artists.
    Artist,
    /// Playlists.
    Playlist,
}

/// A page of search results.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Page {
    /// The results, in Deezer's order, grouped by kind in the order the search asked for them.
    pub items: Vec<Item>,
    /// The cursor of the next page, if Deezer has one.
    pub next: Option<String>,
}

/// An album with its tracks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct AlbumPage {
    /// The album.
    pub album: Album,
    /// Every track of the album, in its order.
    pub tracks: Vec<Track>,
}

/// An artist with their top tracks and albums.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ArtistPage {
    /// The artist.
    pub artist: Artist,
    /// The artist's most played tracks, in Deezer's order.
    pub top_tracks: Vec<Track>,
    /// The artist's albums, in Deezer's order.
    pub albums: Vec<Album>,
}

/// A playlist with one page of its tracks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PlaylistPage {
    /// The playlist.
    pub playlist: Playlist,
    /// The page's tracks, in the playlist's order.
    pub tracks: Vec<Track>,
    /// The cursor of the next page, if the playlist has more tracks.
    pub next: Option<String>,
}

/// Why a call has no answer, in the plugin's own types; each one becomes the contract's error of the same name.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Deezer has no such item, or no item matches.
    NotFound,
    /// The plugin does not provide this for this input.
    Unsupported,
    /// Deezer asked to wait, for the delay it gave if it gave one the plugin could read.
    RateLimited(Option<Duration>),
    /// The call's input cannot be asked of Deezer; no request was sent.
    InvalidInput(String),
    /// Deezer failed, the request did not reach it, or it answered something the plugin cannot read.
    Upstream(String),
}
