//! LRCLIB's answer to a lyrics request, as `response::read_get` reads it: any status, any `Retry-After`, any body.
//! Asserts no panic, and that synced lyrics come in the order of their times.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vehicoule_plugin_lrclib::Lyrics;
use vehicoule_plugin_lrclib::response::read_get;

fuzz_target!(|data: &[u8]| {
    // The first two bytes pick the status among the ones the reader tells apart, the third how `Retry-After` is
    // written; the rest is the body.
    let [kind, retry, body @ ..] = data else {
        return;
    };
    let status = match kind % 6 {
        0 | 1 => 200,
        2 => 404,
        3 => 429,
        4 => 503,
        _ => u16::from(*kind).saturating_mul(3),
    };
    let headers = vec![
        (
            String::from("Content-Type"),
            String::from("application/json"),
        ),
        (String::from("Retry-After"), retry.to_string()),
    ];
    if let Ok(Lyrics::Synced(lines)) = read_get(status, &headers, body) {
        assert!(!lines.is_empty());
        assert!(lines.windows(2).all(|w| w[0].start <= w[1].start));
    }
});
