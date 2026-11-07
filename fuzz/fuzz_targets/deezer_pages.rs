//! Deezer's answers to the requests of an album page or an artist page, as `read_album` and `read_artist` read them:
//! any body for each answer. Asserts no panic, that an album page never holds more tracks than one answer may, and
//! that every track of an album page names that album.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vehicoule_plugin_deezer::read::{Response, read_album, read_artist};
use vehicoule_plugin_deezer::request::ALBUM_TRACKS_LIMIT;

fn answer(body: &[u8]) -> Response {
    Response::new(
        200,
        vec![(
            String::from("Content-Type"),
            String::from("application/json; charset=utf-8"),
        )],
        body.to_vec(),
    )
}

fuzz_target!(|input: (bool, &[u8], &[u8], &[u8])| {
    let (album, first, second, third) = input;
    if album {
        if let Ok(page) = read_album(&answer(first), &answer(second)) {
            assert!(page.tracks.len() <= ALBUM_TRACKS_LIMIT as usize);
            for track in &page.tracks {
                let named = track.album.as_ref().expect("a track of the album names it");
                assert_eq!(named.id, page.album.id);
                assert_eq!(named.title, page.album.title);
            }
        }
    } else if let Ok(page) = read_artist(&answer(first), &answer(second), &answer(third)) {
        assert!(page.artist.id.parse::<u64>().is_ok());
    }
});
