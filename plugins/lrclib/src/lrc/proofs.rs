//! The Kani proof of [`super::time_tag`] (docs/plugin-crates.md, section 5).

use super::*;

/// The longest input the proof covers: the longest tag, `[` + 4 minute digits + `:` + 2 second digits + `.` + 3
/// fraction digits + `]`, is 13 bytes, and one more byte checks what follows it.
const MAX_LEN: usize = 14;

/// For every input of up to `MAX_LEN` bytes: no panic, and a tag that is read is a prefix of the input that
/// starts with `[` and ends with `]`, whose time is below the largest a tag can write.
#[kani::proof]
#[kani::unwind(16)]
fn reads_any_bytes_without_panic_and_returns_a_suffix() {
    let bytes: [u8; MAX_LEN] = kani::any();
    let len: usize = kani::any_where(|&l| l <= MAX_LEN);
    let input = &bytes[..len];
    if let Some((start, rest)) = time_tag(input) {
        let taken = input.len() - rest.len();
        assert!(rest == &input[taken..]);
        assert!(taken >= 6);
        assert!(input[0] == b'[');
        assert!(input[taken - 1] == b']');
        assert!(start <= Duration::from_millis(9999 * 60_000 + 59_999));
    }
}
