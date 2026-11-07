//! Reads a link to a Deezer page that the user pasted: `https://www.deezer.com/fr/album/302127`.

use crate::model::{Error, ItemKind};

/// What a link points to: the kind of item and its Deezer id, in decimal.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Link {
    /// The kind of item.
    pub kind: ItemKind,
    /// Deezer's id of the item.
    pub id: String,
}

/// Reads a link to a Deezer track, album, artist or playlist page.
///
/// Accepted: `https://` or `http://` (or no scheme), the host `www.deezer.com` or `deezer.com` (any case), an optional
/// language segment of two letters (`fr`) or of the form `pt-br`, then `track`, `album`, `artist` or `playlist`, and
/// the id in decimal digits, with nothing after it but a query or a fragment (`?utm_source=…`). The link is trimmed
/// first.
///
/// # Errors
///
/// [`Error::Unsupported`] for a link on another Deezer host (the short links of `link.deezer.com` and
/// `deezer.page.link`, which need a redirect to read); [`Error::InvalidInput`] for anything else.
///
/// # Examples
///
/// ```
/// use vehicoule_plugin_deezer::link::parse;
/// use vehicoule_plugin_deezer::model::ItemKind;
///
/// let link = parse("https://www.deezer.com/fr/album/302127?utm_source=deezer").unwrap();
/// assert_eq!(link.kind, ItemKind::Album);
/// assert_eq!(link.id, "302127");
/// ```
pub fn parse(link: &str) -> Result<Link, Error> {
    let invalid = || Error::InvalidInput(format!("not a link to a Deezer page: {link:?}"));
    let trimmed = link.trim();
    let rest = strip_prefix_ignore_case(trimmed, "https://")
        .or_else(|| strip_prefix_ignore_case(trimmed, "http://"))
        .unwrap_or(trimmed);
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    if !(host.eq_ignore_ascii_case("www.deezer.com") || host.eq_ignore_ascii_case("deezer.com")) {
        let deezer_host = host.to_ascii_lowercase();
        if deezer_host == "link.deezer.com" || deezer_host == "deezer.page.link" {
            return Err(Error::Unsupported);
        }
        return Err(invalid());
    }
    let mut segments: Vec<&str> = path.split('/').collect();
    if segments.last() == Some(&"") {
        segments.pop();
    }
    let segments = match segments.as_slice() {
        [language, rest @ ..] if is_language(language) => rest,
        all => all,
    };
    let [kind, id] = segments else {
        return Err(invalid());
    };
    let kind = match *kind {
        "track" => ItemKind::Track,
        "album" => ItemKind::Album,
        "artist" => ItemKind::Artist,
        "playlist" => ItemKind::Playlist,
        _ => return Err(invalid()),
    };
    crate::request::id(id).map_err(|_| invalid())?;
    Ok(Link {
        kind,
        id: (*id).to_owned(),
    })
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| text.get(prefix.len()..))
        .flatten()
}

/// Whether a path segment is a language: two ASCII lowercase letters, optionally `-` and two more (`pt-br`).
fn is_language(segment: &str) -> bool {
    let letters = |part: &str| part.len() == 2 && part.bytes().all(|b| b.is_ascii_lowercase());
    match segment.split_once('-') {
        Some((language, region)) => letters(language) && letters(region),
        None => letters(segment),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[expect(
        clippy::unnecessary_wraps,
        reason = "compared with what `parse` returns"
    )]
    fn link(kind: ItemKind, id: &str) -> Result<Link, Error> {
        Ok(Link {
            kind,
            id: String::from(id),
        })
    }

    #[test]
    fn reads_every_kind_with_and_without_language() {
        assert_eq!(
            parse("https://www.deezer.com/track/3135556"),
            link(ItemKind::Track, "3135556")
        );
        assert_eq!(
            parse("https://www.deezer.com/fr/album/302127"),
            link(ItemKind::Album, "302127")
        );
        assert_eq!(
            parse("https://www.deezer.com/pt-br/artist/27"),
            link(ItemKind::Artist, "27")
        );
        assert_eq!(
            parse("https://deezer.com/en/playlist/908622995/"),
            link(ItemKind::Playlist, "908622995")
        );
    }

    #[test]
    fn ignores_case_in_scheme_and_host_spaces_and_what_follows_the_path() {
        assert_eq!(
            parse("  HTTPS://WWW.Deezer.com/fr/track/1?utm_source=deezer&x=/album/2#top \n"),
            link(ItemKind::Track, "1")
        );
        assert_eq!(
            parse("http://deezer.com/track/1"),
            link(ItemKind::Track, "1")
        );
        assert_eq!(parse("www.deezer.com/track/1"), link(ItemKind::Track, "1"));
        assert_eq!(parse("deezer.com/track/1#x"), link(ItemKind::Track, "1"));
    }

    #[test]
    fn short_links_are_unsupported() {
        assert_eq!(
            parse("https://link.deezer.com/s/31x2cR3E"),
            Err(Error::Unsupported)
        );
        assert_eq!(
            parse("https://deezer.page.link/abc"),
            Err(Error::Unsupported)
        );
        assert_eq!(parse("LINK.DEEZER.COM/s/abc"), Err(Error::Unsupported));
    }

    #[test]
    fn refuses_what_is_not_a_deezer_page() {
        for bad in [
            "",
            "https://",
            "https://www.deezer.com",
            "https://www.deezer.com/",
            "https://www.deezer.com/fr",
            "https://www.deezer.com/fr/track",
            "https://www.deezer.com/fr/track/",
            "https://www.deezer.com/fr/show/123",
            "https://www.deezer.com/fr/track/12a",
            "https://www.deezer.com/fr/track/-1",
            "https://www.deezer.com/fr/track/18446744073709551616",
            "https://www.deezer.com/fr/track/1/extra",
            "https://www.deezer.com/FR/track/1",
            "https://www.deezer.com/fra/track/1",
            "https://www.deezer.com/pt-BR/track/1",
            "https://www.deezer.com//track/1",
            "https://www.deezer.com/Track/1",
            "https://api.deezer.com/track/1",
            "https://evil.com/www.deezer.com/track/1",
            "https://www.deezer.com.evil.com/track/1",
            "https://music.apple.com/album/1",
            "ftp://www.deezer.com/track/1",
            "é",
        ] {
            assert!(
                matches!(parse(bad), Err(Error::InvalidInput(_))),
                "{bad:?}: {:?}",
                parse(bad)
            );
        }
    }
}
