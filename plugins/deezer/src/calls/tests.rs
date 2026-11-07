use super::*;
use crate::fixtures::{self, NO_DATA, ok};

/// A fake `fetch` that answers with `answers` in order and keeps the URLs it was asked, and fails the test if it is
/// asked more often than it has answers.
struct Fake {
    answers: Vec<Result<Response, Error>>,
    asked: Vec<String>,
}

impl Fake {
    fn new(answers: Vec<Result<Response, Error>>) -> Self {
        Self {
            answers: answers.into_iter().rev().collect(),
            asked: Vec::new(),
        }
    }

    fn fetch(&mut self) -> impl FnMut(&Request) -> Result<Response, Error> + '_ {
        |request| {
            self.asked.push(request.url.clone());
            self.answers.pop().expect("asked more often than expected")
        }
    }
}

/// A fake `fetch` for a call that must fail before any request.
fn never(request: &Request) -> Result<Response, Error> {
    panic!("no request expected, got {}", request.url)
}

fn track_list(next: bool) -> String {
    fixtures::list(&[fixtures::track_json()], next)
}

#[test]
fn a_search_of_one_kind_asks_its_endpoint_from_the_cursor() {
    let mut fake = Fake::new(vec![Ok(ok(&track_list(true)))]);
    let page = search("owls", &[ItemKind::Track], Some("25"), fake.fetch()).unwrap();
    assert_eq!(
        fake.asked,
        ["https://api.deezer.com/search?q=owls&index=25&limit=25"]
    );
    assert_eq!(page.items, vec![Item::Track(fixtures::track())]);
    assert_eq!(page.next.as_deref(), Some("26"));
}

#[test]
fn a_search_of_several_kinds_asks_the_autocomplete() {
    let answer = format!(
        r#"{{"tracks":{},"albums":{{"data":[]}},"artists":{{"data":[]}},"playlists":{{"data":[]}}}}"#,
        track_list(false)
    );
    let mut fake = Fake::new(vec![Ok(ok(&answer))]);
    let page = search("owls", &[], None, fake.fetch()).unwrap();
    assert_eq!(
        fake.asked,
        ["https://api.deezer.com/search/autocomplete?q=owls&index=0&limit=25"]
    );
    assert_eq!(page.items, vec![Item::Track(fixtures::track())]);
    assert_eq!(page.next, None);
}

#[test]
fn a_search_refuses_a_bad_cursor_or_a_blank_query_before_any_request() {
    assert!(matches!(
        search("owls", &[], Some("x"), never),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        search(" ", &[ItemKind::Album], None, never),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(suggest("", never), Err(Error::InvalidInput(_))));
}

#[test]
fn a_failed_request_is_the_calls_error() {
    let mut fake = Fake::new(vec![Err(Error::Upstream(String::from("timed out")))]);
    assert_eq!(
        search("owls", &[ItemKind::Track], None, fake.fetch()),
        Err(Error::Upstream(String::from("timed out")))
    );
    let mut fake = Fake::new(vec![Err(Error::RateLimited(None))]);
    assert_eq!(suggest("owls", fake.fetch()), Err(Error::RateLimited(None)));
}

#[test]
fn suggestions_ask_the_autocomplete() {
    let answer = format!(
        r#"{{"tracks":{},"albums":{{"data":[]}},"artists":{{"data":[]}},"playlists":{{"data":[]}}}}"#,
        track_list(false)
    );
    let mut fake = Fake::new(vec![Ok(ok(&answer))]);
    assert_eq!(
        suggest("nig", fake.fetch()),
        Ok(vec![String::from("Night Train")])
    );
    assert_eq!(
        fake.asked,
        ["https://api.deezer.com/search/autocomplete?q=nig&index=0&limit=25"]
    );
}

#[test]
fn an_album_page_asks_the_album_then_its_tracks() {
    let tracks = fixtures::list(
        &[String::from(
            r#"{"id":101,"title":"Night Train (Live)","duration":201,"isrc":"QZABC2600001","artist":{"id":7,"name":"The Owls"}}"#,
        )],
        false,
    );
    let mut fake = Fake::new(vec![Ok(ok(&fixtures::album_json())), Ok(ok(&tracks))]);
    let page = album("55", fake.fetch()).unwrap();
    assert_eq!(
        fake.asked,
        [
            "https://api.deezer.com/album/55?limit=1",
            "https://api.deezer.com/album/55/tracks?limit=1000"
        ]
    );
    assert_eq!(page.album.title, "Lights");
    assert_eq!(page.tracks.len(), 1);
}

#[test]
fn an_album_that_is_not_found_asks_nothing_more() {
    let mut fake = Fake::new(vec![Ok(ok(NO_DATA))]);
    assert_eq!(album("55", fake.fetch()), Err(Error::NotFound));
    assert_eq!(fake.asked.len(), 1);
    assert!(matches!(album("x", never), Err(Error::InvalidInput(_))));
}

#[test]
fn an_artist_page_asks_the_artist_top_tracks_and_albums() {
    let albums = fixtures::list(
        &[format!(
            r#"{{"id":55,"title":"Lights","release_date":"2019-04-12",{}}}"#,
            fixtures::COVERS
        )],
        false,
    );
    let mut fake = Fake::new(vec![
        Ok(ok(&fixtures::artist_json())),
        Ok(ok(&track_list(false))),
        Ok(ok(&albums)),
    ]);
    let page = artist("7", fake.fetch()).unwrap();
    assert_eq!(
        fake.asked,
        [
            "https://api.deezer.com/artist/7",
            "https://api.deezer.com/artist/7/top?limit=10",
            "https://api.deezer.com/artist/7/albums?limit=50"
        ]
    );
    assert_eq!(page.artist.name, "The Owls");
    assert_eq!(page.top_tracks, vec![fixtures::track()]);
    assert_eq!(page.albums.len(), 1);
}

#[test]
fn an_artist_page_stops_at_the_first_failed_answer() {
    let mut fake = Fake::new(vec![Ok(ok(NO_DATA))]);
    assert_eq!(artist("7", fake.fetch()), Err(Error::NotFound));
    assert_eq!(fake.asked.len(), 1);

    let mut fake = Fake::new(vec![
        Ok(ok(&fixtures::artist_json())),
        Err(Error::RateLimited(None)),
    ]);
    assert_eq!(artist("7", fake.fetch()), Err(Error::RateLimited(None)));
    assert_eq!(fake.asked.len(), 2);

    let mut fake = Fake::new(vec![
        Ok(ok(&fixtures::artist_json())),
        Ok(ok(r#"{"error":{"type":"x","message":"y","code":4}}"#)),
    ]);
    assert!(matches!(artist("7", fake.fetch()), Err(Error::Upstream(_))));
    assert_eq!(fake.asked.len(), 2);
    assert!(matches!(
        artist("7/top", never),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn a_playlist_page_asks_from_the_cursor() {
    let answer = fixtures::playlist_json(300, &[fixtures::track_json()]);
    let mut fake = Fake::new(vec![Ok(ok(&answer))]);
    let page = playlist("900", Some("100"), fake.fetch()).unwrap();
    assert_eq!(
        fake.asked,
        ["https://api.deezer.com/playlist/900?index=100&limit=100"]
    );
    assert_eq!(page.playlist.title, "Road Mix");
    assert_eq!(page.next.as_deref(), Some("101"));
    assert!(matches!(
        playlist("900", Some("-1"), never),
        Err(Error::InvalidInput(_))
    ));
}

fn wanted(title: &str, artists: &[&str]) -> Wanted {
    Wanted {
        title: String::from(title),
        artists: artists.iter().map(|a| String::from(*a)).collect(),
    }
}

#[test]
fn a_tracks_artwork_is_asked_by_its_isrc_when_it_has_a_valid_one() {
    let mut fake = Fake::new(vec![Ok(ok(&fixtures::track_json()))]);
    let of = ArtworkOf::Track {
        wanted: wanted("Night Train", &["The Owls"]),
        isrc: Some(String::from("QZABC2600001")),
    };
    assert_eq!(artwork(&of, fake.fetch()), Ok(fixtures::covers()));
    assert_eq!(
        fake.asked,
        ["https://api.deezer.com/track/isrc:QZABC2600001"]
    );
}

#[test]
fn a_tracks_artwork_without_a_valid_isrc_is_searched_by_artist_and_title() {
    for isrc in [None, Some(String::from("not an isrc"))] {
        let mut fake = Fake::new(vec![Ok(ok(&track_list(false)))]);
        let of = ArtworkOf::Track {
            wanted: wanted("Night Train (Live)", &["The Owls", "Mae"]),
            isrc,
        };
        assert_eq!(artwork(&of, fake.fetch()), Ok(fixtures::covers()));
        assert_eq!(
            fake.asked,
            [
                "https://api.deezer.com/search?q=The%20Owls%20Night%20Train%20%28Live%29&index=0&limit=25"
            ]
        );
    }
}

#[test]
fn an_albums_artwork_is_searched_by_artist_and_title_and_an_artists_by_name() {
    let albums = fixtures::list(
        &[format!(
            r#"{{"id":55,"title":"Lights",{},"artist":{{"id":7,"name":"The Owls"}}}}"#,
            fixtures::COVERS
        )],
        false,
    );
    let mut fake = Fake::new(vec![Ok(ok(&albums))]);
    let of = ArtworkOf::Album(wanted("Lights", &["The Owls"]));
    assert_eq!(artwork(&of, fake.fetch()), Ok(fixtures::covers()));
    assert_eq!(
        fake.asked,
        ["https://api.deezer.com/search/album?q=The%20Owls%20Lights&index=0&limit=25"]
    );

    // An artist is searched by name alone, even if names of artists come with it.
    let artists = fixtures::list(&[fixtures::artist_json()], false);
    let mut fake = Fake::new(vec![Ok(ok(&artists))]);
    let of = ArtworkOf::Artist(wanted("The Owls", &["Mae"]));
    assert_eq!(artwork(&of, fake.fetch()), Ok(fixtures::pictures()));
    assert_eq!(
        fake.asked,
        ["https://api.deezer.com/search/artist?q=The%20Owls&index=0&limit=25"]
    );
}

#[test]
fn artwork_refuses_a_playlist_and_an_item_it_cannot_look_up_before_any_request() {
    assert_eq!(
        artwork(&ArtworkOf::Playlist, never),
        Err(Error::Unsupported)
    );
    for of in [
        ArtworkOf::Track {
            wanted: wanted(" ", &["The Owls"]),
            isrc: None,
        },
        ArtworkOf::Track {
            wanted: wanted("Night Train", &[]),
            isrc: Some(String::from("short")),
        },
        ArtworkOf::Album(wanted("Lights", &[])),
        ArtworkOf::Album(wanted("", &["The Owls"])),
        ArtworkOf::Artist(wanted("  ", &[])),
    ] {
        assert!(
            matches!(artwork(&of, never), Err(Error::InvalidInput(_))),
            "{of:?}"
        );
    }
}

#[test]
fn a_link_is_imported_by_asking_its_item() {
    let mut fake = Fake::new(vec![Ok(ok(&fixtures::track_json()))]);
    assert_eq!(
        import_link("https://www.deezer.com/fr/track/101", fake.fetch()),
        Ok(Item::Track(fixtures::track()))
    );
    assert_eq!(fake.asked, ["https://api.deezer.com/track/101"]);

    let mut fake = Fake::new(vec![Ok(ok(NO_DATA))]);
    assert_eq!(
        import_link("https://www.deezer.com/album/55", fake.fetch()),
        Err(Error::NotFound)
    );
    assert_eq!(fake.asked, ["https://api.deezer.com/album/55?limit=1"]);
}

#[test]
fn a_link_that_is_not_deezers_or_a_short_link_asks_nothing() {
    assert!(matches!(
        import_link("https://example.org/track/1", never),
        Err(Error::InvalidInput(_))
    ));
    assert_eq!(
        import_link("https://link.deezer.com/s/abc", never),
        Err(Error::Unsupported)
    );
}
