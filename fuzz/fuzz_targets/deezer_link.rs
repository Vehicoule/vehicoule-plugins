//! A link the user pasted, as `link::parse` reads it. Asserts no panic, and that a link read is a Deezer id: decimal
//! digits that fit a `u64`, which the requests accept.

#![no_main]

use libfuzzer_sys::fuzz_target;
use vehicoule_plugin_deezer::link::parse;
use vehicoule_plugin_deezer::request::item;

fuzz_target!(|link: &str| {
    if let Ok(read) = parse(link) {
        assert!(read.id.bytes().all(|b| b.is_ascii_digit()));
        assert!(item(read.kind, &read.id).is_ok());
    }
});
