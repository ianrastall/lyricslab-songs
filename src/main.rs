//! Repository validator and release-smoke helper. Not shipped as part of the GUI.
use lyricslab_songs::{document,timing};
fn main(){if let Err(e)=run(){eprintln!("{e}");std::process::exit(1);}}
fn run()->Result<(),String>{
 let args:Vec<_>=std::env::args().skip(1).collect();
 if args.first().map(String::as_str)==Some("--demo"){
  let mut song=document::from_text("[Verse 1]\nMorning light across the floor\nOpen windows welcome more\n[Instrumental]\n[Chorus]\nAnother quiet day begins")?;
  song.title="Morning Light (original format example)".into();song.artist="MediaLab example".into();song.recording="Timing demonstration; no associated recording".into();song.duration=Some(20.0);song.source="Original demonstration text created for LyricsLab".into();song.license="CC0-1.0".into();song.contributor="MediaLab contributors".into();
  song.sections[0].start=Some(2.0);song.sections[0].end=Some(10.0);song.sections[1].start=Some(10.0);song.sections[1].end=Some(14.0);song.sections[2].start=Some(14.0);song.sections[2].end=Some(20.0);
  timing::estimate(&mut song,0)?;timing::estimate(&mut song,2)?;
  for l in song.sections.iter_mut().flat_map(|s|&mut s.lines){l.timing.state="confirmed".into();}
  let xml=document::to_xml(&song)?;std::fs::write(args.get(1).ok_or("Supply demo output path.")?,xml).map_err(|e|e.to_string())?;return Ok(());
 }
 if args.is_empty(){return Err("Supply TTML paths or --demo output.ttml".into());}
 for path in args{
  let xml=std::fs::read_to_string(&path).map_err(|e|e.to_string())?;let song=document::from_xml(&xml)?;
  if song.title.is_empty()||song.source.is_empty()||song.license.is_empty()||song.contributor.is_empty(){return Err(format!("{path}: title, source, rights/license, and contributor are required for submissions."));}
  let timed=song.sections.iter().any(|s|s.start.is_some()||s.end.is_some()||s.lines.iter().any(|l|l.timing.state!="untimed"||l.words.iter().any(|w|w.timing.state!="untimed")));
  if timed&&(song.recording.is_empty()||song.duration.is_none()){return Err(format!("{path}: timed submissions require recording/edition and duration."));}
  println!("OK {path}");
 }Ok(())
}
