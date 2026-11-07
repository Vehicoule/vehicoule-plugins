//! The Kani proof of [`super::encode_byte`], which [`super::percent_encode`] applies to each byte
//! (docs/plugin-crates.md, section 5).

use super::{EncodedByte, encode_byte};

/// For every byte: no panic, a byte is kept if and only if it is unreserved, and an escaped byte is `%` and two
/// uppercase hexadecimal digits that give the byte back.
#[kani::proof]
fn encodes_every_byte_reversibly() {
    let byte: u8 = kani::any();
    let unreserved = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~');
    match encode_byte(byte) {
        EncodedByte::Kept(kept) => {
            assert!(unreserved);
            assert!(kept == byte);
        }
        EncodedByte::Escaped([percent, high, low]) => {
            assert!(!unreserved);
            assert!(percent == b'%');
            assert!(hex_value(high) * 16 + hex_value(low) == byte);
        }
    }
}

fn hex_value(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        b'A'..=b'F' => digit - b'A' + 10,
        _ => panic!("not an uppercase hexadecimal digit"),
    }
}
