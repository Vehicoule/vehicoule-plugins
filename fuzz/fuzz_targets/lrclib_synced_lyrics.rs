//! The LRC text of a record's `syncedLyrics`, as `lrc::parse` reads it. Asserts no panic, that the lines come in the
//! order of their times, and that each line's text is trimmed and holds no line break.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vehicoule_plugin_lrclib::lrc::parse;

fuzz_target!(|text: &str| {
    if let Ok(lines) = parse(text) {
        assert!(!lines.is_empty());
        assert!(lines.windows(2).all(|w| w[0].start <= w[1].start));
        for line in &lines {
            assert_eq!(line.text, line.text.trim());
            assert!(!line.text.contains(['\n', '\r']));
        }
    }
});
