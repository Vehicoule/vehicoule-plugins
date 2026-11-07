//! Deezer's answer to one request, as each reader of one answer reads it: any status, any `Content-Type`, any body,
//! and for an artwork search any names to look for, which come from another plugin. Asserts no panic, and that what
//! is read holds only what the plugin may hand out: decimal ids, `https` artwork of Deezer's four sizes, cursors past
//! the page asked, and suggestions each once and at most `MAX_SUGGESTIONS`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vehicoule_plugin_deezer::model::{Artwork, Item, ItemKind, Track};
use vehicoule_plugin_deezer::read::{
    MAX_SUGGESTIONS, Response, Wanted, read_artwork_by_isrc, read_artwork_search,
    read_autocomplete, read_item, read_playlist, read_search, read_suggestions,
};

const KINDS: [ItemKind; 4] = [
    ItemKind::Track,
    ItemKind::Album,
    ItemKind::Artist,
    ItemKind::Playlist,
];

fn check_artwork(artwork: &[Artwork]) {
    for a in artwork {
        assert!(a.url.starts_with("https://"));
        assert!([56, 250, 500, 1000].contains(&a.size));
    }
}

fn check_id(id: &str) {
    assert!(id.parse::<u64>().is_ok(), "{id:?}");
}

fn check_track(track: &Track) {
    check_id(&track.id);
    track.artists.iter().for_each(|a| check_id(&a.id));
    if let Some(album) = &track.album {
        check_id(&album.id);
    }
    check_artwork(&track.artwork);
}

fn check_item(item: &Item) {
    match item {
        Item::Track(track) => check_track(track),
        Item::Album(album) => {
            check_id(&album.id);
            album.artists.iter().for_each(|a| check_id(&a.id));
            check_artwork(&album.artwork);
        }
        Item::Artist(artist) => {
            check_id(&artist.id);
            check_artwork(&artist.artwork);
        }
        Item::Playlist(playlist) => {
            check_id(&playlist.id);
            check_artwork(&playlist.artwork);
        }
        _ => {}
    }
}

fn check_next(index: u32, next: Option<&String>) {
    if let Some(next) = next {
        assert!(next.parse::<u32>().expect("a decimal cursor") > index);
    }
}

fuzz_target!(|input: (u8, u8, u8, u32, &str, Vec<&str>, &[u8])| {
    let (reader, status, content_type, index, title, artists, body) = input;
    let status = match status % 4 {
        0 | 1 => 200,
        2 => 429,
        _ => u16::from(status).saturating_mul(3),
    };
    let content_type = match content_type % 3 {
        0 | 1 => "application/json; charset=utf-8",
        _ => "text/html",
    };
    let response = Response::new(
        status,
        vec![
            (String::from("Content-Type"), String::from(content_type)),
            (String::from("Retry-After"), index.to_string()),
        ],
        body.to_vec(),
    );
    let kind = KINDS[usize::from(reader / 16) % KINDS.len()];
    match reader % 8 {
        0 => {
            if let Ok(page) = read_search(kind, index, &response) {
                page.items.iter().for_each(check_item);
                check_next(index, page.next.as_ref());
            }
        }
        1 => {
            // The kinds asked, from the bits of the selector: none, some, or all of them.
            let kinds: Vec<ItemKind> = KINDS
                .into_iter()
                .enumerate()
                .filter(|(i, _)| reader & (16 << i) != 0)
                .map(|(_, k)| k)
                .collect();
            if let Ok(page) = read_autocomplete(&kinds, index, &response) {
                page.items.iter().for_each(check_item);
                check_next(index, page.next.as_ref());
            }
        }
        2 => {
            if let Ok(suggestions) = read_suggestions(&response) {
                assert!(suggestions.len() <= MAX_SUGGESTIONS);
                for (i, a) in suggestions.iter().enumerate() {
                    assert!(
                        suggestions[..i]
                            .iter()
                            .all(|b| a.to_lowercase() != b.to_lowercase())
                    );
                }
            }
        }
        3 => {
            if let Ok(page) = read_playlist(index, &response) {
                check_item(&Item::Playlist(page.playlist));
                page.tracks.iter().for_each(check_track);
                check_next(index, page.next.as_ref());
            }
        }
        4 => {
            if let Ok(item) = read_item(kind, &response) {
                check_item(&item);
            }
        }
        5 | 6 => {
            let mut wanted = Wanted::default();
            wanted.title = title.to_owned();
            wanted.artists = artists.into_iter().map(str::to_owned).collect();
            if let Ok(artwork) = read_artwork_search(kind, &wanted, &response) {
                assert!(!artwork.is_empty());
                check_artwork(&artwork);
            }
        }
        _ => {
            if let Ok(artwork) = read_artwork_by_isrc(&response) {
                assert!(!artwork.is_empty());
                check_artwork(&artwork);
            }
        }
    }
});
