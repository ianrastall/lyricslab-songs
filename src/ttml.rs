//! A documented, conservative editing subset of W3C TTML2; no network or external resources.
use crate::{
    Result,
    document::{Line, Section, Song, Timing, Word, escaped, words},
};
use roxmltree::Node;
use std::fmt::Write;
pub const TT: &str = "http://www.w3.org/ns/ttml";
pub const TTM: &str = "http://www.w3.org/ns/ttml#metadata";
pub const TTP: &str = "http://www.w3.org/ns/ttml#parameter";
pub const XML: &str = "http://www.w3.org/XML/1998/namespace";
pub const DC: &str = "http://purl.org/dc/terms/";
pub const LL: &str = "https://github.com/ianrastall/lyricslab-songs/ns/1";
fn attr(out: &mut String, k: &str, v: &str) {
    write!(out, " {k}=\"{}\"", escaped(v)).unwrap();
}
fn offset(out: &mut String, k: &str, v: Option<f64>, base: f64) {
    if let Some(v) = v {
        // Quantize absolute positions first. Independently rounding (value - parent) can
        // accumulate through div/p/span and put a word beyond its containing line on reload.
        let micros =
            ((v * 1_000_000.0).round() as i64 - (base * 1_000_000.0).round() as i64).max(0);
        write!(
            out,
            " {k}=\"{}.{:06}s\"",
            micros / 1_000_000,
            micros % 1_000_000
        )
        .unwrap();
    }
}
fn times(out: &mut String, t: &Timing, base: f64) {
    offset(out, "begin", t.start, base);
    offset(out, "end", t.end, base);
    attr(out, "ll:state", &t.state);
    if let Some(p) = t.score {
        write!(out, " ll:score=\"{p:.6}\"").unwrap();
    }
}
fn meta(out: &mut String, k: &str, v: &str) {
    if !v.is_empty() {
        writeln!(out, "      <{k}>{}</{k}>", escaped(v)).unwrap();
    }
}
pub fn to_xml(song: &Song) -> Result<String> {
    song.validate()?;
    let ranges = song.sections.iter().flat_map(|s| {
        std::iter::once((s.start, s.end)).chain(s.lines.iter().flat_map(|l| {
            std::iter::once((l.timing.start, l.timing.end))
                .chain(l.words.iter().map(|w| (w.timing.start, w.timing.end)))
        }))
    });
    for (start, end) in ranges {
        if start
            .zip(end)
            .is_some_and(|(a, b)| (b * 1_000_000.0).round() <= (a * 1_000_000.0).round())
        {
            return Err("A range is shorter than TTML's one-microsecond save precision. Increase the range before saving.".into());
        }
    }
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<tt xmlns=\"{TT}\" xmlns:ttm=\"{TTM}\" xmlns:ttp=\"{TTP}\" xmlns:dc=\"{DC}\" xmlns:ll=\"{LL}\" ttp:timeBase=\"media\" ll:version=\"1\""
    );
    attr(&mut out, "xml:lang", &song.language);
    out.push_str(">\n  <head>\n    <metadata>\n");
    for (k, v) in [
        ("ttm:title", &song.title),
        ("dc:creator", &song.artist),
        ("dc:description", &song.recording),
        ("dc:source", &song.source),
        ("dc:license", &song.license),
        ("dc:contributor", &song.contributor),
    ] {
        meta(&mut out, k, v);
    }
    out.push_str("    </metadata>\n  </head>\n  <body");
    offset(&mut out, "end", song.duration, 0.0);
    out.push_str(">\n");
    for section in &song.sections {
        out.push_str("    <div");
        attr(&mut out, "xml:id", &section.id);
        attr(&mut out, "ll:kind", &section.kind);
        attr(
            &mut out,
            "ttm:role",
            if section.kind.eq_ignore_ascii_case("instrumental") {
                "music"
            } else {
                "lyrics"
            },
        );
        offset(&mut out, "begin", section.start, 0.0);
        offset(&mut out, "end", section.end, 0.0);
        out.push_str(">\n      <metadata><ttm:title>");
        out.push_str(&escaped(&section.label));
        out.push_str("</ttm:title></metadata>\n");
        let base = section.start.unwrap_or(0.0);
        for line in &section.lines {
            out.push_str("      <p");
            attr(&mut out, "xml:id", &line.id);
            attr(&mut out, "xml:space", "preserve");
            times(&mut out, &line.timing, base);
            out.push('>');
            let word_base = line.timing.start.unwrap_or(base);
            let mut cursor = 0;
            for word in &line.words {
                let relative = line.text[cursor..]
                    .find(&word.text)
                    .ok_or("Word text no longer matches its line.")?;
                let pos = cursor + relative;
                out.push_str(&escaped(&line.text[cursor..pos]));
                out.push_str("<span");
                times(&mut out, &word.timing, word_base);
                out.push('>');
                out.push_str(&escaped(&word.text));
                out.push_str("</span>");
                cursor = pos + word.text.len();
            }
            out.push_str(&escaped(&line.text[cursor..]));
            out.push_str("</p>\n");
        }
        out.push_str("    </div>\n");
    }
    out.push_str("  </body>\n</tt>\n");
    Ok(out)
}
pub fn time(value: &str) -> Result<f64> {
    let n = if value.contains(':') {
        let p: Vec<_> = value.split(':').collect();
        if p.len() != 3 {
            return Err("Only hh:mm:ss.fraction clock times are supported; frame/tick timing is not supported.".into());
        }
        let h = p[0].parse::<u32>().map_err(|_| "Invalid clock hour.")?;
        let m = p[1].parse::<u32>().map_err(|_| "Invalid clock minute.")?;
        let s = p[2].parse::<f64>().map_err(|_| "Invalid clock seconds.")?;
        if m >= 60 || !(0.0..60.0).contains(&s) {
            return Err("Invalid clock time.".into());
        }
        h as f64 * 3600.0 + m as f64 * 60.0 + s
    } else {
        let (number, factor) = if let Some(v) = value.strip_suffix("ms") {
            (v, 0.001)
        } else if let Some(v) = value.strip_suffix('s') {
            (v, 1.0)
        } else if let Some(v) = value.strip_suffix('m') {
            (v, 60.0)
        } else if let Some(v) = value.strip_suffix('h') {
            (v, 3600.0)
        } else {
            return Err(
                "Use TTML seconds, milliseconds, minutes, hours, or hh:mm:ss.fraction.".into(),
            );
        };
        number.parse::<f64>().map_err(|_| "Invalid TTML time.")? * factor
    };
    if !n.is_finite() || !(0.0..=86400.0).contains(&n) {
        return Err("TTML times must be finite and within 24 hours.".into());
    }
    Ok(n)
}
fn bound(n: Node<'_, '_>, base: f64) -> Result<(Option<f64>, Option<f64>)> {
    let start = n
        .attribute("begin")
        .map(time)
        .transpose()?
        .map(|v| v + base);
    let end = n.attribute("end").map(time).transpose()?.map(|v| v + base);
    let duration = n.attribute("dur").map(time).transpose()?;
    if end.is_some() && duration.is_some() {
        return Err("This editor supports end or dur, not both on the same element.".into());
    }
    Ok((start, end.or(duration.map(|d| start.unwrap_or(base) + d))))
}
fn timing(n: Node<'_, '_>, base: f64) -> Result<Timing> {
    let (start, end) = bound(n, base)?;
    let state = n
        .attribute((LL, "state"))
        .unwrap_or(if start.is_some() || end.is_some() {
            "suggested"
        } else {
            "untimed"
        });
    let start = if state != "untimed" && end.is_some() {
        Some(start.unwrap_or(base))
    } else {
        start
    };
    let score = n
        .attribute((LL, "score"))
        .map(|v| v.parse().map_err(|_| "Invalid token score."))
        .transpose()?;
    Ok(Timing {
        start,
        end,
        state: state.into(),
        score,
    })
}
fn check(n: Node<'_, '_>, attrs: &[(&str, &str)], children: &[(&str, &str)]) -> Result<()> {
    if n.attributes()
        .any(|a| !attrs.contains(&(a.namespace().unwrap_or(""), a.name())))
        || n.children().filter(|c| c.is_element()).any(|c| {
            !children.contains(&(c.tag_name().namespace().unwrap_or(""), c.tag_name().name()))
        })
    {
        return Err(format!(
            "Unsupported field in TTML <{}>. This editor accepts a lyrics subset; import stopped to avoid losing styling, agents, translations or other metadata.",
            n.tag_name().name()
        ));
    }
    if n.attribute("timeContainer").is_some_and(|v| v != "par") {
        return Err("Sequential TTML containers are not supported. Convert them to parallel media timing first.".into());
    }
    Ok(())
}
fn child<'a, 'i>(n: Node<'a, 'i>, ns: &str, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| c.has_tag_name((ns, name)))
}
fn metadata(n: Node<'_, '_>) -> Result<Vec<(String, String)>> {
    let mut out = vec![];
    for m in n.children().filter(|c| c.has_tag_name((TT, "metadata"))) {
        check(
            m,
            &[],
            &[
                (TTM, "title"),
                (DC, "creator"),
                (DC, "description"),
                (DC, "source"),
                (DC, "license"),
                (DC, "contributor"),
            ],
        )?;
        for c in m.children().filter(|c| c.is_element()) {
            check(c, &[], &[])?;
            let key = format!(
                "{}:{}",
                c.tag_name().namespace().unwrap_or(""),
                c.tag_name().name()
            );
            if out.iter().any(|(k, _)| k == &key) {
                return Err("Multiple metadata values are not supported by this editor.".into());
            }
            out.push((key, c.text().unwrap_or("").into()));
        }
    }
    Ok(out)
}
pub fn from_xml(xml: &str) -> Result<Song> {
    if xml.len() > 4_000_000 {
        return Err("Song XML exceeds 4 MB.".into());
    }
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("Invalid XML: {e}"))?;
    let root = doc.root_element();
    if root.has_tag_name((crate::openlyrics::NS, "song")) {
        return crate::openlyrics::read(xml);
    }
    if !root.has_tag_name((TT, "tt")) {
        return Err("Open a TTML document (<tt> in the W3C namespace) or OpenLyrics song.".into());
    }
    check(
        root,
        &[(XML, "lang"), (TTP, "timeBase"), (LL, "version")],
        &[(TT, "head"), (TT, "body")],
    )?;
    if root
        .attribute((TTP, "timeBase"))
        .is_some_and(|v| v != "media")
    {
        return Err("LyricsLab supports the TTML media time base.".into());
    }
    if root.attribute((LL, "version")).is_some_and(|v| v != "1") {
        return Err("A newer LyricsLab TTML metadata profile is required.".into());
    }
    if root
        .children()
        .filter(|c| c.has_tag_name((TT, "body")))
        .count()
        != 1
        || root
            .children()
            .filter(|c| c.has_tag_name((TT, "head")))
            .count()
            > 1
    {
        return Err("TTML needs exactly one body and at most one head.".into());
    }
    let mut song = Song {
        language: root.attribute((XML, "lang")).unwrap_or("en").into(),
        ..Song::default()
    };
    if let Some(head) = child(root, TT, "head") {
        check(head, &[], &[(TT, "metadata")])?;
        let meta = metadata(head)?;
        for (k, v) in meta {
            match k.as_str() {
                k if k == format!("{TTM}:title") => song.title = v,
                k if k == format!("{DC}:creator") => song.artist = v,
                k if k == format!("{DC}:description") => song.recording = v,
                k if k == format!("{DC}:source") => song.source = v,
                k if k == format!("{DC}:license") => song.license = v,
                k if k == format!("{DC}:contributor") => song.contributor = v,
                _ => {}
            }
        }
    }
    let body = child(root, TT, "body").unwrap();
    check(
        body,
        &[
            ("", "begin"),
            ("", "end"),
            ("", "dur"),
            ("", "timeContainer"),
        ],
        &[(TT, "div")],
    )?;
    let (body_start, body_end) = bound(body, 0.0)?;
    song.duration = body_end;
    let body_base = body_start.unwrap_or(0.0);
    let mut line_count = 0;
    for div in body.children().filter(|c| c.is_element()) {
        check(
            div,
            &[
                (XML, "id"),
                ("", "begin"),
                ("", "end"),
                ("", "dur"),
                ("", "timeContainer"),
                (LL, "kind"),
                (TTM, "role"),
            ],
            &[(TT, "metadata"), (TT, "p")],
        )?;
        let (start, end) = bound(div, body_base)?;
        let base = start.unwrap_or(body_base);
        let id = div
            .attribute((XML, "id"))
            .map(str::to_string)
            .unwrap_or_else(|| format!("s{}", song.sections.len() + 1));
        let label = metadata(div)?
            .into_iter()
            .find(|(k, _)| k == &format!("{TTM}:title"))
            .map(|(_, v)| v)
            .unwrap_or_else(|| id.clone());
        let kind = div
            .attribute((LL, "kind"))
            .unwrap_or(if div.attribute((TTM, "role")) == Some("music") {
                "instrumental"
            } else {
                "verse"
            })
            .into();
        let mut section = Section {
            id,
            label,
            kind,
            start,
            end,
            lines: vec![],
        };
        for p in div.children().filter(|c| c.has_tag_name((TT, "p"))) {
            check(
                p,
                &[
                    (XML, "id"),
                    (XML, "space"),
                    ("", "begin"),
                    ("", "end"),
                    ("", "dur"),
                    ("", "timeContainer"),
                    (LL, "state"),
                    (LL, "score"),
                ],
                &[(TT, "span")],
            )?;
            line_count += 1;
            let t = timing(p, base)?;
            let word_base = t.start.unwrap_or(base);
            let mut text = String::new();
            let mut word_list = vec![];
            for n in p.children() {
                if n.is_text() {
                    text.push_str(n.text().unwrap_or(""));
                } else if n.is_element() {
                    check(
                        n,
                        &[
                            ("", "begin"),
                            ("", "end"),
                            ("", "dur"),
                            (LL, "state"),
                            (LL, "score"),
                        ],
                        &[],
                    )?;
                    let value = n.text().unwrap_or("");
                    if value.split_whitespace().count() != 1 || value.trim() != value {
                        return Err("Each word span must contain exactly one word; phrase and styling spans are not supported.".into());
                    }
                    text.push_str(value);
                    word_list.push(Word {
                        text: value.into(),
                        timing: timing(n, word_base)?,
                    });
                }
            }
            if p.attribute((XML, "space")) != Some("preserve") {
                text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            }
            if word_list.is_empty() {
                word_list = words(&text);
            }
            section.lines.push(Line {
                id: p
                    .attribute((XML, "id"))
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("l{line_count}")),
                text,
                timing: t,
                words: word_list,
            });
        }
        song.sections.push(section);
    }
    song.validate()?;
    Ok(song)
}
