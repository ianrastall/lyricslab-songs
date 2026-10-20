//! Untimed OpenLyrics 0.8/0.9 exchange. Unsupported expressive features are rejected.
use crate::{
    Result,
    document::{Line, Section, Song, empty_timing, escaped, words},
};
use std::{collections::HashMap, fmt::Write};
pub const NS: &str = "http://openlyrics.info/namespace/2009/song";
fn check(n: roxmltree::Node<'_, '_>, attrs: &[&str], children: &[&str]) -> Result<()> {
    if n.attributes()
        .any(|a| a.namespace().is_some() || !attrs.contains(&a.name()))
        || n.children().filter(|c| c.is_element()).any(|c| {
            c.tag_name().namespace() != Some(NS) || !children.contains(&c.tag_name().name())
        })
    {
        return Err(format!(
            "Unsupported OpenLyrics field in <{}>. Chords, part assignments, translation variants and extra metadata cannot be imported without losing information.",
            n.tag_name().name()
        ));
    }
    Ok(())
}
pub fn read(xml: &str) -> Result<Song> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    if !root.has_tag_name((NS, "song")) || !matches!(root.attribute("version"), Some("0.8" | "0.9"))
    {
        return Err("Expected OpenLyrics 0.8 or 0.9.".into());
    }
    for a in root.attributes() {
        if !((a.namespace().is_none()
            && ["version", "createdIn", "modifiedIn", "modifiedDate"].contains(&a.name()))
            || (a.namespace() == Some(crate::ttml::XML) && a.name() == "lang"))
        {
            return Err("Unsupported OpenLyrics root metadata.".into());
        }
    }
    let mut song = Song {
        language: root
            .attribute((crate::ttml::XML, "lang"))
            .unwrap_or("en")
            .into(),
        ..Song::default()
    };
    if root
        .children()
        .filter(|c| c.is_element())
        .any(|c| !c.has_tag_name((NS, "properties")) && !c.has_tag_name((NS, "lyrics")))
    {
        return Err("Unsupported OpenLyrics song element.".into());
    }
    let prop: Vec<_> = root
        .children()
        .filter(|c| c.has_tag_name((NS, "properties")))
        .collect();
    let lyrics: Vec<_> = root
        .children()
        .filter(|c| c.has_tag_name((NS, "lyrics")))
        .collect();
    if prop.len() != 1 || lyrics.len() != 1 {
        return Err("OpenLyrics needs one properties and one lyrics element.".into());
    }
    check(
        prop[0],
        &[],
        &["titles", "authors", "copyright", "verseOrder"],
    )?;
    let mut seen = std::collections::HashSet::new();
    let mut order = None;
    for p in prop[0].children().filter(|c| c.is_element()) {
        if !seen.insert(p.tag_name().name()) {
            return Err("Duplicate OpenLyrics property.".into());
        }
        match p.tag_name().name() {
            "titles" => {
                check(p, &[], &["title"])?;
                let titles: Vec<_> = p.children().filter(|c| c.is_element()).collect();
                if titles.len() != 1 {
                    return Err("This editor imports exactly one title; translation titles are unsupported.".into());
                }
                check(titles[0], &[], &[])?;
                song.title = titles[0].text().unwrap_or("").into();
            }
            "authors" => {
                check(p, &[], &["author"])?;
                let mut names = vec![];
                for a in p.children().filter(|c| c.is_element()) {
                    check(a, &["type"], &[])?;
                    if a.attribute("type").is_some_and(|t| t != "words") {
                        return Err(
                            "Only untyped or lyric-word authors can be represented by this editor."
                                .into(),
                        );
                    }
                    names.push(a.text().unwrap_or(""));
                }
                song.artist = names.join("; ");
            }
            "copyright" => {
                check(p, &[], &[])?;
                song.license = p.text().unwrap_or("").into();
            }
            "verseOrder" => {
                check(p, &[], &[])?;
                order = Some(
                    p.text()
                        .unwrap_or("")
                        .split_whitespace()
                        .map(str::to_string)
                        .collect::<Vec<_>>(),
                );
            }
            _ => {}
        }
    }
    if song.title.is_empty() {
        return Err("OpenLyrics requires a title.".into());
    }
    check(lyrics[0], &[], &["verse"])?;
    let mut definitions = HashMap::new();
    let mut document_order = vec![];
    for verse in lyrics[0].children().filter(|c| c.is_element()) {
        check(verse, &["name"], &["lines"])?;
        let name = verse
            .attribute("name")
            .ok_or("OpenLyrics verse needs a name.")?
            .to_string();
        let mut lines = vec![];
        for block in verse.children().filter(|c| c.is_element()) {
            check(block, &[], &["br"])?;
            let mut text = String::new();
            for n in block.children() {
                if n.is_text() {
                    text.push_str(n.text().unwrap_or(""));
                } else if n.is_element() {
                    check(n, &[], &[])?;
                    text.push('\n');
                }
            }
            for line in text.split('\n') {
                let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
                if !line.is_empty() {
                    lines.push(line);
                }
            }
        }
        if lines.is_empty() || definitions.insert(name.clone(), lines).is_some() {
            return Err("Empty or duplicate OpenLyrics verse definition.".into());
        }
        document_order.push(name);
    }
    let order = order.unwrap_or(document_order);
    if order.len() > 200 {
        return Err("OpenLyrics order exceeds 200 occurrences.".into());
    }
    let mut line_id = 0;
    for name in order {
        let lines = definitions
            .get(&name)
            .ok_or_else(|| format!("Unknown verse in verseOrder: {name}"))?;
        let kind = match name.chars().next() {
            Some('c') => "chorus",
            Some('p') => "pre-chorus",
            Some('b') => "bridge",
            Some('i') => "intro",
            Some('e') => "outro",
            Some('v') => "verse",
            _ => "other",
        };
        song.sections.push(Section {
            id: format!("s{}", song.sections.len() + 1),
            kind: kind.into(),
            label: name,
            start: None,
            end: None,
            lines: lines
                .iter()
                .map(|text| {
                    line_id += 1;
                    Line {
                        id: format!("l{line_id}"),
                        text: text.clone(),
                        timing: empty_timing(),
                        words: words(text),
                    }
                })
                .collect(),
        });
    }
    song.validate()?;
    Ok(song)
}
pub fn write(song: &Song) -> Result<String> {
    song.validate()?;
    if song.title.trim().is_empty() {
        return Err("Enter a title before exporting OpenLyrics.".into());
    }
    let mut definitions: Vec<(String, String, Vec<String>)> = vec![];
    let mut order = vec![];
    let mut counters = HashMap::new();
    for sec in &song.sections {
        if sec.kind.eq_ignore_ascii_case("instrumental") || sec.lines.is_empty() {
            continue;
        }
        let lines: Vec<_> = sec.lines.iter().map(|l| l.text.clone()).collect();
        if let Some((name, _, _)) = definitions
            .iter()
            .find(|(_, kind, text)| kind == &sec.kind && text == &lines)
        {
            order.push(name.clone());
            continue;
        }
        let prefix = match sec.kind.as_str() {
            "verse" => "v",
            "chorus" => "c",
            "pre-chorus" => "p",
            "bridge" => "b",
            "intro" => "i",
            "outro" | "ending" => "e",
            _ => "o",
        };
        let count = counters.entry(prefix).or_insert(0);
        *count += 1;
        let name = format!("{prefix}{count}");
        order.push(name.clone());
        definitions.push((name, sec.kind.clone(), lines));
    }
    if definitions.is_empty() {
        return Err("No vocal sections to export as OpenLyrics.".into());
    }
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<song xmlns=\"{NS}\" version=\"0.9\" createdIn=\"LyricsLab 0.1.0\" xml:lang=\"{}\">\n  <properties>\n    <titles><title>{}</title></titles>\n    <verseOrder>{}</verseOrder>\n  </properties>\n  <lyrics>\n",
        escaped(&song.language),
        escaped(&song.title),
        order.join(" ")
    );
    for (name, _, lines) in definitions {
        writeln!(
            out,
            "    <verse name=\"{name}\"><lines>{}</lines></verse>",
            lines
                .iter()
                .map(|t| escaped(t))
                .collect::<Vec<_>>()
                .join("<br/>")
        )
        .unwrap();
    }
    out.push_str("  </lyrics>\n</song>\n");
    Ok(out)
}
