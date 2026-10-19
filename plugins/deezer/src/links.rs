//! Deezer links: `(www.)deezer.com/(xx/)?(track|album|artist|playlist)/<id>`, and the share
//! links that redirect to one. Scheme, host and path segments are matched in any case.

use vehicoule_sdk::types::{EntityKind, EntityRef};

pub fn parse_link(url: &str) -> Option<EntityRef> {
    let (host, path) = split(url)?;
    if host != "www.deezer.com" && host != "deezer.com" {
        return None;
    }
    let path = path.split(['?', '#']).next()?.trim_end_matches('/').to_ascii_lowercase();
    let mut parts: Vec<&str> = path.split('/').collect();
    // The language: `fr/`, `en/`…
    if parts.len() == 3 && parts[0].len() == 2 && parts[0].bytes().all(|b| b.is_ascii_alphabetic()) {
        parts.remove(0);
    }
    let [kind, id] = parts[..] else { return None };
    let kind = match kind {
        "track" => EntityKind::Track,
        "album" => EntityKind::Album,
        "artist" => EntityKind::Artist,
        "playlist" => EntityKind::Playlist,
        _ => return None,
    };
    is_id(id).then(|| EntityRef { kind, id: id.to_string() })
}

/// `link.deezer.com/s/…`: only the redirect tells what it names.
pub fn is_short_link(url: &str) -> bool {
    split(url).is_some_and(|(host, path)| host == "link.deezer.com" && path.strip_prefix("s/").is_some_and(|code| !code.is_empty()))
}

/// Deezer ids are decimal: anything else never reaches a URL.
pub fn is_id(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// An http(s) URL's host, lowercased, and its path as given (without the leading `/`).
fn split(url: &str) -> Option<(String, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return None;
    }
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    Some((host.to_ascii_lowercase(), path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(url: &str) -> Option<(EntityKind, String)> {
        parse_link(url).map(|r| (r.kind, r.id))
    }

    #[test]
    fn links_are_recognised() {
        assert_eq!(link("https://www.deezer.com/fr/playlist/908622995"), Some((EntityKind::Playlist, "908622995".into())));
        assert_eq!(link("https://www.deezer.com/album/302127"), Some((EntityKind::Album, "302127".into())));
        assert_eq!(link("https://www.deezer.com/en/track/3135556?utm_source=deezer"), Some((EntityKind::Track, "3135556".into())));
        assert_eq!(link("http://www.deezer.com/artist/27/"), Some((EntityKind::Artist, "27".into())));
        assert_eq!(link("https://www.deezer.com/fr/playlist/908622995#x"), Some((EntityKind::Playlist, "908622995".into())));
        // Without `www`, and any case in the scheme, the host and the language.
        assert_eq!(link("https://deezer.com/fr/album/302127"), Some((EntityKind::Album, "302127".into())));
        assert_eq!(link("HTTPS://WWW.Deezer.COM/FR/playlist/908622995"), Some((EntityKind::Playlist, "908622995".into())));
        assert_eq!(link("Http://Deezer.com/track/3135556"), Some((EntityKind::Track, "3135556".into())));
        // Another domain, another kind, a non-numeric id, extra segments.
        assert_eq!(link("https://open.spotify.com/album/302127"), None);
        assert_eq!(link("https://www.deezer.com.evil.com/album/302127"), None);
        assert_eq!(link("https://www.deezer.com/fr/show/302127"), None);
        assert_eq!(link("https://www.deezer.com/album/30x127"), None);
        assert_eq!(link("https://www.deezer.com/album/302127/tracks"), None);
        assert_eq!(link("https://www.deezer.com/deezer-links-404"), None);
    }

    #[test]
    fn short_links_are_recognised() {
        assert!(is_short_link("https://link.deezer.com/s/34bMMRXCn45BPl52sk6ml"));
        // Firebase Dynamic Links are gone: no longer followed.
        assert!(!is_short_link("https://deezer.page.link/ABCdef123"));
        assert!(is_short_link("HTTPS://Link.Deezer.com/s/34bMMRXCn45BPl52sk6ml"));
        assert!(!is_short_link("https://link.deezer.com/s/"));
        assert!(!is_short_link("https://link.deezer.com.evil.com/s/abc"));
        assert!(!is_short_link("https://www.deezer.com/album/302127"));
    }

    #[test]
    fn ids_are_decimal() {
        assert!(is_id("908622995"));
        assert!(!is_id("") && !is_id("../1") && !is_id("1?x=2"));
    }
}
