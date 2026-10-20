# LyricsLab songs

A community collection of sectioned song lyrics in **W3C TTML2 XML**, with optional line/word
timings. Untimed and partly timed documents are welcome. Each timed document belongs to a
specific recording/edition. No audio recordings or local audio paths belong in this repository.

LyricsLab is the Windows editor in [MediaLab](https://github.com/ianrastall/medialab-suite).
Use its three tabs to structure text, gather timing references, then refine timestamps and
export SRT/WebVTT. Save the complete work as `.ttml`.

## Contribute

Anyone with a GitHub account can fork this repository and open a pull request. Put a song under
`songs/<artist-or-author>/<song-and-recording>.ttml`, include source, contributor and rights
metadata, and describe the recording and timing work in the PR. Maintainers review submissions;
automated checks do not merge them. See [CONTRIBUTING.md](CONTRIBUTING.md).

The initial `songs/examples/morning-light-demo.ttml` is original CC0 demonstration text with
illustrative timestamps, not timings for an existing recording.

## Format and validation

[FORMAT.md](FORMAT.md) documents the supported editing subset and optional editorial metadata.
The main vocabulary and timing are standard TTML. Other TTML processors may ignore the optional
LyricsLab attributes. This editor currently rejects unsupported styling/agents/translations
instead of discarding them. More TTML interoperability can be added through reviewed changes.

Install Rust 1.99.1 and Python 3.12+; then run:

```sh
cargo test --locked
python -m pip install -r requirements.txt
python scripts/check.py
```

The validator is a portable snapshot of LyricsLab's document rules, independently buildable
without the Windows app or speech tools. CI checks both W3C XSD validity after foreign namespace
pruning and domain rules including order, containment and contribution metadata.

## Rights

Validator code/documentation is MIT licensed, except the W3C schemas under their upstream
license. **That license does not license the lyrics.** Each song carries its own `dc:license`
and `dc:source` values. Submit your own text, public-domain text with a source and jurisdiction,
or text under an explicit license/permission allowing this redistribution. Attribution alone
is not permission. Corrections to timing must retain the song's provenance and license.
