use crate::{
    Result,
    document::{Song, Timing},
};
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write;

fn normalized(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
#[derive(Debug, Clone)]
struct Heard {
    text: String,
    start: f64,
    end: f64,
    score: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub section: usize,
    pub line: usize,
    pub word: usize,
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub score: f64,
    pub context: String,
}
fn recognition(text: &str) -> Result<Vec<Heard>> {
    if text.len() > 64_000_000 {
        return Err("Recognition JSON exceeds 64 MB.".into());
    }
    let root: Value =
        serde_json::from_str(text).map_err(|e| format!("Invalid Whisper JSON: {e}"))?;
    let segments = root["transcription"]
        .as_array()
        .ok_or("Expected whisper.cpp full JSON with a transcription array (-ojf).")?;
    let mut heard: Vec<Heard> = vec![];
    for seg in segments {
        let Some(tokens) = seg["tokens"].as_array() else {
            continue;
        };
        let mut current: Option<Heard> = None;
        for token in tokens {
            let t = token["text"].as_str().unwrap_or("");
            if t.starts_with("[_") || t.starts_with("<|") {
                continue;
            }
            let start = token["offsets"]["from"].as_f64().map(|v| v / 1000.0);
            let end = token["offsets"]["to"].as_f64().map(|v| v / 1000.0);
            let score = token["p"]
                .as_f64()
                .filter(|p| p.is_finite() && (0.0..=1.0).contains(p));
            let boundary = t.starts_with(char::is_whitespace);
            if boundary && let Some(w) = current.take() {
                heard.push(w);
            }
            if let Some(w) = &mut current {
                w.text.push_str(t);
                w.end = end.unwrap_or(-1.0);
                w.score = w.score.zip(score).map(|(a, b)| a.min(b));
            } else {
                current = Some(Heard {
                    text: t.trim().into(),
                    start: start.unwrap_or(-1.0),
                    end: end.unwrap_or(-1.0),
                    score,
                });
            }
        }
        if let Some(w) = current {
            heard.push(w);
        }
    }
    heard.retain(|w| !normalized(&w.text).is_empty() && w.start >= 0.0 && w.end > w.start);
    if heard.len() > 50000 {
        return Err("Recognition contains too many words.".into());
    }
    if heard.is_empty() {
        return Err("No word tokens with usable timestamps. Use whisper.cpp -ojf with token timestamps; scores are never assumed when absent.".into());
    }
    Ok(heard)
}
pub fn candidates(song: &Song, json: &str, threshold: f64) -> Result<Vec<Candidate>> {
    if !threshold.is_finite() || !(0.9..=1.0).contains(&threshold) {
        return Err("Anchor threshold must be 0.90–1.00.".into());
    }
    let heard = recognition(json)?;
    let mut lyrics = vec![];
    for (si, s) in song.sections.iter().enumerate() {
        for (li, l) in s.lines.iter().enumerate() {
            for (wi, w) in l.words.iter().enumerate() {
                lyrics.push((si, li, wi, normalized(&w.text)));
            }
        }
    }
    let mut proposals = std::collections::BTreeMap::<usize, Vec<(usize, f64)>>::new();
    for (h, hw) in heard.windows(3).enumerate() {
        if hw.iter().any(|w| w.score.is_none_or(|p| p < threshold))
            || hw
                .windows(2)
                .any(|w| w[1].start < w[0].end - 0.02 || w[1].start - w[0].end > 2.0)
        {
            continue;
        }
        let norm: Vec<_> = hw.iter().map(|w| normalized(&w.text)).collect();
        let matches: Vec<_> = lyrics
            .windows(3)
            .enumerate()
            .filter(|(_, lw)| {
                if lw[0].0 != lw[2].0 || lw.iter().zip(&norm).any(|(l, n)| &l.3 != n) {
                    return false;
                }
                let sec = &song.sections[lw[0].0];
                sec.start.is_none_or(|s| hw[0].start >= s)
                    && sec.end.is_none_or(|e| hw[2].end <= e)
                    && song.duration.is_none_or(|d| hw[2].end <= d)
            })
            .map(|(i, _)| i)
            .collect();
        if let [l] = matches.as_slice() {
            for (k, heard_word) in hw.iter().enumerate() {
                proposals
                    .entry(l + k)
                    .or_default()
                    .push((h + k, heard_word.score.unwrap()));
            }
        }
    }
    let mut selected = vec![];
    for (l, poss) in proposals {
        let h = poss[0].0;
        if poss.iter().any(|p| p.0 != h) {
            continue;
        }
        let (si, li, wi, _) = &lyrics[l];
        let word = &song.sections[*si].lines[*li].words[*wi];
        if word.timing.state != "untimed" {
            continue;
        }
        let t = &heard[h];
        // Every existing timed word is a fence; never relocate a repeated phrase across it.
        let compatible = lyrics.iter().enumerate().all(|(i, (s, line, w, _))| {
            let time = &song.sections[*s].lines[*line].words[*w].timing;
            if i < l {
                time.end.is_none_or(|e| t.start >= e)
            } else if i > l {
                time.start.is_none_or(|s| t.end <= s)
            } else {
                true
            }
        });
        if compatible {
            selected.push((l, h, poss.iter().map(|p| p.1).fold(1.0, f64::min)));
        }
    }
    // Conflicting recognition order invalidates both candidates, rather than choosing one arbitrarily.
    let mut out = vec![];
    for &(l, h, score) in &selected {
        if selected.iter().any(|&(ol, oh, _)| {
            (ol < l && heard[oh].end > heard[h].start) || (ol > l && heard[oh].start < heard[h].end)
        }) {
            continue;
        }
        let (si, li, wi, _) = &lyrics[l];
        let line = &song.sections[*si].lines[*li];
        let t = &heard[h];
        if line.timing.start.is_some_and(|s| t.start < s)
            || line.timing.end.is_some_and(|e| t.end > e)
        {
            continue;
        }
        out.push(Candidate {
            section: *si,
            line: *li,
            word: *wi,
            text: line.words[*wi].text.clone(),
            start: t.start,
            end: t.end,
            score,
            context: format!("{} · {}", song.sections[*si].label, line.text),
        });
    }
    Ok(out)
}
pub fn estimate(song: &mut Song, index: usize) -> Result<()> {
    // Operate on a copy so errors cannot leave partial estimates in the caller's document.
    let mut proposed = song.clone();
    let sec = proposed
        .sections
        .get_mut(index)
        .ok_or("Choose a section.")?;
    let (start, end) = sec
        .start
        .zip(sec.end)
        .ok_or("Set both section start and end before estimating.")?;
    let count: usize = sec.lines.iter().map(|l| l.words.len()).sum();
    if count == 0 {
        return Err("This section has no lyrics.".into());
    }
    let mut starts = vec![None; count];
    let mut ends = vec![None; count];
    let mut weights = vec![];
    let mut offset = 0;
    for line in &sec.lines {
        for word in &line.words {
            weights.push(word.text.chars().count().max(1) as f64);
            if word.timing.state == "confirmed" {
                starts[offset] = word.timing.start;
                ends[offset] = word.timing.end;
            }
            offset += 1;
        }
        if line.timing.state == "confirmed" {
            let first = offset - line.words.len();
            starts[first] = starts[first].or(line.timing.start);
            ends[offset - 1] = ends[offset - 1].or(line.timing.end);
        }
    }
    starts[0] = starts[0].or(Some(start));
    ends[count - 1] = ends[count - 1].or(Some(end));
    let points: Vec<_> = (0..=count)
        .filter(|&i| (i < count && starts[i].is_some()) || (i > 0 && ends[i - 1].is_some()))
        .collect();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let ta = starts[a]
            .or(if a > 0 { ends[a - 1] } else { None })
            .unwrap();
        let tb = ends[b - 1]
            .or(if b < count { starts[b] } else { None })
            .unwrap();
        if tb <= ta {
            return Err("Anchors leave no duration for intervening words.".into());
        }
        let total: f64 = weights[a..b].iter().sum();
        let mut t = ta;
        for i in a..b {
            starts[i] = Some(t);
            t += (tb - ta) * weights[i] / total;
            ends[i] = Some(if i + 1 == b { tb } else { t });
        }
    }
    offset = 0;
    for line in &mut sec.lines {
        for word in &mut line.words {
            if word.timing.state != "confirmed" {
                word.timing = Timing {
                    start: starts[offset],
                    end: ends[offset],
                    state: "estimated".into(),
                    score: None,
                };
            }
            offset += 1;
        }
        if line.timing.state != "confirmed" {
            line.timing = Timing {
                start: line.words.first().and_then(|w| w.timing.start),
                end: line.words.last().and_then(|w| w.timing.end),
                state: "estimated".into(),
                score: None,
            };
        }
    }
    proposed.validate()?;
    *song = proposed;
    Ok(())
}
fn stamp(seconds: f64, vtt: bool) -> String {
    let ms = (seconds * 1000.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02}{}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        if vtt { '.' } else { ',' },
        ms % 1000
    )
}
pub fn subtitles(song: &Song, format: &str) -> Result<String> {
    song.validate()?;
    if !matches!(format, "srt" | "vtt") {
        return Err("Choose SRT or WebVTT.".into());
    }
    let vtt = format == "vtt";
    let mut out = if vtt {
        "WEBVTT\n\n".into()
    } else {
        String::new()
    };
    let mut count = 0;
    for s in &song.sections {
        for l in &s.lines {
            if l.timing.state != "confirmed" {
                return Err(format!(
                    "Confirm line {} before exporting: {}",
                    l.id, l.text
                ));
            }
            let (a, b) = l
                .timing
                .start
                .zip(l.timing.end)
                .ok_or("Line timing is missing.")?;
            if (b * 1000.0).round() <= (a * 1000.0).round() {
                return Err("A subtitle is shorter than one millisecond.".into());
            }
            count += 1;
            if !vtt {
                writeln!(out, "{count}").unwrap();
            }
            let text = l
                .text
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            write!(out, "{} --> {}\n{text}\n\n", stamp(a, vtt), stamp(b, vtt)).unwrap();
        }
    }
    if count == 0 {
        return Err("No lyric lines to export.".into());
    }
    Ok(out)
}
