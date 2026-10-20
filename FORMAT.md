# LyricsLab TTML editing profile 1

Native files are W3C TTML2 XML, normally saved as `.ttml`. They are not the earlier custom
`<song>` vocabulary. See [TTML2](https://www.w3.org/TR/ttml2/), especially content, media timing,
metadata and foreign namespace processing. This is a deliberately limited editor, not a general
TTML renderer or an Apple Music/IMSC implementation.

## Structure and timing

The root is `{http://www.w3.org/ns/ttml}tt`. Its `head/metadata` contains `ttm:title` plus Dublin
Core Terms `creator` (artist/author), `description` (recording/edition), `source`, `license`, and
`contributor`. `body/@end` records the recording duration. `xml:lang` gives the recognition
language (`auto` requests Whisper detection). Metadata is plain text; URLs are never fetched.

Each `body/div` is one performed section occurrence, each `p` one lyric line, and each `span`
one whitespace-delimited word. Repeated choruses have independent divs and timestamps. An empty
div with `ttm:role="music"` represents an instrumental. Vocal divs use `ttm:role="lyrics"`.
Section names are standard `div/metadata/ttm:title` elements. IDs use `xml:id`.

Timing uses standard `begin` and `end` attributes, with the media time base and parallel
containers. **Child offsets are relative to the parent's begin**, including word offsets relative
to line begins. The editor displays absolute recording seconds. Writers emit six decimal places
and seconds units. Imports also accept `dur`, millisecond/minute/hour offsets and three-part
clock expressions. Frame/tick expressions, sequential containers and clock/SMPTE bases are
rejected. No supplied lyric text is sent to Whisper as a recognition prompt.

Untimed nodes omit timing attributes. In a TTML player their implicit timing still follows TTML
rules; omission means *unresolved for editing*, not a special standard TTML timing state.
Untimed documents are drafts, not finished playback subtitles. `p/@xml:space="preserve"` and the
whitespace between spans preserve lyric text. Complete lines may be confirmed before every word
is timed. SRT/WebVTT export uses confirmed lines only.

## Optional editorial metadata

Namespace `ll` is `https://github.com/ianrastall/lyricslab-songs/ns/1`.

| Location | Attribute | Meaning |
| --- | --- | --- |
| tt | ll:version="1" | Version of this optional editorial vocabulary |
| div | ll:kind | Section kind, such as verse, chorus, bridge, instrumental |
| p, span | ll:state | untimed, suggested, estimated, confirmed |
| p, span | ll:score | Minimum recognition token score, 0–1, when available |

Foreign metadata is permitted by TTML. Other processors can ignore it while retaining the
standard text and timing. Removing it loses review history and exact section kinds, not TTML
text or timing. An imported standard timing without ll:state becomes a *suggestion*, requiring
review. Unknown metadata, styling, agents, nested divisions, nested/phrase spans and translations
are rejected rather than silently discarded. This editor does not claim to import every TTML file.

## Validation and contribution rules

The core checks unique NCName-compatible ASCII IDs, exact word/text correspondence, bounded
finite times, ordering, containment and non-overlap. Limits: 200 sections, 2,000 lines, 10,000
words, 1 MB combined lyric text and 4 MB XML input. DTDs/external entities are prohibited.
Native files are tested against the W3C XSD after foreign namespace pruning, as required by
the specification's abstract document validation procedure. XSD validity alone does not check
timing semantics; the domain validator adds those checks.

Community submissions require title, source, contributor and rights/license metadata. Timed
submissions also identify the recording/edition and duration. Do not put local file paths or
audio files in submissions. Different recordings need separate files. An untimed contribution
is welcome. Review states describe editorial work, not a guarantee of accuracy.

## OpenLyrics exchange

OpenLyrics 0.8/0.9 imports expand verseOrder into individual untimed occurrences, splitting
`br` line breaks. Basic title, lyric authors and copyright text are retained. Unsupported chords,
instruments, translation variants, multipart lyrics and extra metadata are rejected. Its
[official schema](https://github.com/openlyrics/openlyrics/blob/master/openlyrics-0.9.rng)
and [format documentation](https://github.com/openlyrics/openlyrics/blob/master/docs/source/dataformat.rst)
were checked during implementation.

OpenLyrics 0.9 export is explicitly lossy: title, language, vocal text and performed order only.
Identical vocal sections of the same kind reuse a definition. Instrumentals, timings, review
state, recording, author and rights metadata are omitted after a visible confirmation. TTML
remains the complete saved document.
