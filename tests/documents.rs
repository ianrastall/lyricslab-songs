use lyricslab_songs::{
    document::{self, Timing},
    openlyrics, timing, ttml,
};
fn timed() -> document::Song {
    let mut s=document::from_text("[Verse 1]\nMorning light across the floor\nOpen windows welcome more\n[Instrumental]\n[Chorus]\nAnother quiet day begins").unwrap();
    s.title = "Morning & light".into();
    s.artist = "Example author".into();
    s.duration = Some(20.0);
    s.recording = "Demo".into();
    s.source = "Original example".into();
    s.license = "CC0-1.0".into();
    s.contributor = "MediaLab".into();
    s.sections[0].start = Some(2.0);
    s.sections[0].end = Some(10.0);
    s.sections[1].start = Some(10.0);
    s.sections[1].end = Some(14.0);
    s.sections[2].start = Some(14.0);
    s.sections[2].end = Some(20.0);
    timing::estimate(&mut s, 0).unwrap();
    timing::estimate(&mut s, 2).unwrap();
    s
}
fn recognition(words: &[(&str, f64, f64, Option<f64>)]) -> String {
    serde_json::json!({"transcription":[{"tokens":words.iter().map(|(w,a,b,p)|serde_json::json!({"text":format!(" {w}"),"offsets":{"from":a*1000.0,"to":b*1000.0},"p":p})).collect::<Vec<_>>()}]}).to_string()
}
#[test]
fn ttml_roundtrip_keeps_sections_review_and_relative_times() {
    let mut s = timed();
    s.sections[0].lines[0].words[0].timing.score = Some(0.99);
    let xml = document::to_xml(&s).unwrap();
    assert!(xml.contains("<tt xmlns=\"http://www.w3.org/ns/ttml\""));
    assert!(xml.contains("<div xml:id=\"s1\""));
    assert!(xml.contains("begin=\"0.000000s\""));
    let read = document::from_xml(&xml).unwrap();
    assert_eq!(read.sections[0].start, Some(2.0));
    assert!(
        (read.sections[0].lines[1].words[1].timing.start.unwrap()
            - s.sections[0].lines[1].words[1].timing.start.unwrap())
        .abs()
            < 0.000002
    );
    assert_eq!(read.sections[0].lines[0].timing.state, "estimated");
    assert_eq!(read.sections[0].lines[0].words[0].timing.score, Some(0.99));
    assert_eq!(read.title, s.title);
    assert_eq!(read.recording, s.recording);
    assert_eq!(read.license, s.license);
    assert_eq!(read.sections[1].kind, "instrumental");
}
#[test]
fn untimed_ttml_does_not_invent_times() {
    let s = document::from_text("[Chorus]\nWe are here\n[Chorus]\nWe are here").unwrap();
    let r = document::from_xml(&document::to_xml(&s).unwrap()).unwrap();
    assert_eq!(s, r);
}
#[test]
fn whitespace_and_unicode_are_lossless() {
    let mut s = document::from_text("星 & moon <above>").unwrap();
    s.sections[0].lines[0].text = "星  &\tmoon <above>".into();
    let r = document::from_xml(&document::to_xml(&s).unwrap()).unwrap();
    assert_eq!(r, s);
}
#[test]
fn external_ttml_offsets_and_durations_are_suggestions() {
    let x = format!(
        r#"<tt xmlns="{}"><body begin="1s" end="30s"><div begin="2s" dur="10s"><p begin="1s" dur="2s">A quiet morning</p></div></body></tt>"#,
        ttml::TT
    );
    let s = document::from_xml(&x).unwrap();
    assert_eq!(s.sections[0].start, Some(3.0));
    assert_eq!(s.sections[0].lines[0].timing.start, Some(4.0));
    assert_eq!(s.sections[0].lines[0].timing.end, Some(6.0));
    assert_eq!(s.sections[0].lines[0].timing.state, "suggested");
}
#[test]
fn reject_unsupported_ttml_instead_of_losing_data() {
    for fragment in [
        r#"<p style="bold">Hi</p>"#,
        r#"<p><span>Two words</span></p>"#,
        r#"<p begin="NaNs">Hi</p>"#,
    ] {
        let x = format!(
            "<tt xmlns=\"{}\"><body><div>{fragment}</div></body></tt>",
            ttml::TT
        );
        assert!(document::from_xml(&x).is_err());
    }
    assert!(document::from_xml(r#"<!DOCTYPE tt [<!ENTITY x "hello">]><tt xmlns="http://www.w3.org/ns/ttml"><body><div><p>&x;</p></div></body></tt>"#).is_err());
}
#[test]
fn clocks_and_units() {
    assert_eq!(ttml::time("01:02:03.5").unwrap(), 3723.5);
    assert_eq!(ttml::time("1500ms").unwrap(), 1.5);
    for x in ["NaNs", "-1s", "00:70:00", "00:00:00:12", "4f", "infh"] {
        assert!(ttml::time(x).is_err());
    }
}
#[test]
fn windows_textbox_carriage_returns_split_lines() {
    let s = document::from_text("[Verse 1]\rFirst line\rSecond line").unwrap();
    assert_eq!(s.sections.len(), 1);
    assert_eq!(s.sections[0].lines.len(), 2);
}
#[test]
fn lyric_timings_cannot_cross_an_instrumental_without_section_bounds() {
    let mut s = timed();
    s.sections[2].start = None;
    s.sections[2].end = None;
    let l = &mut s.sections[2].lines[0];
    l.timing.start = Some(12.0);
    l.timing.end = Some(16.0);
    for w in &mut l.words {
        w.timing = document::empty_timing();
    }
    assert!(s.validate().is_err());
}
#[test]
fn estimates_preserve_confirmed_anchors_and_silent_gaps() {
    let mut s = document::from_text("a b c d").unwrap();
    s.sections[0].start = Some(0.0);
    s.sections[0].end = Some(10.0);
    for (i, a, b) in [(1, 3.0, 4.0), (2, 6.0, 7.0)] {
        s.sections[0].lines[0].words[i].timing = Timing {
            start: Some(a),
            end: Some(b),
            state: "confirmed".into(),
            score: None,
        };
    }
    timing::estimate(&mut s, 0).unwrap();
    let w = &s.sections[0].lines[0].words;
    assert_eq!(w[0].timing.end, Some(3.0));
    assert_eq!(w[1].timing.end, Some(4.0));
    assert_eq!(w[2].timing.start, Some(6.0));
    assert_eq!(w[3].timing.start, Some(7.0));
    assert_eq!(w[1].timing.state, "confirmed");
}
#[test]
fn estimate_requires_section_bounds_and_is_transactional() {
    let mut s = timed();
    s.sections[0].end = None;
    let before = s.clone();
    assert!(timing::estimate(&mut s, 0).is_err());
    assert_eq!(s, before);
    assert!(timing::estimate(&mut s, 1).is_err());
}
#[test]
fn rejects_overlap_invalid_words_and_nan() {
    let s = timed();
    let mut bad = s.clone();
    bad.sections[2].start = Some(9.0);
    assert!(bad.validate().is_err());
    let mut bad = s.clone();
    bad.sections[0].lines[0].words[0].text = "changed".into();
    assert!(bad.validate().is_err());
    let mut bad = s;
    bad.duration = Some(f64::NAN);
    assert!(bad.validate().is_err());
}
#[test]
fn exports_only_confirmed_lines_and_no_instrumental_captions() {
    let mut s = timed();
    assert!(timing::subtitles(&s, "srt").is_err());
    for l in s.sections.iter_mut().flat_map(|s| &mut s.lines) {
        l.timing.state = "confirmed".into();
    }
    let srt = timing::subtitles(&s, "srt").unwrap();
    assert!(srt.starts_with("1\n00:00:02,000 -->"));
    assert!(!srt.contains("Instrumental"));
    assert_eq!(srt.matches(" --> ").count(), 3);
    assert!(
        timing::subtitles(&s, "vtt")
            .unwrap()
            .starts_with("WEBVTT\n\n00:00:02.000")
    );
}
#[test]
fn missing_or_low_scores_never_create_anchors() {
    let s = document::from_text("Morning light across the floor").unwrap();
    for score in [None, Some(0.8)] {
        let json = recognition(&[
            ("Morning", 1., 2., score),
            ("light", 2., 3., score),
            ("across", 3., 4., score),
        ]);
        assert!(timing::candidates(&s, &json, 0.95).unwrap().is_empty());
    }
}
#[test]
fn strong_unique_phrase_creates_anchors_but_repetitions_are_ambiguous() {
    let s = document::from_text("Morning light across the floor").unwrap();
    let json = recognition(&[
        ("Morning", 1., 2., Some(0.99)),
        ("light", 2., 3., Some(0.98)),
        ("across", 3., 4., Some(0.97)),
    ]);
    let c = timing::candidates(&s, &json, 0.95).unwrap();
    assert_eq!(c.len(), 3);
    assert_eq!(c[0].score, 0.99);
    let mut repeated =
        document::from_text("[Chorus]\nMorning light across\n[Chorus]\nMorning light across")
            .unwrap();
    assert!(
        timing::candidates(&repeated, &json, 0.95)
            .unwrap()
            .is_empty()
    );
    repeated.sections[0].start = Some(0.0);
    repeated.sections[0].end = Some(5.0);
    repeated.sections[1].start = Some(10.0);
    repeated.sections[1].end = Some(15.0);
    assert_eq!(timing::candidates(&repeated, &json, 0.95).unwrap().len(), 3);
}
#[test]
fn conflicting_recognition_order_is_rejected() {
    let s = document::from_text("one two three four five six").unwrap();
    let json = recognition(&[
        ("four", 1., 2., Some(0.99)),
        ("five", 2., 3., Some(0.99)),
        ("six", 3., 4., Some(0.99)),
        ("one", 5., 6., Some(0.99)),
        ("two", 6., 7., Some(0.99)),
        ("three", 7., 8., Some(0.99)),
    ]);
    assert!(timing::candidates(&s, &json, 0.95).unwrap().is_empty());
}
#[test]
fn openlyrics_expands_order_and_exports_valid_names() {
    let x = format!(
        r#"<song xmlns="{}" version="0.9"><properties><titles><title>Example</title></titles><verseOrder>v1 c1 c1</verseOrder></properties><lyrics><verse name="v1"><lines>first line<br/>second line</lines></verse><verse name="c1"><lines>sing again</lines></verse></lyrics></song>"#,
        openlyrics::NS
    );
    let s = document::from_xml(&x).unwrap();
    assert_eq!(s.sections.len(), 3);
    assert_eq!(s.sections[0].lines.len(), 2);
    assert_eq!(s.sections[1].lines[0].timing.state, "untimed");
    let out = openlyrics::write(&s).unwrap();
    assert!(out.contains("<verseOrder>v1 c1 c1</verseOrder>"));
    assert_eq!(document::from_xml(&out).unwrap().sections.len(), 3);
    assert!(document::from_xml(&x.replace("first line", "<chord root=\"C\"/>first line")).is_err());
}

