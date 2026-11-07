use super::*;
use crate::fixtures::{
    COVERS, NO_DATA, PICTURES, album_json, artist_json, artist_ref, covers, list, ok, pictures,
    playlist_json, track, track_json,
};

fn upstream<T: std::fmt::Debug>(result: Result<T, Error>) -> String {
    match result {
        Err(Error::Upstream(message)) => message,
        other => panic!("expected an upstream error, got {other:?}"),
    }
}

fn autocomplete(next: bool) -> String {
    let next = if next {
        r#","next":"https://api.deezer.com/search/autocomplete?q=x&index=25""#
    } else {
        ""
    };
    format!(
        r#"{{"tracks":{{"data":[{{"id":1,"title":"T","title_short":"Ts","duration":3,"artist":{{"id":7,"name":"The Owls"}},"album":{{"id":55,"title":"Lights",{COVERS}}}}}]}},"albums":{{"data":[{{"id":55,"title":"Lights","nb_tracks":2,{COVERS},"artist":{{"id":7,"name":"The Owls"}}}}]}},"artists":{{"data":[{{"id":7,"name":"The Owls",{PICTURES}}}]}},"playlists":{{"data":[{{"id":900,"title":"Road Mix","nb_tracks":3,{PICTURES},"creator":{{"id":3,"name":"Ana"}}}}]}},"podcasts":{{"data":[]}},"radios":{{"data":[]}},"total":4{next}}}"#
    )
}

#[test]
fn check_accepts_a_json_200_without_error() {
    assert_eq!(check(&ok("{}")), Ok(()));
    assert_eq!(check(&ok("[]")), Ok(()));
    assert_eq!(check(&ok("not json")), Ok(()));
}

#[test]
fn check_reads_no_data_as_not_found_and_any_other_error_as_upstream() {
    assert_eq!(check(&ok(NO_DATA)), Err(Error::NotFound));
    assert_eq!(
        upstream(check(&ok(
            r#"{"error":{"type":"ParameterException","message":"empty parameter","code":500}}"#
        ))),
        "Deezer answered with error 500: empty parameter (ParameterException)"
    );
    assert_eq!(
        upstream(check(&ok(r#"{"error":{}}"#))),
        "Deezer answered with error without code:  ()"
    );
}

#[test]
fn check_reads_429_as_rate_limited_with_its_delay() {
    let response = |value: &str| {
        Response::new(
            429,
            vec![(String::from("retry-after"), String::from(value))],
            Vec::new(),
        )
    };
    assert_eq!(
        check(&response("4")),
        Err(Error::RateLimited(Some(Duration::from_secs(4))))
    );
    assert_eq!(check(&response("soon")), Err(Error::RateLimited(None)));
    assert_eq!(
        check(&response("18446744073709552")),
        Err(Error::RateLimited(None))
    );
    assert_eq!(
        check(&Response::new(429, Vec::new(), Vec::new())),
        Err(Error::RateLimited(None))
    );
}

#[test]
fn check_reads_any_other_status_as_upstream() {
    for status in [0, 201, 204, 301, 302, 400, 403, 404, 500, 503] {
        let mut response = ok("{}");
        response.status = status;
        assert_eq!(
            upstream(check(&response)),
            format!("Deezer answered with status {status}")
        );
    }
}

#[test]
fn check_refuses_an_answer_that_is_not_json() {
    let html = Response::new(
        200,
        vec![(String::from("content-type"), String::from("text/html"))],
        b"{}".to_vec(),
    );
    assert_eq!(
        upstream(check(&html)),
        "Deezer answered with Content-Type \"text/html\", not JSON"
    );
    let none = Response::new(200, Vec::new(), b"{}".to_vec());
    assert_eq!(
        upstream(check(&none)),
        "Deezer answered with Content-Type \"\", not JSON"
    );
    assert!(is_json("APPLICATION/JSON"));
    assert!(!is_json("application/jsonx"));
}

#[test]
fn reads_a_track_search_with_its_next_cursor() {
    let page = read_search(ItemKind::Track, 25, &ok(&list(&[track_json()], true))).unwrap();
    assert_eq!(page.items, vec![Item::Track(track())]);
    assert_eq!(page.next, Some(String::from("26")));
}

#[test]
fn a_search_without_next_or_without_results_has_no_next_page() {
    let last = read_search(ItemKind::Track, 0, &ok(&list(&[track_json()], false))).unwrap();
    assert_eq!(last.next, None);
    let empty = read_search(ItemKind::Album, 0, &ok(&list(&[], true))).unwrap();
    assert_eq!(empty, Page::default());
    let at_the_end =
        read_search(ItemKind::Track, u32::MAX, &ok(&list(&[track_json()], true))).unwrap();
    assert_eq!(at_the_end.next, None);
}

#[test]
fn reads_album_artist_and_playlist_searches() {
    let albums = read_search(ItemKind::Album, 0, &ok(&list(&[album_json()], false))).unwrap();
    assert_eq!(
        albums.items,
        vec![Item::Album(Album {
            id: String::from("55"),
            title: String::from("Lights"),
            artists: vec![artist_ref("7", "The Owls")],
            year: Some(2019),
            artwork: covers(),
        })]
    );
    let artists = read_search(ItemKind::Artist, 0, &ok(&list(&[artist_json()], false))).unwrap();
    assert_eq!(
        artists.items,
        vec![Item::Artist(Artist {
            id: String::from("7"),
            name: String::from("The Owls"),
            artwork: pictures(),
        })]
    );
    let playlist =
        format!(r#"{{"id":900,"title":"Road Mix",{PICTURES},"user":{{"id":3,"name":"Ana"}}}}"#);
    let playlists = read_search(ItemKind::Playlist, 0, &ok(&list(&[playlist], true))).unwrap();
    assert_eq!(
        playlists.items,
        vec![Item::Playlist(Playlist {
            id: String::from("900"),
            title: String::from("Road Mix"),
            owner: Some(String::from("Ana")),
            artwork: pictures(),
        })]
    );
    assert_eq!(playlists.next, Some(String::from("1")));
}

#[test]
fn a_search_answer_of_another_shape_is_upstream() {
    for (kind, body) in [
        (
            ItemKind::Track,
            r#"{"data":[{"id":"1","title":"x","artist":{"id":1,"name":"a"}}]}"#,
        ),
        (ItemKind::Track, r#"{"data":[{"id":1,"title":"x"}]}"#),
        (ItemKind::Track, r#"{"total":1}"#),
        (ItemKind::Album, r#"{"data":[{"id":1}]}"#),
        (ItemKind::Artist, r#"{"data":[{"id":-1,"name":"a"}]}"#),
        (ItemKind::Playlist, r#"{"data":null}"#),
        (ItemKind::Playlist, "[]"),
        (ItemKind::Track, "not json"),
    ] {
        let message = upstream(read_search(kind, 0, &ok(body)));
        assert!(message.starts_with("Deezer's "), "{body}: {message}");
    }
}

#[test]
fn an_error_object_wins_over_the_shape() {
    assert_eq!(
        read_search(ItemKind::Track, 0, &ok(NO_DATA)),
        Err(Error::NotFound)
    );
}

#[test]
fn optional_fields_that_are_missing_empty_or_not_https_are_left_out() {
    let bare = r#"{"id":1,"title":"T","artist":{"id":2,"name":"A"},"isrc":"","contributors":[],"album":{"id":3,"title":"B","cover_small":"http://img.example/56.jpg","cover_xl":null}}"#;
    let page = read_search(ItemKind::Track, 0, &ok(&list(&[String::from(bare)], false))).unwrap();
    assert_eq!(
        page.items,
        vec![Item::Track(Track {
            id: String::from("1"),
            title: String::from("T"),
            artists: vec![artist_ref("2", "A")],
            album: Some(AlbumRef {
                id: String::from("3"),
                title: String::from("B"),
            }),
            duration: None,
            isrc: None,
            artwork: Vec::new(),
        })]
    );
}

#[test]
fn a_release_year_is_read_only_from_four_digits_that_are_not_zero() {
    assert_eq!(year(Some("2001-03-07")), Some(2001));
    assert_eq!(year(Some("1999")), Some(1999));
    assert_eq!(year(Some("0000-00-00")), None);
    assert_eq!(year(Some("19")), None);
    assert_eq!(year(Some("+999-01-01")), None);
    assert_eq!(year(Some("99999")), Some(9999));
    assert_eq!(year(Some("é999")), None);
    assert_eq!(year(None), None);
}

#[test]
fn reads_the_kinds_asked_from_the_autocomplete_in_their_order() {
    let page = read_autocomplete(
        &[ItemKind::Playlist, ItemKind::Track],
        0,
        &ok(&autocomplete(true)),
    )
    .unwrap();
    let kinds: Vec<&str> = page
        .items
        .iter()
        .map(|item| match item {
            Item::Track(_) => "track",
            Item::Album(_) => "album",
            Item::Artist(_) => "artist",
            Item::Playlist(_) => "playlist",
        })
        .collect();
    assert_eq!(kinds, ["playlist", "track"]);
    assert_eq!(page.next, Some(String::from("25")));
    let all = read_autocomplete(
        &[
            ItemKind::Track,
            ItemKind::Album,
            ItemKind::Artist,
            ItemKind::Playlist,
            ItemKind::Track,
        ],
        50,
        &ok(&autocomplete(false)),
    )
    .unwrap();
    assert_eq!(all.items.len(), 4);
    assert_eq!(all.next, None);
}

#[test]
fn an_empty_autocomplete_has_no_next_page() {
    let empty = r#"{"tracks":{"data":[]},"albums":{"data":[]},"artists":{"data":[]},"playlists":{"data":[]},"next":"x"}"#;
    assert_eq!(
        read_autocomplete(&[ItemKind::Track], 0, &ok(empty)).unwrap(),
        Page::default()
    );
}

#[test]
fn an_autocomplete_missing_a_kind_is_upstream() {
    let body = r#"{"tracks":{"data":[]},"albums":{"data":[]},"artists":{"data":[]}}"#;
    assert!(
        upstream(read_autocomplete(&[ItemKind::Track], 0, &ok(body)))
            .starts_with("Deezer's autocomplete")
    );
}

#[test]
fn suggestions_are_artists_then_tracks_then_albums_each_once() {
    let body = r#"{"tracks":{"data":[{"id":1,"title":"Owls (Live)","title_short":"Owls","artist":{"id":7,"name":"x"}},{"id":2,"title":"Night","artist":{"id":7,"name":"x"}}]},"albums":{"data":[{"id":3,"title":"night"},{"id":4,"title":"  "}]},"artists":{"data":[{"id":7,"name":" The Owls "},{"id":8,"name":"the owls"}]},"playlists":{"data":[]}}"#;
    assert_eq!(
        read_suggestions(&ok(body)).unwrap(),
        vec![
            String::from("The Owls"),
            String::from("Owls"),
            String::from("Night")
        ]
    );
}

#[test]
fn suggestions_stop_at_the_most_returned() {
    let artists: Vec<String> = (0..15)
        .map(|i| format!(r#"{{"id":{i},"name":"Artist {i}"}}"#))
        .collect();
    let body = format!(
        r#"{{"tracks":{{"data":[]}},"albums":{{"data":[]}},"artists":{{"data":[{}]}},"playlists":{{"data":[]}}}}"#,
        artists.join(",")
    );
    let suggestions = read_suggestions(&ok(&body)).unwrap();
    assert_eq!(suggestions.len(), MAX_SUGGESTIONS);
    assert_eq!(suggestions.last().map(String::as_str), Some("Artist 9"));
}

#[test]
fn reads_an_album_page_and_gives_each_track_the_album() {
    let tracks = list(
        &[String::from(
            r#"{"id":101,"title":"Night Train (Live)","isrc":"QZABC2600001","duration":201,"artist":{"id":7,"name":"The Owls"}}"#,
        )],
        false,
    );
    let page = read_album(&ok(&album_json()), &ok(&tracks)).unwrap();
    assert_eq!(page.album.id, "55");
    assert_eq!(page.album.year, Some(2019));
    let mut expected = track();
    expected.artists.truncate(1);
    assert_eq!(page.tracks, vec![expected]);
}

#[test]
fn an_album_with_more_tracks_than_one_answer_is_refused_not_cut() {
    let message =
        format!("the album has more than {ALBUM_TRACKS_LIMIT} tracks, more than one answer holds");
    assert_eq!(
        upstream(read_album(&ok(&album_json()), &ok(&list(&[], true)))),
        message
    );
    let big = album_json().replace(r#""nb_tracks":2"#, r#""nb_tracks":1001"#);
    assert_eq!(
        upstream(read_album(&ok(&big), &ok(&list(&[], false)))),
        message
    );
    let full = album_json().replace(r#""nb_tracks":2"#, r#""nb_tracks":1000"#);
    assert!(read_album(&ok(&full), &ok(&list(&[], false))).is_ok());
}

#[test]
fn an_album_page_fails_on_either_answer() {
    assert_eq!(
        read_album(&ok(NO_DATA), &ok(&list(&[], false))),
        Err(Error::NotFound)
    );
    assert_eq!(
        read_album(&ok(&album_json()), &ok(NO_DATA)),
        Err(Error::NotFound)
    );
    assert!(
        upstream(read_album(&ok(&album_json()), &ok("{}"))).starts_with("Deezer's album's tracks")
    );
}

#[test]
fn reads_an_artist_page_and_gives_their_albums_the_artist() {
    let albums = list(
        &[format!(
            r#"{{"id":56,"title":"Dark",{COVERS},"release_date":"2021-01-01"}}"#
        )],
        true,
    );
    let page = read_artist(
        &ok(&artist_json()),
        &ok(&list(&[track_json()], true)),
        &ok(&albums),
    )
    .unwrap();
    assert_eq!(page.artist.name, "The Owls");
    assert_eq!(page.artist.artwork, pictures());
    assert_eq!(page.top_tracks, vec![track()]);
    assert_eq!(
        page.albums,
        vec![Album {
            id: String::from("56"),
            title: String::from("Dark"),
            artists: vec![artist_ref("7", "The Owls")],
            year: Some(2021),
            artwork: covers(),
        }]
    );
}

#[test]
fn an_artist_page_fails_on_any_answer() {
    let fine = ok(&list(&[], false));
    assert_eq!(
        read_artist(&ok(NO_DATA), &fine, &fine),
        Err(Error::NotFound)
    );
    assert_eq!(
        read_artist(&ok(&artist_json()), &ok(NO_DATA), &fine),
        Err(Error::NotFound)
    );
    assert_eq!(
        read_artist(&ok(&artist_json()), &fine, &ok(NO_DATA)),
        Err(Error::NotFound)
    );
}

#[test]
fn reads_a_playlist_page_with_its_next_cursor_while_tracks_remain() {
    let page = read_playlist(100, &ok(&playlist_json(250, &[track_json(), track_json()]))).unwrap();
    assert_eq!(
        page.playlist,
        Playlist {
            id: String::from("900"),
            title: String::from("Road Mix"),
            owner: Some(String::from("Ana")),
            artwork: pictures(),
        }
    );
    assert_eq!(page.tracks, vec![track(), track()]);
    assert_eq!(page.next, Some(String::from("102")));
    let last = read_playlist(248, &ok(&playlist_json(250, &[track_json(), track_json()]))).unwrap();
    assert_eq!(last.next, None);
    let empty = read_playlist(0, &ok(&playlist_json(250, &[]))).unwrap();
    assert_eq!(empty.next, None);
}

#[test]
fn a_playlist_without_tracks_or_count_is_read_as_such() {
    let no_tracks = format!(r#"{{"id":900,"title":"Road Mix",{PICTURES}}}"#);
    assert_eq!(
        upstream(read_playlist(0, &ok(&no_tracks))),
        "Deezer's playlist has no tracks field"
    );
    let no_count = playlist_json(1, &[track_json()]).replace(r#""nb_tracks":1,"#, "");
    assert_eq!(read_playlist(0, &ok(&no_count)).unwrap().next, None);
}

#[test]
fn reads_one_item_of_each_kind() {
    assert_eq!(
        read_item(ItemKind::Track, &ok(&track_json())),
        Ok(Item::Track(track()))
    );
    assert!(
        matches!(read_item(ItemKind::Album, &ok(&album_json())), Ok(Item::Album(a)) if a.id == "55")
    );
    assert!(
        matches!(read_item(ItemKind::Artist, &ok(&artist_json())), Ok(Item::Artist(a)) if a.id == "7")
    );
    assert!(
        matches!(read_item(ItemKind::Playlist, &ok(&playlist_json(1, &[]))), Ok(Item::Playlist(p)) if p.id == "900")
    );
    assert_eq!(
        read_item(ItemKind::Album, &ok(NO_DATA)),
        Err(Error::NotFound)
    );
    assert!(
        upstream(read_item(ItemKind::Artist, &ok(&track_json()))).starts_with("Deezer's artist")
    );
}

fn wanted(title: &str, artists: &[&str]) -> Wanted {
    Wanted {
        title: String::from(title),
        artists: artists.iter().map(|a| String::from(*a)).collect(),
    }
}

#[test]
fn artwork_search_takes_the_first_result_with_the_same_title_and_an_artist() {
    let other_title = track_json()
        .replace("Night Train (Live)", "Day Train")
        .replace(r#""title_short":"Night Train""#, r#""title_short":"Day""#);
    let other_artist = track_json()
        .replace("The Owls", "Crows")
        .replace(r#""name":"Mae""#, r#""name":"Jay""#);
    let other_covers = track_json().replace("img.example/c/", "img.example/d/");
    let answer = ok(&list(
        &[other_title, other_artist, track_json(), other_covers],
        false,
    ));
    assert_eq!(
        read_artwork_search(
            ItemKind::Track,
            &wanted(" night train ", &["x", "MAE"]),
            &answer
        ),
        Ok(covers())
    );
    assert_eq!(
        read_artwork_search(
            ItemKind::Track,
            &wanted("Night Train (Live)", &["The Owls"]),
            &answer
        ),
        Ok(covers())
    );
    assert_eq!(
        read_artwork_search(ItemKind::Track, &wanted("Night", &["The Owls"]), &answer),
        Err(Error::NotFound)
    );
    assert_eq!(
        read_artwork_search(
            ItemKind::Track,
            &wanted("Night Train", &["Nobody"]),
            &answer
        ),
        Err(Error::NotFound)
    );
}

#[test]
fn artwork_search_of_albums_and_artists() {
    let albums = ok(&list(
        &[album_json().replace("Lights", "Darks"), album_json()],
        false,
    ));
    assert_eq!(
        read_artwork_search(ItemKind::Album, &wanted("lights", &["the owls"]), &albums),
        Ok(covers())
    );
    assert_eq!(
        read_artwork_search(ItemKind::Album, &wanted("lights", &["crows"]), &albums),
        Err(Error::NotFound)
    );
    let artists = ok(&list(
        &[artist_json().replace("The Owls", "Owl"), artist_json()],
        false,
    ));
    assert_eq!(
        read_artwork_search(ItemKind::Artist, &wanted("THE OWLS", &[]), &artists),
        Ok(pictures())
    );
    assert_eq!(
        read_artwork_search(ItemKind::Artist, &wanted("Owls", &[]), &artists),
        Err(Error::NotFound)
    );
    assert_eq!(
        read_artwork_search(ItemKind::Playlist, &wanted("x", &[]), &artists),
        Err(Error::Unsupported)
    );
}

#[test]
fn a_match_without_artwork_is_not_found() {
    let bare = ok(&list(
        &[String::from(r#"{"id":7,"name":"The Owls"}"#)],
        false,
    ));
    assert_eq!(
        read_artwork_search(ItemKind::Artist, &wanted("The Owls", &[]), &bare),
        Err(Error::NotFound)
    );
}

#[test]
fn artwork_by_isrc_is_the_tracks_album_covers() {
    assert_eq!(read_artwork_by_isrc(&ok(&track_json())), Ok(covers()));
    assert_eq!(read_artwork_by_isrc(&ok(NO_DATA)), Err(Error::NotFound));
    let no_album = r#"{"id":1,"title":"T","artist":{"id":2,"name":"A"}}"#;
    assert_eq!(read_artwork_by_isrc(&ok(no_album)), Err(Error::NotFound));
    assert!(upstream(read_artwork_by_isrc(&ok("{}"))).starts_with("Deezer's track"));
}
