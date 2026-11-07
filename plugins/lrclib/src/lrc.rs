//! Reads LRC text, the format of LRCLIB's `syncedLyrics`, into timed lines.
//!
//! What is read, line by line:
//!
//! - A line that starts with one or more time tags (`[mm:ss]`, `[mm:ss.f]`, `[mm:ss.ff]` or `[mm:ss.fff]`, with 1 to
//!   [`MAX_MINUTE_DIGITS`] minute digits and seconds below 60) gives one timed line per tag, with the text after the
//!   last tag, trimmed. The text may be empty: LRCLIB ends its records with such a line, which marks the end of the
//!   line before it.
//! - A blank line is skipped.
//! - An ID tag, `[key:value]` alone on its line with a key of ASCII letters (`[ar:…]`, `[length:…]`), is skipped,
//!   except `offset`, which moves every time: an offset of zero is skipped, any other is refused, since moving the
//!   lines is not implemented and reading them unmoved would put them at the wrong times.
//! - Any other line is refused: the whole text is unreadable rather than read in part.
//!
//! The lines are returned in the order of their times; lines with the same time keep the order of the text. Text with
//! no timed line is refused.

use std::time::Duration;

use crate::{Error, TimedLine};

/// The most minute digits a time tag may have. Four digits reach 9999 minutes, far past any track; the bound keeps the
/// tag short enough for its Kani proof.
pub const MAX_MINUTE_DIGITS: usize = 4;

/// Reads the LRC `text` into timed lines, in the order of their times.
///
/// # Errors
///
/// [`Error::Upstream`] for a line that is neither timed, blank nor an ID tag, for a non-zero `offset`, and for a text
/// with no timed line; the message names the line by its number, from 1.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use vehicoule_plugin_lrclib::lrc::parse;
///
/// let lines = parse("[ar:Ana]\n[00:12.50] Blue hour\n[00:15.00] ").unwrap();
/// assert_eq!(lines.len(), 2);
/// assert_eq!(lines[0].start, Duration::from_millis(12_500));
/// assert_eq!(lines[0].text, "Blue hour");
/// assert_eq!(lines[1].text, "");
/// ```
pub fn parse(text: &str) -> Result<Vec<TimedLine>, Error> {
    let mut timed = Vec::new();
    // `lines` ends a line at `\n` or `\r\n`; a lone `\r` ends one too, as in LRC files written with old Mac line
    // endings, so that no text keeps a line break. Such a line is numbered as the `\n` line it is part of.
    let lines_and_numbers = text.lines().enumerate().flat_map(|(index, line)| {
        line.split('\r')
            .map(move |line| (index.saturating_add(1), line))
    });
    for (number, line) in lines_and_numbers {
        if line.trim().is_empty() {
            continue;
        }
        let mut rest = line.as_bytes();
        let mut starts = Vec::new();
        while let Some((start, after)) = time_tag(rest) {
            starts.push(start);
            rest = after;
        }
        if starts.is_empty() {
            id_tag(line, number)?;
            continue;
        }
        // `rest` is what follows a `]`, an ASCII byte, so it starts on a character boundary of `line`.
        let words = std::str::from_utf8(rest)
            .map_err(|_| Error::Upstream(format!("synced lyrics, line {number}: not UTF-8")))?
            .trim();
        timed.extend(starts.into_iter().map(|start| TimedLine {
            start,
            text: words.to_owned(),
        }));
    }
    if timed.is_empty() {
        return Err(Error::Upstream(String::from(
            "synced lyrics hold no timed line",
        )));
    }
    timed.sort_by_key(|line| line.start);
    Ok(timed)
}

/// Checks that `line`, which has no time tag, is an ID tag that can be skipped.
fn id_tag(line: &str, number: usize) -> Result<(), Error> {
    let unreadable = || Error::Upstream(format!("synced lyrics, line {number}: not LRC"));
    let inner = line
        .trim()
        .strip_prefix('[')
        .and_then(|l| l.strip_suffix(']'))
        .ok_or_else(unreadable)?;
    let (key, value) = inner.split_once(':').ok_or_else(unreadable)?;
    if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Err(unreadable());
    }
    if key.eq_ignore_ascii_case("offset") && value.trim().parse::<i64>() != Ok(0) {
        return Err(Error::Upstream(format!(
            "synced lyrics, line {number}: an offset tag other than 0 is not supported"
        )));
    }
    Ok(())
}

/// Reads one time tag at the start of `bytes`: `[` then 1 to [`MAX_MINUTE_DIGITS`] minute digits, `:`, two second
/// digits below 60, an optional `.` with 1 to 3 digits of fraction (tenths, hundredths or thousandths), and `]`.
/// Returns its time and what follows the tag, or `None` if `bytes` does not start with one.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use vehicoule_plugin_lrclib::lrc::time_tag;
///
/// assert_eq!(time_tag(b"[01:02.5]x"), Some((Duration::from_millis(62_500), &b"x"[..])));
/// assert_eq!(time_tag(b"[1:60]"), None);
/// ```
#[must_use]
pub fn time_tag(bytes: &[u8]) -> Option<(Duration, &[u8])> {
    let rest = bytes.strip_prefix(b"[")?;
    let close = rest.iter().position(|&b| b == b']')?;
    let (inner, after) = rest.split_at_checked(close)?;
    let after = after.strip_prefix(b"]")?;
    let colon = inner.iter().position(|&b| b == b':')?;
    let (minutes, seconds) = inner.split_at_checked(colon)?;
    let seconds = seconds.strip_prefix(b":")?;
    let (seconds, fraction) = match seconds.iter().position(|&b| b == b'.') {
        Some(dot) => {
            let (seconds, fraction) = seconds.split_at_checked(dot)?;
            (seconds, fraction.strip_prefix(b".")?)
        }
        None => (seconds, &b""[..]),
    };
    if minutes.is_empty() || minutes.len() > MAX_MINUTE_DIGITS || seconds.len() != 2 {
        return None;
    }
    let seconds = digits(seconds)?;
    if seconds >= 60 {
        return None;
    }
    // A fraction of n digits counts units of 10^(3 - n) milliseconds; a dot with no digit, or more than 3, is no tag.
    let fraction_ms = match fraction.len() {
        0 if inner.contains(&b'.') => return None,
        0 => 0,
        1 => digits(fraction)?.checked_mul(100)?,
        2 => digits(fraction)?.checked_mul(10)?,
        3 => digits(fraction)?,
        _ => return None,
    };
    let ms = digits(minutes)?
        .checked_mul(60_000)?
        .checked_add(seconds.checked_mul(1_000)?)?
        .checked_add(fraction_ms)?;
    Some((Duration::from_millis(ms), after))
}

/// The value of ASCII decimal `digits`, or `None` if one byte is not a digit or the value does not fit.
fn digits(digits: &[u8]) -> Option<u64> {
    digits.iter().try_fold(0_u64, |value, &b| {
        let digit = char::from(b).to_digit(10)?;
        value.checked_mul(10)?.checked_add(u64::from(digit))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(ms: u64, text: &str) -> TimedLine {
        TimedLine {
            start: Duration::from_millis(ms),
            text: String::from(text),
        }
    }

    #[test]
    fn reads_every_form_of_time_tag() {
        let cases: [(&[u8], u64); 6] = [
            (b"[00:17.12]", 17_120),
            (b"[03:25.7]", 205_700),
            (b"[3:25.705]", 205_705),
            (b"[3:25]", 205_000),
            (b"[9999:59.999]", 599_999_999),
            (b"[0:00.00]", 0),
        ];
        for (tag, ms) in cases {
            assert_eq!(
                time_tag(tag),
                Some((Duration::from_millis(ms), &b""[..])),
                "{}",
                String::from_utf8_lossy(tag)
            );
        }
    }

    #[test]
    fn returns_what_follows_the_tag() {
        assert_eq!(
            time_tag(b"[00:01.00][00:02.00] a"),
            Some((Duration::from_secs(1), &b"[00:02.00] a"[..]))
        );
    }

    #[test]
    fn refuses_what_is_not_a_time_tag() {
        let cases: [&[u8]; 18] = [
            b"",
            b"00:01.00]",
            b"[00:01.00",
            b"[:01.00]",
            b"[00:1.00]",
            b"[00:001.00]",
            b"[00:60.00]",
            b"[00:01.]",
            b"[00:01.0000]",
            b"[00:01:00]",
            b"[0a:01.00]",
            b"[00:0a.00]",
            b"[00:01.0a]",
            b"[10000:00.00]",
            b"[00.01.00]",
            b"[ar:Ana]",
            b"[00:01,00]",
            b" [00:01.00]",
        ];
        for tag in cases {
            assert_eq!(time_tag(tag), None, "{}", String::from_utf8_lossy(tag));
        }
    }

    #[test]
    fn digits_refuse_a_value_that_does_not_fit() {
        assert_eq!(digits(b"18446744073709551615"), Some(u64::MAX));
        assert_eq!(digits(b"18446744073709551616"), None);
        assert_eq!(digits(b""), Some(0));
        assert_eq!(digits(b"0x"), None);
    }

    #[test]
    fn reads_lrclibs_lines_and_trims_their_text() {
        let lines =
            parse("[00:17.12] I see the light\n[00:20.50]  Keep going  \n[00:25.72] ").unwrap();
        assert_eq!(
            lines,
            vec![
                line(17_120, "I see the light"),
                line(20_500, "Keep going"),
                line(25_720, ""),
            ]
        );
    }

    #[test]
    fn a_line_with_several_tags_repeats_at_each_time_in_order() {
        let lines =
            parse("[00:10.00][00:30.00] chorus\n[00:20.00] verse\r\n[00:20.00] verse two").unwrap();
        assert_eq!(
            lines,
            vec![
                line(10_000, "chorus"),
                line(20_000, "verse"),
                line(20_000, "verse two"),
                line(30_000, "chorus"),
            ]
        );
    }

    #[test]
    fn skips_blank_lines_and_id_tags() {
        let lines =
            parse("[ar:Ana]\n\n  \n[length: 03:12]\n[offset:+0]\n[00:01.00] a\n[Ti:x]").unwrap();
        assert_eq!(lines, vec![line(1_000, "a")]);
    }

    #[test]
    fn refuses_a_non_zero_offset() {
        for offset in ["[offset:+500]", "[OFFSET:-20]", "[offset:soon]"] {
            let text = format!("{offset}\n[00:01.00] a");
            assert_eq!(
                parse(&text),
                Err(Error::Upstream(String::from(
                    "synced lyrics, line 1: an offset tag other than 0 is not supported"
                ))),
                "{offset}"
            );
        }
    }

    #[test]
    fn refuses_a_line_that_is_not_lrc_and_names_it() {
        for bad in [
            "plain words",
            "[00:01:00] a",
            "[a1:x]",
            "[:x]",
            "[ar Ana]",
            "[ar:Ana",
            "x [00:01.00] a",
        ] {
            let text = format!("[00:00.50] ok\n{bad}");
            assert_eq!(
                parse(&text),
                Err(Error::Upstream(String::from(
                    "synced lyrics, line 2: not LRC"
                ))),
                "{bad}"
            );
        }
    }

    #[test]
    fn refuses_text_with_no_timed_line() {
        for text in ["", "\n \n", "[ar:Ana]\n[ti:x]"] {
            assert_eq!(
                parse(text),
                Err(Error::Upstream(String::from(
                    "synced lyrics hold no timed line"
                ))),
                "{text:?}"
            );
        }
    }

    // Found by the `lrclib_synced_lyrics` fuzz target before the plugin was merged: a lone `\r` stayed in a line's
    // text, with the tags that followed it.
    #[test]
    fn a_lone_carriage_return_ends_a_line() {
        assert_eq!(
            parse("[00:01.00] a\r[00:02.00] b\r\n[00:03.00] c\r").unwrap(),
            vec![line(1_000, "a"), line(2_000, "b"), line(3_000, "c")]
        );
        assert_eq!(
            parse("[00:01.00] a\rnot lrc\n"),
            Err(Error::Upstream(String::from(
                "synced lyrics, line 1: not LRC"
            )))
        );
        assert_eq!(
            parse("[00:01.00] a\n[00:02.00] b\rnot lrc"),
            Err(Error::Upstream(String::from(
                "synced lyrics, line 2: not LRC"
            )))
        );
    }

    #[test]
    fn keeps_text_that_is_not_ascii() {
        assert_eq!(
            parse("[00:01.00] été ♪").unwrap(),
            vec![line(1_000, "été ♪")]
        );
    }
}

#[cfg(kani)]
mod proofs;
