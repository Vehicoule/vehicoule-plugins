use super::*;
use crate::fixtures;

fn request() -> Request {
    Request {
        url: String::from("https://api.deezer.com/track/1"),
    }
}

#[test]
fn sends_a_get_with_no_header_and_no_body_and_keeps_the_answer() {
    let mut sent = None;
    let response = send(&request(), |r| {
        sent = Some(r.clone());
        Ok(http::Response {
            status: 429,
            headers: vec![(String::from("Retry-After"), String::from("3"))],
            body: b"{}".to_vec(),
        })
    })
    .unwrap();
    assert_eq!(
        sent,
        Some(http::Request {
            method: http::Method::Get,
            url: String::from("https://api.deezer.com/track/1"),
            headers: Vec::new(),
            body: None,
        })
    );
    assert_eq!(
        response,
        Response::new(
            429,
            vec![(String::from("Retry-After"), String::from("3"))],
            b"{}".to_vec()
        )
    );
}

#[test]
fn a_request_that_did_not_complete_is_upstream_and_says_why() {
    for (error, why) in [
        (http::HttpError::RefusedHost, "refused host"),
        (http::HttpError::RefusedScheme, "refused scheme"),
        (http::HttpError::BudgetExceeded, "request budget exceeded"),
        (http::HttpError::Timeout, "timed out"),
        (
            http::HttpError::Transport(String::from("reset")),
            "transport: reset",
        ),
    ] {
        assert_eq!(
            send(&request(), |_| Err(error)),
            Err(Error::Upstream(format!(
                "the request did not complete: {why}"
            )))
        );
    }
}

#[test]
fn kinds_map_one_to_one() {
    assert_eq!(kind(t::ItemKind::Track), ItemKind::Track);
    assert_eq!(kind(t::ItemKind::Album), ItemKind::Album);
    assert_eq!(kind(t::ItemKind::Artist), ItemKind::Artist);
    assert_eq!(kind(t::ItemKind::Playlist), ItemKind::Playlist);
}

fn t_artist_ref(name: &str) -> t::ArtistRef {
    t::ArtistRef {
        id: String::from("1"),
        name: String::from(name),
    }
}

#[test]
fn artwork_is_looked_up_by_names_and_a_tracks_isrc() {
    let track = t::Item::Track(t::Track {
        id: String::from("1"),
        title: String::from("Night Train"),
        artists: vec![t_artist_ref("The Owls"), t_artist_ref("Mae")],
        album: None,
        duration_ms: None,
        isrc: Some(String::from("QZABC2600001")),
        artwork: Vec::new(),
    });
    assert_eq!(
        artwork_of(&track),
        ArtworkOf::Track {
            wanted: Wanted {
                title: String::from("Night Train"),
                artists: vec![String::from("The Owls"), String::from("Mae")],
            },
            isrc: Some(String::from("QZABC2600001")),
        }
    );
    let album = t::Item::Album(t::Album {
        id: String::from("2"),
        title: String::from("Lights"),
        artists: vec![t_artist_ref("The Owls")],
        year: None,
        artwork: Vec::new(),
    });
    assert_eq!(
        artwork_of(&album),
        ArtworkOf::Album(Wanted {
            title: String::from("Lights"),
            artists: vec![String::from("The Owls")],
        })
    );
    let artist = t::Item::Artist(t::Artist {
        id: String::from("3"),
        name: String::from("The Owls"),
        artwork: Vec::new(),
    });
    assert_eq!(
        artwork_of(&artist),
        ArtworkOf::Artist(Wanted {
            title: String::from("The Owls"),
            artists: Vec::new(),
        })
    );
    let playlist = t::Item::Playlist(t::Playlist {
        id: String::from("4"),
        title: String::from("Mix"),
        owner: None,
        artwork: Vec::new(),
    });
    assert_eq!(artwork_of(&playlist), ArtworkOf::Playlist);
}

#[test]
fn errors_map_one_to_one_and_a_delay_becomes_milliseconds() {
    assert_eq!(error(Error::NotFound), t::Error::NotFound);
    assert_eq!(error(Error::Unsupported), t::Error::Unsupported);
    assert_eq!(
        error(Error::InvalidInput(String::from("a"))),
        t::Error::InvalidInput(String::from("a"))
    );
    assert_eq!(
        error(Error::Upstream(String::from("b"))),
        t::Error::Upstream(String::from("b"))
    );
    assert_eq!(error(Error::RateLimited(None)), t::Error::RateLimited(None));
    assert_eq!(
        error(Error::RateLimited(Some(Duration::from_secs(3)))),
        t::Error::RateLimited(Some(3000))
    );
    assert_eq!(
        error(Error::RateLimited(Some(Duration::MAX))),
        t::Error::RateLimited(None)
    );
}

#[test]
fn artwork_is_square() {
    assert_eq!(
        artwork(vec![Artwork {
            url: String::from("https://img.example/a.jpg"),
            size: 250,
        }]),
        vec![t::Artwork {
            url: String::from("https://img.example/a.jpg"),
            width: Some(250),
            height: Some(250),
        }]
    );
}

fn t_covers() -> Vec<t::Artwork> {
    artwork(fixtures::covers())
}

fn t_track() -> t::Track {
    t::Track {
        id: String::from("101"),
        title: String::from("Night Train (Live)"),
        artists: vec![
            t::ArtistRef {
                id: String::from("7"),
                name: String::from("The Owls"),
            },
            t::ArtistRef {
                id: String::from("8"),
                name: String::from("Mae"),
            },
        ],
        album: Some(t::AlbumRef {
            id: String::from("55"),
            title: String::from("Lights"),
        }),
        duration_ms: Some(201_000),
        isrc: Some(String::from("QZABC2600001")),
        artwork: t_covers(),
    }
}

#[test]
fn a_track_maps_field_for_field_with_its_duration_in_milliseconds() {
    assert_eq!(track(fixtures::track()), t_track());
    let mut long = fixtures::track();
    long.duration = Some(Duration::MAX);
    long.album = None;
    long.isrc = None;
    let long = track(long);
    assert_eq!(long.duration_ms, None);
    assert_eq!(long.album, None);
    assert_eq!(long.isrc, None);
}

fn lights() -> Album {
    Album {
        id: String::from("55"),
        title: String::from("Lights"),
        artists: vec![fixtures::artist_ref("7", "The Owls")],
        year: Some(2019),
        artwork: fixtures::covers(),
    }
}

fn t_lights() -> t::Album {
    t::Album {
        id: String::from("55"),
        title: String::from("Lights"),
        artists: vec![t::ArtistRef {
            id: String::from("7"),
            name: String::from("The Owls"),
        }],
        year: Some(2019),
        artwork: t_covers(),
    }
}

fn owls() -> Artist {
    Artist {
        id: String::from("7"),
        name: String::from("The Owls"),
        artwork: fixtures::pictures(),
    }
}

fn t_owls() -> t::Artist {
    t::Artist {
        id: String::from("7"),
        name: String::from("The Owls"),
        artwork: artwork(fixtures::pictures()),
    }
}

fn road_mix() -> Playlist {
    Playlist {
        id: String::from("900"),
        title: String::from("Road Mix"),
        owner: Some(String::from("Ana")),
        artwork: fixtures::pictures(),
    }
}

fn t_road_mix() -> t::Playlist {
    t::Playlist {
        id: String::from("900"),
        title: String::from("Road Mix"),
        owner: Some(String::from("Ana")),
        artwork: artwork(fixtures::pictures()),
    }
}

#[test]
fn items_of_each_kind_map_field_for_field() {
    assert_eq!(
        item(Item::Track(fixtures::track())),
        t::Item::Track(t_track())
    );
    assert_eq!(item(Item::Album(lights())), t::Item::Album(t_lights()));
    assert_eq!(item(Item::Artist(owls())), t::Item::Artist(t_owls()));
    assert_eq!(
        item(Item::Playlist(road_mix())),
        t::Item::Playlist(t_road_mix())
    );
}

#[test]
fn pages_map_field_for_field() {
    assert_eq!(
        page(Page {
            items: vec![Item::Artist(owls()), Item::Album(lights())],
            next: Some(String::from("25")),
        }),
        s::Page {
            items: vec![t::Item::Artist(t_owls()), t::Item::Album(t_lights())],
            next: Some(String::from("25")),
        }
    );
    assert_eq!(
        album_page(AlbumPage {
            album: lights(),
            tracks: vec![fixtures::track()],
        }),
        m::AlbumPage {
            album: t_lights(),
            tracks: vec![t_track()],
        }
    );
    assert_eq!(
        artist_page(ArtistPage {
            artist: owls(),
            top_tracks: vec![fixtures::track()],
            albums: vec![lights()],
        }),
        m::ArtistPage {
            artist: t_owls(),
            top_tracks: vec![t_track()],
            albums: vec![t_lights()],
        }
    );
    assert_eq!(
        playlist_page(PlaylistPage {
            playlist: road_mix(),
            tracks: vec![fixtures::track()],
            next: None,
        }),
        m::PlaylistPage {
            playlist: t_road_mix(),
            tracks: vec![t_track()],
            next: None,
        }
    );
}
