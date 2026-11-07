//! What a request is built from: a query or a prefix the user typed, an id and a cursor the plugin handed out and gets
//! back, an ISRC from another plugin. Asserts no panic, and that every request is a URL on `api.deezer.com` made of
//! nothing but ASCII that cannot add a path segment, a parameter or a fragment.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vehicoule_plugin_deezer::model::ItemKind;
use vehicoule_plugin_deezer::request::{
    Request, album, artist, cursor, is_isrc, item, playlist, search, search_plan, suggest,
    track_by_isrc,
};

/// Checks a request: `https://api.deezer.com/`, then a path of known segments, then `name=value` parameters whose
/// values are percent-encoded or decimal.
fn check(request: &Request) {
    let rest = request
        .url
        .strip_prefix("https://api.deezer.com/")
        .expect("a request on api.deezer.com");
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    for segment in path.split('/') {
        assert!(
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-._~%:".contains(&b)),
            "{segment:?}"
        );
        assert!(segment != "." && segment != "..");
    }
    for parameter in query.split('&').filter(|p| !p.is_empty()) {
        let (name, value) = parameter.split_once('=').expect("name=value");
        assert!(matches!(name, "q" | "index" | "limit"), "{name:?}");
        assert!(
            value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~%".contains(&b)),
            "{value:?}"
        );
    }
}

fuzz_target!(|input: (&str, &str, Option<&str>, u8)| {
    let (text, id, cursor_text, kinds) = input;
    let all = [
        ItemKind::Track,
        ItemKind::Album,
        ItemKind::Artist,
        ItemKind::Playlist,
    ];
    let kinds: Vec<ItemKind> = all
        .into_iter()
        .enumerate()
        .filter(|(i, _)| kinds & (1 << i) != 0)
        .map(|(_, k)| k)
        .collect();
    let index = cursor(cursor_text).unwrap_or(0);
    if let Ok(request) = search(text, &search_plan(&kinds), index) {
        check(&request);
    }
    if let Ok(request) = suggest(text) {
        check(&request);
    }
    if is_isrc(text) {
        check(&track_by_isrc(text));
    }
    if let Ok(requests) = album(id) {
        requests.iter().for_each(check);
    }
    if let Ok(requests) = artist(id) {
        requests.iter().for_each(check);
    }
    if let Ok(request) = playlist(id, index) {
        check(&request);
    }
    for kind in all {
        if let Ok(request) = item(kind, id) {
            check(&request);
        }
    }
});
