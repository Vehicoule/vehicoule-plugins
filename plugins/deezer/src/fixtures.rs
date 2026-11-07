//! Answers written for the tests, in the shape Deezer was checked to return (crate documentation, "Service"), with
//! made-up values (docs/plugin-crates.md, section 3).

use crate::model::{AlbumRef, ArtistRef, Artwork, Track};
use crate::read::Response;

/// An answer with status `200`, Deezer's `Content-Type` and `body`.
pub(crate) fn ok(body: &str) -> Response {
    Response::new(
        200,
        vec![(
            String::from("Content-Type"),
            String::from("application/json; charset=utf-8"),
        )],
        body.as_bytes().to_vec(),
    )
}

/// Deezer's "no data" error.
pub(crate) const NO_DATA: &str =
    r#"{"error":{"type":"DataException","message":"no data","code":800}}"#;

/// The four covers of album 55, as the JSON fields of an album.
pub(crate) const COVERS: &str = r#""cover":"https://api.deezer.com/album/55/image","cover_small":"https://img.example/c/56x56.jpg","cover_medium":"https://img.example/c/250x250.jpg","cover_big":"https://img.example/c/500x500.jpg","cover_xl":"https://img.example/c/1000x1000.jpg""#;

/// The four pictures of an artist or a playlist, as JSON fields.
pub(crate) const PICTURES: &str = r#""picture":"https://api.deezer.com/artist/7/image","picture_small":"https://img.example/p/56x56.jpg","picture_medium":"https://img.example/p/250x250.jpg","picture_big":"https://img.example/p/500x500.jpg","picture_xl":"https://img.example/p/1000x1000.jpg""#;

/// A track as `/track/{id}` gives it, with contributors and its album.
pub(crate) fn track_json() -> String {
    format!(
        r#"{{"id":101,"readable":true,"title":"Night Train (Live)","title_short":"Night Train","title_version":"(Live)","isrc":"QZABC2600001","duration":201,"rank":5,"artist":{{"id":7,"name":"The Owls","type":"artist"}},"contributors":[{{"id":7,"name":"The Owls"}},{{"id":8,"name":"Mae"}}],"album":{{"id":55,"title":"Lights",{COVERS},"type":"album"}},"type":"track"}}"#
    )
}

/// An album as `/album/{id}` gives it, with one embedded track.
pub(crate) fn album_json() -> String {
    format!(
        r#"{{"id":55,"title":"Lights","upc":"000000000055","release_date":"2019-04-12","nb_tracks":2,{COVERS},"contributors":[{{"id":7,"name":"The Owls","role":"Main"}}],"artist":{{"id":7,"name":"The Owls"}},"type":"album","tracks":{{"data":[{{"id":101,"title":"Night Train (Live)","duration":201,"artist":{{"id":7,"name":"The Owls"}}}}]}}}}"#
    )
}

/// An artist as `/artist/{id}` gives it.
pub(crate) fn artist_json() -> String {
    format!(r#"{{"id":7,"name":"The Owls",{PICTURES},"nb_album":2,"nb_fan":10,"type":"artist"}}"#)
}

/// A playlist as `/playlist/{id}` gives it, with `tracks` embedded and `nb_tracks` tracks in all.
pub(crate) fn playlist_json(nb_tracks: u64, tracks: &[String]) -> String {
    format!(
        r#"{{"id":900,"title":"Road Mix","nb_tracks":{nb_tracks},{PICTURES},"creator":{{"id":3,"name":"Ana","type":"user"}},"type":"playlist","tracks":{{"data":[{}],"checksum":"x"}}}}"#,
        tracks.join(",")
    )
}

/// A list answer: `{"data":[…],"total":…}`, with `next` when `next` is true.
pub(crate) fn list(items: &[String], next: bool) -> String {
    let next = if next {
        r#","next":"https://api.deezer.com/search?q=x&index=25""#
    } else {
        ""
    };
    format!(r#"{{"data":[{}],"total":99{next}}}"#, items.join(","))
}

/// The covers of album 55, as the plugin reads them.
pub(crate) fn covers() -> Vec<Artwork> {
    [
        ("https://img.example/c/56x56.jpg", 56),
        ("https://img.example/c/250x250.jpg", 250),
        ("https://img.example/c/500x500.jpg", 500),
        ("https://img.example/c/1000x1000.jpg", 1000),
    ]
    .into_iter()
    .map(|(url, size)| Artwork {
        url: String::from(url),
        size,
    })
    .collect()
}

/// The pictures of [`PICTURES`], as the plugin reads them.
pub(crate) fn pictures() -> Vec<Artwork> {
    [
        ("https://img.example/p/56x56.jpg", 56),
        ("https://img.example/p/250x250.jpg", 250),
        ("https://img.example/p/500x500.jpg", 500),
        ("https://img.example/p/1000x1000.jpg", 1000),
    ]
    .into_iter()
    .map(|(url, size)| Artwork {
        url: String::from(url),
        size,
    })
    .collect()
}

/// [`track_json`], as the plugin reads it.
pub(crate) fn track() -> Track {
    Track {
        id: String::from("101"),
        title: String::from("Night Train (Live)"),
        artists: vec![artist_ref("7", "The Owls"), artist_ref("8", "Mae")],
        album: Some(AlbumRef {
            id: String::from("55"),
            title: String::from("Lights"),
        }),
        duration: Some(std::time::Duration::from_secs(201)),
        isrc: Some(String::from("QZABC2600001")),
        artwork: covers(),
    }
}

pub(crate) fn artist_ref(id: &str, name: &str) -> ArtistRef {
    ArtistRef {
        id: String::from(id),
        name: String::from(name),
    }
}
