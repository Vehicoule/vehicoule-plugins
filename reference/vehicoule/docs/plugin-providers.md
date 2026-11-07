# Plugin providers: which service does what

- Status: Accepted (the user, 2026-11-06)
- Scope: the first plugins, the capabilities each one provides, and the default order the core asks them in, for
  the music domain. The routing itself (a priority list per capability, the next candidate on failure) is the
  core's ([plugin host spec](plugin-host-spec.md), section 1); this file gives its default lists.
- Rules: [plugin host spec](plugin-host-spec.md) (the contract and its capabilities), AGENTS.md rule 8 (every claim
  about a service says how it was checked).

## 1. The domains

Vehicoule has three content domains (`domain` in the contract):

- **Music**: tracks, albums, artists, playlists.
- **Video**: films, series, anime.
- **Books**: novels, manga, comics (BD), light novels, newspapers.

Audiobooks are not a domain. This file covers music; video and books get their providers in their own section when
their work starts.

## 2. Music

Chosen by the user on 2026-11-06:

| Capability | Default order | Why |
|---|---|---|
| `search` (search and suggestions) | `deezer`, then `itunes` | Deezer gives each track's ISRC, searches playlists and has an autocomplete endpoint; iTunes has none of the three (section 4) |
| `metadata` (album, artist, playlist pages) | the provider of the item's id, with no fallback | A page is asked by id, and an id belongs to the plugin that returned it ([plugin host spec](plugin-host-spec.md), section 8): another plugin cannot read it. When that provider fails, the page fails; the user can search again, which falls back to `itunes` |
| `artwork` (covers and artist pictures) | `deezer`, then `itunes` | Deezer gives covers and artist pictures; iTunes gives covers only. `find` takes the whole item (title, artists, album), so a provider can look up an item another one returned |
| `resolve` (a track to a playable stream) | `ytmusic` only | Deezer and iTunes give 30-second previews, not full tracks. `ytmusic` resolves tracks found by any provider, matching on ISRC, title, artists and duration |
| `radio` (continuation candidates) | `ytmusic` | Its radio starts from a track; Deezer's public radio starts from an artist |
| `lyrics` | `lrclib` only | Synced and plain lyrics. A second provider is added only if missing lyrics are measured |
| `link-import` | the provider of the link's domain (`deezer`, `itunes`) | Each plugin imports its own service's links |

`ytmusic` provides no `search`, `metadata` or `link-import`: the user said it is there to play, and maybe for radio.
The core never shows a `ytmusic` id to the user as a library item; it maps it to the item the user chose.

## 3. Where each plugin lives

| Plugin | Repository | Capabilities |
|---|---|---|
| `deezer` | `Vehicoule/vehicoule-plugins` | `search`, `metadata`, `artwork`, `link-import` |
| `itunes` | `Vehicoule/vehicoule-plugins` | `search`, `metadata`, `artwork`, `link-import` |
| `lrclib` | `Vehicoule/vehicoule-plugins` | `lyrics` |
| `ytmusic` | `Vehicoule/vehicoule-plugins-extra` ([plugin host spec](plugin-host-spec.md), section 2) | `resolve`, `radio` |

## 4. What was checked

Each claim above was checked on 2026-11-06 by calling the public endpoints once from a cloud container, without an
account, and reading the JSON returned; one call each, not a measurement.

Deezer (`api.deezer.com`):

- `GET /search?q=…` returns tracks with `isrc`, `duration`, a 30-second `preview`, the artist with `picture_xl`
  (1000×1000) and the album with `cover_xl` (1000×1000).
- `GET /track/isrc:{isrc}` finds a track by ISRC.
- `GET /search/playlist?q=…` returns playlists with their pictures.
- `GET /search/autocomplete?q=…` returns tracks, albums and artists for a prefix.
- `GET /artist/{id}/radio` returns tracks for an artist.
- `GET /album/{id}` returns `upc`, `release_date` and `cover_xl`.

iTunes Search API (`itunes.apple.com/search`):

- `entity=song` returns tracks with `trackTimeMillis`, a 30-second `previewUrl` and `artworkUrl100`; no ISRC.
- `entity=musicArtist` returns artists without a picture.
- No playlist entity, no autocomplete endpoint.
- `media=ebook` and `entity=musicVideo` return results, which makes iTunes a candidate for books and video later.
- Its documentation (`performance-partners.apple.com/search-api`) says it is "limited to approximately 20 calls per
  minute (subject to change)".

Both services serve covers larger than their documented sizes when the size in the image URL is changed (iTunes up
to 3000×3000, Deezer up to 1800×1800, one request each). This is not documented by either; a plugin does not rely on
it.

Not checked: Deezer's rate limit and its terms of use, whose pages need a Deezer account; how often a track exists on
one service and not the other, which needs a sample of real tracks. Both are checked when the `deezer` plugin is
written, and a finding that changes a row above comes back here first.
