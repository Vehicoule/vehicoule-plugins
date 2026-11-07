use std::fmt::Write as _;

use super::*;

const ALL: [ItemKind; 4] = [
    ItemKind::Track,
    ItemKind::Album,
    ItemKind::Artist,
    ItemKind::Playlist,
];

#[test]
fn one_kind_is_searched_on_its_own_endpoint() {
    assert_eq!(
        search_plan(&[ItemKind::Track]),
        SearchPlan::One(ItemKind::Track)
    );
    assert_eq!(
        search_plan(&[ItemKind::Playlist, ItemKind::Playlist]),
        SearchPlan::One(ItemKind::Playlist)
    );
}

#[test]
fn several_kinds_keep_their_order_without_repeats_and_none_means_all() {
    assert_eq!(
        search_plan(&[ItemKind::Artist, ItemKind::Track, ItemKind::Artist]),
        SearchPlan::Several(vec![ItemKind::Artist, ItemKind::Track])
    );
    assert_eq!(search_plan(&[]), SearchPlan::Several(ALL.to_vec()));
}

#[test]
fn each_plan_has_its_endpoint_and_the_query_is_trimmed_and_encoded() {
    let url = |plan: SearchPlan| search("  Mae & Co ", &plan, 50).unwrap().url;
    assert_eq!(
        url(SearchPlan::One(ItemKind::Track)),
        "https://api.deezer.com/search?q=Mae%20%26%20Co&index=50&limit=25"
    );
    assert_eq!(
        url(SearchPlan::One(ItemKind::Album)),
        "https://api.deezer.com/search/album?q=Mae%20%26%20Co&index=50&limit=25"
    );
    assert_eq!(
        url(SearchPlan::One(ItemKind::Artist)),
        "https://api.deezer.com/search/artist?q=Mae%20%26%20Co&index=50&limit=25"
    );
    assert_eq!(
        url(SearchPlan::One(ItemKind::Playlist)),
        "https://api.deezer.com/search/playlist?q=Mae%20%26%20Co&index=50&limit=25"
    );
    assert_eq!(
        url(SearchPlan::Several(ALL.to_vec())),
        "https://api.deezer.com/search/autocomplete?q=Mae%20%26%20Co&index=50&limit=25"
    );
}

#[test]
fn a_query_cannot_add_a_parameter_or_a_path() {
    let request = search(
        "x&index=9/../track/1?#",
        &SearchPlan::One(ItemKind::Track),
        0,
    )
    .unwrap();
    assert_eq!(
        request.url,
        "https://api.deezer.com/search?q=x%26index%3D9%2F..%2Ftrack%2F1%3F%23&index=0&limit=25"
    );
}

#[test]
fn a_blank_query_or_prefix_is_refused() {
    for blank in ["", " ", "\t\n"] {
        assert!(matches!(
            search(blank, &SearchPlan::One(ItemKind::Track), 0),
            Err(Error::InvalidInput(_))
        ));
        assert!(matches!(suggest(blank), Err(Error::InvalidInput(_))));
    }
}

#[test]
fn suggestions_are_the_first_autocomplete_page() {
    assert_eq!(
        suggest("dafT").unwrap().url,
        "https://api.deezer.com/search/autocomplete?q=dafT&index=0&limit=25"
    );
}

#[test]
fn an_album_page_asks_the_album_then_all_its_tracks() {
    let [album, tracks] = album("302127").unwrap();
    assert_eq!(album.url, "https://api.deezer.com/album/302127?limit=1");
    assert_eq!(
        tracks.url,
        "https://api.deezer.com/album/302127/tracks?limit=1000"
    );
}

#[test]
fn an_artist_page_asks_the_artist_their_top_tracks_and_albums() {
    let [artist, top, albums] = artist("27").unwrap();
    assert_eq!(artist.url, "https://api.deezer.com/artist/27");
    assert_eq!(top.url, "https://api.deezer.com/artist/27/top?limit=10");
    assert_eq!(
        albums.url,
        "https://api.deezer.com/artist/27/albums?limit=50"
    );
}

#[test]
fn a_playlist_page_asks_its_tracks_from_the_index() {
    assert_eq!(
        playlist("908622995", 200).unwrap().url,
        "https://api.deezer.com/playlist/908622995?index=200&limit=100"
    );
}

#[test]
fn an_item_is_asked_by_its_kind() {
    let url = |kind| item(kind, "5").unwrap().url;
    assert_eq!(url(ItemKind::Track), "https://api.deezer.com/track/5");
    assert_eq!(
        url(ItemKind::Album),
        "https://api.deezer.com/album/5?limit=1"
    );
    assert_eq!(url(ItemKind::Artist), "https://api.deezer.com/artist/5");
    assert_eq!(
        url(ItemKind::Playlist),
        "https://api.deezer.com/playlist/5?limit=1"
    );
}

#[test]
fn every_request_with_an_id_refuses_one_that_is_not_deezers() {
    for bad in [
        "",
        "x",
        "1/tracks",
        "-1",
        " 1",
        "1 ",
        "+1",
        "18446744073709551616",
    ] {
        assert!(matches!(album(bad), Err(Error::InvalidInput(_))), "{bad:?}");
        assert!(
            matches!(artist(bad), Err(Error::InvalidInput(_))),
            "{bad:?}"
        );
        assert!(
            matches!(playlist(bad, 0), Err(Error::InvalidInput(_))),
            "{bad:?}"
        );
        assert!(
            matches!(item(ItemKind::Track, bad), Err(Error::InvalidInput(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn an_id_is_decimal_digits_that_fit_a_u64() {
    assert_eq!(id("0"), Ok(0));
    assert_eq!(id("007"), Ok(7));
    assert_eq!(id("18446744073709551615"), Ok(u64::MAX));
    assert!(matches!(
        id("18446744073709551616"),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(id("١"), Err(Error::InvalidInput(_))));
}

#[test]
fn a_cursor_is_a_decimal_index_and_none_is_the_first_page() {
    assert_eq!(cursor(None), Ok(0));
    assert_eq!(cursor(Some("0")), Ok(0));
    assert_eq!(cursor(Some("125")), Ok(125));
    assert_eq!(cursor(Some("4294967295")), Ok(u32::MAX));
    for bad in ["", "x", "-1", "1.5", " 1", "4294967296"] {
        assert!(
            matches!(cursor(Some(bad)), Err(Error::InvalidInput(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn an_isrc_is_twelve_ascii_letters_and_digits() {
    assert!(is_isrc("QZABC2600001"));
    assert!(is_isrc("qzabc2600001"));
    for bad in [
        "",
        "QZABC260000",
        "QZABC26000012",
        "QZ-BC2600001",
        "QZABC260000é",
        "QZABC260000 ",
    ] {
        assert!(!is_isrc(bad), "{bad:?}");
    }
}

#[test]
fn a_track_is_asked_by_its_isrc() {
    assert_eq!(
        track_by_isrc("QZABC2600001").url,
        "https://api.deezer.com/track/isrc:QZABC2600001"
    );
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
