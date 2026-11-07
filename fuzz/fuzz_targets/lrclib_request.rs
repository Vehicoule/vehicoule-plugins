//! The track a lyrics request is built from, which comes from another plugin and so from another service. Asserts no
//! panic, and that a request is a URL on LRCLIB's lyrics endpoint made of nothing but ASCII that cannot end a query
//! parameter early.

#![no_main]

use std::time::Duration;

use libfuzzer_sys::fuzz_target;
use vehicoule_plugin_lrclib::request::{Query, get};

fuzz_target!(|input: (&str, Vec<&str>, Option<&str>, Option<u64>)| {
    let (title, artists, album, duration_ms) = input;
    let mut query = Query::default();
    query.title = title.to_owned();
    query.artists = artists.into_iter().map(str::to_owned).collect();
    query.album = album.map(str::to_owned);
    query.duration = duration_ms.map(Duration::from_millis);
    if let Ok(request) = get(&query) {
        let query_string = request
            .url
            .strip_prefix("https://lrclib.net/api/get?")
            .expect("a request on LRCLIB's lyrics endpoint");
        for parameter in query_string.split('&') {
            let (name, value) = parameter.split_once('=').expect("name=value");
            assert!(matches!(
                name,
                "track_name" | "artist_name" | "album_name" | "duration"
            ));
            assert!(
                value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-._~%".contains(&b))
            );
        }
    }
});
