//! LRC lyric text to timestamped lines: `[mm:ss.xx]`/`[mm:ss.xxx]`/`[mm:ss]` tags, several per
//! line, sorted by time. Metadata tags (`[ar:]`, `[ti:]`, ...) and unparseable lines are
//! ignored rather than erroring — an LRC file is third-party text, never trusted to be
//! well-formed, and a bad line here must never panic.

/// `text` to `(start_ms, line_text)` pairs, sorted by `start_ms`.
pub fn parse_lrc(text: &str) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> =
        text.split('\n').flat_map(|line| parse_line(line.trim_end_matches('\r'))).collect();
    out.sort_by_key(|(ms, _)| *ms);
    out
}

/// One line's leading `[mm:ss.xx]` tags (any number, including zero) plus the text that
/// follows; `[]` metadata tags (`[ar:...]`) and malformed tags don't parse as a timestamp, so
/// they stop the scan and the whole line is dropped (no entries), matching an LRC parser that
/// never trusts a line to carry useful time info just because it starts with `[`.
fn parse_line(line: &str) -> Vec<(u32, String)> {
    let mut rest = line;
    let mut times = Vec::new();
    while let Some(after_bracket) = rest.strip_prefix('[') {
        let Some(end) = after_bracket.find(']') else { break };
        match parse_timestamp(&after_bracket[..end]) {
            Some(ms) => {
                times.push(ms);
                rest = &after_bracket[end + 1..];
            }
            None => break,
        }
    }
    if times.is_empty() {
        return Vec::new();
    }
    let text = rest.trim().to_string();
    times.into_iter().map(|ms| (ms, text.clone())).collect()
}

/// `mm:ss`, `mm:ss.xx` (centiseconds) or `mm:ss.xxx` (milliseconds) to milliseconds; `None` for
/// anything else, including a metadata tag's `ar: Queen` (its "minutes" half isn't digits).
fn parse_timestamp(inner: &str) -> Option<u32> {
    let (mm, rest) = inner.split_once(':')?;
    let mm: u32 = parse_digits(mm)?;
    let (ss, frac_ms) = match rest.split_once('.') {
        None => (rest, 0),
        Some((ss, frac)) => {
            let f: u32 = parse_digits(frac)?;
            let ms = match frac.len() {
                1 => f * 100,
                2 => f * 10,
                3 => f,
                _ => return None,
            };
            (ss, ms)
        }
    };
    let ss: u32 = parse_digits(ss)?;
    mm.checked_mul(60)?.checked_add(ss)?.checked_mul(1000)?.checked_add(frac_ms)
}

/// Plain ASCII digits only (no sign, no whitespace); empty is refused.
fn parse_digits(s: &str) -> Option<u32> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrc_parser_reads_timestamps() {
        assert_eq!(parse_lrc("[01:02.50] a"), vec![(62_500, "a".to_string())]);
        assert_eq!(parse_lrc("[00:01.123]"), vec![(1_123, String::new())]);
    }

    #[test]
    fn lrc_parser_skips_bad_lines() {
        // Metadata tags, bad timestamps, and blank lines carry no time: ignored.
        assert_eq!(parse_lrc("[ar: X]"), Vec::<(u32, String)>::new());
        assert_eq!(parse_lrc("[xx:yy] z"), Vec::<(u32, String)>::new());
        assert_eq!(parse_lrc(""), Vec::<(u32, String)>::new());
        // Several timestamps on one line share its text: one entry per timestamp.
        assert_eq!(
            parse_lrc("[00:01.00][00:05.00] chœur"),
            vec![(1_000, "chœur".to_string()), (5_000, "chœur".to_string())]
        );
    }
}
