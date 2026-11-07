//! The Kani proof of [`super::next_index`], which every paged reader's cursor comes from (docs/plugin-crates.md,
//! section 5).

use super::next_index;

/// For every index, count and answer: no panic, a next page only when Deezer says there is one and this page was not
/// empty, and then the index just after this page's results, which is always past the page asked.
#[kani::proof]
fn the_next_index_is_just_after_the_page_or_none() {
    let index: u32 = kani::any();
    let count: u32 = kani::any();
    let has_next: bool = kani::any();
    match next_index(index, count, has_next) {
        Some(next) => {
            assert!(has_next && count > 0);
            assert!(u64::from(next) == u64::from(index) + u64::from(count));
            assert!(next > index);
        }
        None => {
            assert!(
                !has_next
                    || count == 0
                    || u64::from(index) + u64::from(count) > u64::from(u32::MAX)
            );
        }
    }
}
