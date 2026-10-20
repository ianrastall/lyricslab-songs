# Submitting a song

1. Fork the repository and create a branch.
2. Save a TTML document from LyricsLab (or use the documented editing subset).
3. Add it under `songs/<artist-or-author>/<title-and-recording>.ttml`; use a separate file for
   a different recording, edit, mix or tempo. Never upload recordings or local file paths.
4. Set title, source/attribution, rights/license and contributor. For timed files, identify
   the recording/edition and duration. State whether timings are suggested, estimated or reviewed.
5. Run the checks in README.md and open a pull request using the template.

You must have permission to redistribute the full submitted lyrics. For public-domain material,
identify the source and jurisdiction. For licensed material, link the applicable license or
permission. Do not submit commercial song lyrics merely because they are publicly viewable.

Untimed structure submissions are useful. Whisper candidates are suggestions until reviewed;
never relabel estimates as confirmed without listening and checking. Repeated choruses must
have separate timing occurrences. Keep instrumental intervals explicit when known.

Maintainers review rights/provenance, recording identity, text, structure and timings. CI checks
technical validity only; it cannot establish rights or musical accuracy. A failed check should
be fixed before merging. Use pull requests for corrections and preserve existing attribution.

For validator changes, update tests and FORMAT.md with any supported-subset or metadata change.
Keep the native app and this independently buildable validator in sync deliberately.
