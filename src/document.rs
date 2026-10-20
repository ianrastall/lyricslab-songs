use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Timing {
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub state: String,
    pub score: Option<f64>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Word {
    pub text: String,
    pub timing: Timing,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Line {
    pub id: String,
    pub text: String,
    pub timing: Timing,
    pub words: Vec<Word>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Section {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub lines: Vec<Line>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Song {
    pub version: u32,
    pub title: String,
    pub artist: String,
    pub language: String,
    pub recording: String,
    pub duration: Option<f64>,
    pub source: String,
    pub license: String,
    pub contributor: String,
    pub sections: Vec<Section>,
}
impl Default for Song {
    fn default() -> Self {
        Self {
            version: 2,
            title: String::new(),
            artist: String::new(),
            language: "en".into(),
            recording: String::new(),
            duration: None,
            source: String::new(),
            license: String::new(),
            contributor: String::new(),
            sections: vec![],
        }
    }
}
pub fn empty_timing() -> Timing {
    Timing {
        state: "untimed".into(),
        ..Timing::default()
    }
}
pub fn words(text: &str) -> Vec<Word> {
    text.split_whitespace()
        .map(|w| Word {
            text: w.into(),
            timing: empty_timing(),
        })
        .collect()
}
fn xml_text(s: &str) -> bool {
    s.chars().all(|c| matches!(c, '\t'|'\n'|'\r'|'\u{20}'..='\u{d7ff}'|'\u{e000}'..='\u{fffd}'|'\u{10000}'..='\u{10ffff}'))
}
pub fn valid_time(v: Option<f64>) -> bool {
    v.is_none_or(|x| x.is_finite() && (0.0..=86400.0).contains(&x))
}
fn range(start: Option<f64>, end: Option<f64>) -> Result<()> {
    if !valid_time(start) || !valid_time(end) || start.zip(end).is_some_and(|(a, b)| a >= b) {
        return Err("Times must be finite seconds from 0 to 86400, with end after start.".into());
    }
    Ok(())
}
fn timing(t: &Timing) -> Result<()> {
    range(t.start, t.end)?;
    if !matches!(
        t.state.as_str(),
        "untimed" | "suggested" | "estimated" | "confirmed"
    ) {
        return Err("Unknown timing state.".into());
    }
    if (t.state == "untimed" && (t.start.is_some() || t.end.is_some()))
        || (t.state != "untimed" && (t.start.is_none() || t.end.is_none()))
    {
        return Err("Timed entries need both start and end; untimed entries need neither.".into());
    }
    if t.score
        .is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(&p))
    {
        return Err("Model score must be between 0 and 1.".into());
    }
    Ok(())
}
fn inside(a: Option<f64>, b: Option<f64>, start: Option<f64>, end: Option<f64>) -> Result<()> {
    if a.zip(start).is_some_and(|(a, s)| a + 0.000001 < s)
        || b.zip(end).is_some_and(|(b, e)| b > e + 0.000001)
    {
        return Err("A timing extends outside its section, line, or recording.".into());
    }
    Ok(())
}
impl Song {
    pub fn validate(&self) -> Result<()> {
        if self.version != 2 {
            return Err("Unsupported document model version. Reopen the TTML file with a current LyricsLab version.".into());
        }
        if self.sections.is_empty() || self.sections.len() > 200 {
            return Err("A song needs 1–200 sections.".into());
        }
        if !valid_time(self.duration) || self.duration == Some(0.0) {
            return Err("Invalid recording duration.".into());
        }
        let meta = [
            &self.title,
            &self.artist,
            &self.language,
            &self.recording,
            &self.source,
            &self.license,
            &self.contributor,
        ];
        if meta.iter().any(|s| s.len() > 4096 || !xml_text(s)) {
            return Err("Metadata is too long or contains invalid XML characters.".into());
        }
        let mut ids = HashSet::new();
        let mut count = 0;
        let mut previous_section_end = None;
        let mut previous_line_end = None;
        let mut previous_word_end = None;
        for (section_index, s) in self.sections.iter().enumerate() {
            let section_floor = s.start.or(previous_section_end);
            let section_ceiling = s.end.or_else(|| {
                self.sections[section_index + 1..]
                    .iter()
                    .find_map(|s| s.start)
            });
            if !s
                .id
                .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                || s.id.len() > 128
                || !s
                    .id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                || !ids.insert(&s.id)
            {
                return Err("Section and line IDs must be unique ASCII letters, digits, underscores or hyphens.".into());
            }
            if s.kind.trim().is_empty()
                || s.kind.len() > 128
                || s.label.len() > 512
                || !xml_text(&s.kind)
                || !xml_text(&s.label)
            {
                return Err("Invalid section name.".into());
            }
            range(s.start, s.end)?;
            inside(s.start, s.end, Some(0.0), self.duration)?;
            if s.start
                .zip(previous_section_end)
                .is_some_and(|(a, b)| a < b - 0.000001)
            {
                return Err("Sections overlap or are out of order.".into());
            }
            if s.end.is_some() {
                previous_section_end = s.end;
            }
            if s.kind.eq_ignore_ascii_case("instrumental") && !s.lines.is_empty() {
                return Err("Instrumental sections cannot contain lyric lines.".into());
            }
            for l in &s.lines {
                count += 1;
                if !l
                    .id
                    .starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                    || l.id.len() > 128
                    || !l
                        .id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    || !ids.insert(&l.id)
                {
                    return Err("Duplicate or invalid line ID.".into());
                }
                if l.text.trim().is_empty()
                    || l.text.len() > 4096
                    || !xml_text(&l.text)
                    || l.text.contains(['\n', '\r'])
                {
                    return Err(
                        "Each lyric line must contain plain text on one line (maximum 4096 bytes)."
                            .into(),
                    );
                }
                if l.text
                    .split_whitespace()
                    .ne(l.words.iter().map(|w| w.text.as_str()))
                {
                    return Err("Word references must match the lyric text exactly.".into());
                }
                timing(&l.timing)?;
                inside(l.timing.start, l.timing.end, section_floor, section_ceiling)?;
                inside(l.timing.start, l.timing.end, None, self.duration)?;
                if l.timing
                    .start
                    .zip(previous_line_end)
                    .is_some_and(|(a, b)| a < b - 0.000001)
                {
                    return Err("Lyric lines overlap or are out of order.".into());
                }
                if l.timing.end.is_some() {
                    previous_line_end = l.timing.end;
                }
                for w in &l.words {
                    timing(&w.timing)?;
                    inside(w.timing.start, w.timing.end, section_floor, section_ceiling)?;
                    inside(w.timing.start, w.timing.end, l.timing.start, l.timing.end)?;
                    inside(w.timing.start, w.timing.end, None, self.duration)?;
                    if w.timing
                        .start
                        .zip(previous_word_end)
                        .is_some_and(|(a, b)| a < b - 0.000001)
                    {
                        return Err("Word timings overlap or are out of order.".into());
                    }
                    if w.timing.end.is_some() {
                        previous_word_end = w.timing.end;
                    }
                }
            }
        }
        let word_count: usize = self
            .sections
            .iter()
            .flat_map(|s| &s.lines)
            .map(|l| l.words.len())
            .sum();
        let text_bytes: usize = self
            .sections
            .iter()
            .flat_map(|s| &s.lines)
            .map(|l| l.text.len())
            .sum();
        if text_bytes > 1_000_000 {
            return Err("Combined lyric text exceeds 1 MB.".into());
        }
        if count > 2000 || word_count > 10000 {
            return Err("Song exceeds 2000 lines or 10000 words.".into());
        }
        Ok(())
    }
}
pub fn from_text(text: &str) -> Result<Song> {
    if text.len() > 1_000_000 {
        return Err("Lyrics text exceeds 1 MB.".into());
    }
    let mut song = Song::default();
    let mut section: Option<Section> = None;
    let mut line_id = 0;
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    for raw in normalized.trim_start_matches('\u{feff}').lines() {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }
        if t.starts_with('[') && t.ends_with(']') {
            if let Some(s) = section.take() {
                song.sections.push(s);
            }
            let label = t[1..t.len() - 1].trim().to_string();
            let kind = if label.to_lowercase().contains("instrumental")
                || label.to_lowercase().contains("solo")
            {
                "instrumental".into()
            } else {
                label
                    .split_whitespace()
                    .next()
                    .unwrap_or("verse")
                    .to_lowercase()
            };
            section = Some(Section {
                id: format!("s{}", song.sections.len() + 1),
                kind,
                label,
                ..Section::default()
            });
        } else {
            let s = section.get_or_insert_with(|| Section {
                id: format!("s{}", song.sections.len() + 1),
                kind: "verse".into(),
                label: "Verse".into(),
                ..Section::default()
            });
            line_id += 1;
            s.lines.push(Line {
                id: format!("l{line_id}"),
                text: t.into(),
                words: words(t),
                timing: empty_timing(),
            });
        }
    }
    if let Some(s) = section {
        song.sections.push(s);
    }
    song.validate()?;
    Ok(song)
}
pub fn escaped(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
        .replace('\r', "&#13;")
        .replace('\n', "&#10;")
        .replace('\t', "&#9;")
}

pub use crate::ttml::{from_xml, to_xml};
