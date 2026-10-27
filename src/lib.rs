//! Write Ableton Live 12 sets (`.als`) whose MIDI clips carry per-note pitch
//! expression (MPE).
//!
//! Live's MIDI file import drops MPE, so a set is the way to hand Live notes
//! with their own pitch curves. `.als` is gzipped, undocumented,
//! version-specific XML. Rather than hand-author it (and risk a file Live
//! won't open), we clone a real Live 12.4 set ([`TEMPLATE`], an empty set
//! with one MIDI track carrying a Pitch device inside a MIDI Effect Rack) and
//! inject the data:
//!
//! * one `<MidiTrack>` per [`Track`], cloned from a template track, named, with
//!   all `Id="…"` offset per track so they stay globally unique;
//! * one Session clip slot per [`Scene`], plus an optional Arrangement clip;
//! * the master tempo and its automation;
//! * optionally, a loaded tuning system ([`write_tuned`], [`TuningSystem`]).
//!
//! Version-specific: this targets Live 12.4 (the template was saved by
//! 12.4.6). Re-template on upgrade.

use std::io::Write;

use flate2::{write::GzEncoder, Compression};

mod tuning;
pub use tuning::{Anchor, NotePosition, NoteTuning, Ratio, TuningSystem, MIDI_NOTES};

/// A minimal Live 12.4 set with one MIDI track (Pitch-in-rack, no
/// instrument), no return tracks and an empty groove pool. Pass it to
/// [`write`] when you have no template of your own.
pub const TEMPLATE: &str = include_str!("../templates/live12-base.als.xml");

/// Live's stock Tension instrument (`StringStudio`) with its default preset,
/// as Live 12.4.6 saved it. Extracted with `examples/extract_device.rs` from a
/// set Bengisu saved on 2026-09-27; its only path is Live's built-in device
/// reference inside the Live app.
pub const TENSION: &str = include_str!("../templates/devices/tension.xml");
/// Live's stock Drum Rack (`DrumGroupDevice`) with its default preset: an empty
/// rack, no pads or samples. Same source as [`TENSION`].
pub const DRUM_RACK: &str = include_str!("../templates/devices/drum-rack.xml");

/// An instrument to put on a track, after its Pitch rack. See
/// [`with_instrument`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Instrument {
    /// No instrument: the track is left exactly as the template has it.
    #[default]
    None,
    /// Live's stock Tension ([`TENSION`]), default preset.
    Melodic,
    /// Live's stock Drum Rack ([`DRUM_RACK`]), empty.
    Drums,
}

/// A copy of `track` (a `<MidiTrack>` block from [`template_tracks`] of
/// `template`) with `instrument` inserted right after its MIDI Effect Rack.
///
/// The device's pointee ids are moved above every id in `template` and
/// `track`, so they never collide; [`write`] then offsets them per track like
/// the rest of the block. [`Instrument::None`] returns `track` unchanged.
/// Errors if the track has no MIDI Effect Rack to insert after.
pub fn with_instrument(
    template: &str,
    track: &str,
    instrument: Instrument,
) -> Result<String, String> {
    let device = match instrument {
        Instrument::None => return Ok(track.to_string()),
        Instrument::Melodic => TENSION,
        Instrument::Drums => DRUM_RACK,
    };
    let anchor = "</MidiEffectGroupDevice>";
    let at = track
        .find(anchor)
        .map(|p| p + anchor.len())
        .ok_or("track has no MIDI Effect Rack to insert the instrument after")?;
    let base = max_id(template).max(max_id(track)) + 1;
    let device = rebase_ids(device.trim_end(), base);
    Ok(format!("{}{device}{}", &track[..at], &track[at..]))
}

/// Ids below this are list-local indices (`<ClipSlot Id="0">`, a device's
/// place in its chain) and are left alone by [`rebase_ids`].
const POINTEE_MIN: u64 = 1000;

/// Move every pointee-style id (>= [`POINTEE_MIN`]) in `s` so the smallest
/// becomes `base`, keeping their spacing.
fn rebase_ids(s: &str, base: u64) -> String {
    let mut min = u64::MAX;
    map_ids(s, |n| {
        if n >= POINTEE_MIN {
            min = min.min(n);
        }
        n
    });
    map_ids(s, |n| if n >= POINTEE_MIN { n - min + base } else { n })
}

/// Per-track id offset, keeping cloned `Id="…"` values globally unique. Larger
/// than any id a template realistically contains.
const ID_STRIDE: u64 = 10_000_000;
/// Base id for injected clip elements (above any template id, below ID_STRIDE).
const CLIP_ID_BASE: u64 = 5_000_000;
/// MPE per-note pitch-bend range, in semitones (matches the template's
/// `MpePitchBendRange`); a value of 8192 corresponds to a full +range bend.
const MPE_BEND_RANGE: f32 = 48.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSig {
    pub numerator: u8,
    pub denominator: u8,
}

/// One note. Times are in beats and share the clip's time base.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub pitch: u8,
    pub start: f64,
    pub dur: f64,
    pub velocity: u8,
    /// Per-note pitch expression: `(beats from the note start, semitones)`.
    /// Empty means no curve.
    pub pitch_curve: Vec<(f64, f32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub name: String,
    /// Timeline beat the clip starts at (0 for a Session clip).
    pub start: f64,
    pub length: f64,
    pub time_sig: TimeSig,
    pub notes: Vec<Note>,
}

/// A Session row: launching it starts every track's clip in that row and sets
/// the tempo.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub name: String,
    pub tempo: f64,
}

pub struct Track<'a> {
    /// The `<MidiTrack>…</MidiTrack>` block to clone, from [`template_tracks`].
    pub template: &'a str,
    pub name: String,
    /// One slot per [`Scene`], in order; `None` is an empty slot.
    pub session: Vec<Option<Clip>>,
    pub arrangement: Option<Clip>,
}

pub struct Set<'a> {
    pub tempo: f64,
    /// `(beat, bpm)` tempo changes after the start, for the Arrangement.
    pub tempo_changes: Vec<(f64, f64)>,
    pub scenes: Vec<Scene>,
    pub tracks: Vec<Track<'a>>,
}

/// Render `set` to `.als` bytes, keeping everything outside the tracks from
/// `template` (gunzipped Live set XML).
pub fn write(set: &Set, template: &str) -> Result<Vec<u8>, String> {
    gzip(&build_xml(set, template))
}

/// [`write`], with `tuning` loaded in the set. Note pitches are then steps of
/// the tuning (MIDI note 0 = [`TuningSystem::lowest`]) and pitch curves bend
/// from those steps. `None` writes exactly what [`write`] writes.
pub fn write_tuned(
    set: &Set,
    template: &str,
    tuning: Option<&TuningSystem>,
) -> Result<Vec<u8>, String> {
    gzip(&build_xml_tuned(set, template, tuning)?)
}

/// [`build_xml`] with `tuning` in the set's `<TuningSystems>`, replacing
/// whatever the template has there. Errors if the template has no
/// `<TuningSystems>` element (a set saved before Live 12).
pub fn build_xml_tuned(
    set: &Set,
    template: &str,
    tuning: Option<&TuningSystem>,
) -> Result<String, String> {
    let xml = build_xml(set, template);
    let Some(t) = tuning else {
        return Ok(xml);
    };
    let tuned = t.to_xml();
    if let Some(at) = xml.find("<TuningSystems />") {
        return Ok(format!(
            "{}{tuned}{}",
            &xml[..at],
            &xml[at + "<TuningSystems />".len()..]
        ));
    }
    if xml.contains("<TuningSystems>") {
        return Ok(strip_between(
            &xml,
            "<TuningSystems>",
            "</TuningSystems>",
            &tuned,
        ));
    }
    Err("template has no <TuningSystems> element".into())
}

/// Assemble the `.als` XML (before gzip).
pub fn build_xml(set: &Set, template: &str) -> String {
    let first = template
        .find("<MidiTrack ")
        .expect("template missing <MidiTrack>");
    let last_end = template
        .rfind("</MidiTrack>")
        .expect("template missing </MidiTrack>")
        + "</MidiTrack>".len();
    let head = &template[..first];
    let tail = &template[last_end..];

    // Master tempo lives in the MainTrack's Tempo block.
    let tail = set_value_after(tail, "<Tempo>", &fmt(set.tempo), Some("<Manual Value=\""));
    // Tempo automation on the Main track (the PointeeId-8 envelope), so the
    // Arrangement plays the full tempo map, including mid-song changes.
    let tail = set_tempo_automation(&tail, &tempo_events(set));
    // Drop any leftover groove so exports don't ship a stray groove + its notes.
    let tail = strip_between(&tail, "<Grooves>", "</Grooves>", "<Grooves />").replace(
        "<DefaultGrooveId Value=\"4\" />",
        "<DefaultGrooveId Value=\"-1\" />",
    );
    let tail = strip_between(
        &tail,
        "<Scenes>",
        "</Scenes>",
        &build_scenes(&tail, &set.scenes),
    );

    let mut out = String::with_capacity(template.len() * (1 + set.tracks.len()));
    out.push_str(head);
    for (idx, track) in set.tracks.iter().enumerate() {
        out.push_str(&render_track(track, idx as u64, set.scenes.len()));
    }
    out.push_str(&tail);

    // NextPointeeId must exceed every `Id="…"` in the document: compute it from
    // the actual maximum so no template (or id offset) can make it too low.
    let next_pointee = max_id(&out) + 1;
    set_value_after(
        &out,
        "<NextPointeeId Value=\"",
        &next_pointee.to_string(),
        None,
    )
}

/// Split a Live set into its top-level `<MidiTrack>…</MidiTrack>` blocks.
pub fn template_tracks(template: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut i = 0;
    while let Some(rel) = template[i..].find("<MidiTrack ") {
        let start = i + rel;
        let end = start
            + template[start..]
                .find("</MidiTrack>")
                .expect("unterminated MidiTrack")
            + "</MidiTrack>".len();
        blocks.push(&template[start..end]);
        i = end;
    }
    blocks
}

/// A template track's own name (its first `EffectiveName`).
pub fn track_name(block: &str) -> &str {
    let anchor = "<EffectiveName Value=\"";
    match block.find(anchor) {
        Some(p) => {
            let v = p + anchor.len();
            &block[v..v + block[v..].find('"').unwrap_or(0)]
        }
        None => "",
    }
}

/// Gzip the assembled XML into `.als` bytes.
fn gzip(xml: &str) -> Result<Vec<u8>, String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(xml.as_bytes())
        .map_err(|e| e.to_string())?;
    encoder.finish().map_err(|e| e.to_string())
}

/// Clone one template track: name, unique ids, Session clips, and the
/// Arrangement clip.
fn render_track(track: &Track, idx: u64, scenes: usize) -> String {
    // 1. Track name (the block's first EffectiveName/UserName).
    let block = set_track_name(track.template, &escape_xml(&track.name));
    // 2. Offset ids first so the cloned template is globally unique; the clips
    //    we inject next carry their own already-unique ids.
    let block = offset_ids(&block, idx * ID_STRIDE);
    // 3. Session view: replace the two ClipSlotLists. MainSequencer gets the
    //    clips, FreezeSequencer gets matching empty slots.
    let main = session_clips(track, idx, scenes);
    let empty = empty_clipslotlist(scenes);
    let block = replace_two_clipslotlists(&block, &main, &empty);
    // 4. Arrangement view: the MainSequencer's first empty <Events />.
    let events = match &track.arrangement {
        Some(c) if !c.notes.is_empty() => {
            format!(
                "<Events>{}</Events>",
                build_clip(c, CLIP_ID_BASE + idx * ID_STRIDE + 900_000)
            )
        }
        _ => "<Events />".to_string(),
    };
    block.replacen("<Events />", &events, 1)
}

/// `<ClipSlotList>` with one slot per scene.
fn session_clips(track: &Track, idx: u64, scenes: usize) -> String {
    let mut out = String::from("<ClipSlotList>");
    for si in 0..scenes {
        let inner = match track.session.get(si).and_then(|c| c.as_ref()) {
            Some(clip) => {
                let clip_id = CLIP_ID_BASE + idx * ID_STRIDE + si as u64 * 1000;
                format!(
                    "<ClipSlot><Value>{}</Value></ClipSlot>",
                    build_clip(clip, clip_id)
                )
            }
            None => "<ClipSlot><Value /></ClipSlot>".to_string(),
        };
        out.push_str(&format!(
            "<ClipSlot Id=\"{si}\"><LomId Value=\"0\" />{inner}<HasStop Value=\"true\" /></ClipSlot>"
        ));
    }
    out.push_str("</ClipSlotList>");
    out
}

/// A `<ClipSlotList>` of `count` empty slots.
fn empty_clipslotlist(count: usize) -> String {
    let mut out = String::from("<ClipSlotList>");
    for si in 0..count {
        out.push_str(&format!(
            "<ClipSlot Id=\"{si}\"><LomId Value=\"0\" /><ClipSlot><Value /></ClipSlot><HasStop Value=\"true\" /></ClipSlot>"
        ));
    }
    out.push_str("</ClipSlotList>");
    out
}

/// Replace the first two `<ClipSlotList>…</ClipSlotList>` blocks (MainSequencer,
/// then FreezeSequencer) with `first` and `second`.
fn replace_two_clipslotlists(block: &str, first: &str, second: &str) -> String {
    let open = "<ClipSlotList>";
    let close = "</ClipSlotList>";
    let Some(a1) = block.find(open) else {
        return block.to_string();
    };
    let a1_end = block[a1..]
        .find(close)
        .map(|e| a1 + e + close.len())
        .unwrap_or(block.len());
    match block[a1_end..].find(open) {
        Some(rel) => {
            let a2 = a1_end + rel;
            let a2_end = block[a2..]
                .find(close)
                .map(|e| a2 + e + close.len())
                .unwrap_or(block.len());
            format!(
                "{}{first}{}{second}{}",
                &block[..a1],
                &block[a1_end..a2],
                &block[a2_end..]
            )
        }
        None => format!("{}{first}{}", &block[..a1], &block[a1_end..]),
    }
}

/// Tempo-automation breakpoints (`<FloatEvent>`s): the initial tempo at Live's
/// "before the start" sentinel time, then each change as a step. Live draws a
/// straight line between breakpoints, so each change has two at its beat: the
/// tempo before it, then the new one.
fn tempo_events(set: &Set) -> String {
    let mut out = format!(
        "<FloatEvent Id=\"0\" Time=\"-63072000\" Value=\"{}\" />",
        fmt(set.tempo)
    );
    let mut before = set.tempo;
    let mut id = 1;
    for &(beat, bpm) in &set.tempo_changes {
        for value in [before, bpm] {
            out.push_str(&format!(
                "<FloatEvent Id=\"{id}\" Time=\"{}\" Value=\"{}\" />",
                fmt(beat),
                fmt(value)
            ));
            id += 1;
        }
        before = bpm;
    }
    out
}

/// Replace the inner `<Events>` of the Main track's tempo automation envelope
/// (the one whose `EnvelopeTarget` is `PointeeId 8`) with `float_events`.
fn set_tempo_automation(tail: &str, float_events: &str) -> String {
    let Some(p) = tail.find("<PointeeId Value=\"8\" />") else {
        return tail.to_string();
    };
    let Some(es) = tail[p..].find("<Events>").map(|i| p + i + "<Events>".len()) else {
        return tail.to_string();
    };
    let Some(ee) = tail[es..].find("</Events>").map(|i| es + i) else {
        return tail.to_string();
    };
    format!("{}{float_events}{}", &tail[..es], &tail[ee..])
}

/// `<Scenes>` with one `<Scene>` per scene, cloning the template's first scene
/// (preserving its fields) and setting each scene's id, name, and tempo
/// (enabled, so launching the scene sets the global tempo).
fn build_scenes(tail: &str, scenes: &[Scene]) -> String {
    let scene_tpl = match (tail.find("<Scene "), tail.find("</Scene>")) {
        (Some(a), Some(b)) if b > a => &tail[a..b + "</Scene>".len()],
        _ => return "<Scenes />".to_string(),
    };
    // The template scene's own id (e.g. `<Scene Id="0">`).
    let old_id = scene_tpl
        .find("Id=\"")
        .and_then(|p| {
            scene_tpl[p + 4..]
                .find('"')
                .map(|e| scene_tpl[p + 4..p + 4 + e].to_string())
        })
        .unwrap_or_else(|| "0".to_string());

    let mut out = String::from("<Scenes>");
    for (si, scene) in scenes.iter().enumerate() {
        let xml = scene_tpl
            .replacen(
                &format!("<Scene Id=\"{old_id}\">"),
                &format!("<Scene Id=\"{si}\">"),
                1,
            )
            .replacen(
                "<Name Value=\"\" />",
                &format!("<Name Value=\"{}\" />", escape_xml(&scene.name)),
                1,
            );
        let xml = set_value_after(&xml, "<Tempo Value=\"", &fmt(scene.tempo), None);
        let xml = xml.replacen(
            "<IsTempoEnabled Value=\"false\" />",
            "<IsTempoEnabled Value=\"true\" />",
            1,
        );
        out.push_str(&xml);
    }
    out.push_str("</Scenes>");
    out
}

/// Replace the track's own name (the first `EffectiveName`, and the matching
/// `UserName`) without touching device/chain names deeper in the block.
fn set_track_name(block: &str, new: &str) -> String {
    let old = track_name(block);
    block
        .replacen(
            &format!("<EffectiveName Value=\"{old}\" />"),
            &format!("<EffectiveName Value=\"{new}\" />"),
            1,
        )
        .replacen(
            &format!("<UserName Value=\"{old}\" />"),
            &format!("<UserName Value=\"{new}\" />"),
            1,
        )
}

/// The largest `Id="N"` value in the document.
fn max_id(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let needle = b"Id=\"";
    let mut i = 0;
    let mut max = 0u64;
    while i < bytes.len() {
        if bytes[i..].starts_with(needle) {
            let start = i + needle.len();
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'"' {
                j += 1;
            }
            if let Ok(n) = s[start..j].parse::<u64>() {
                max = max.max(n);
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    max
}

/// Add `offset` to every `Id="N"` (non-negative integers only).
fn offset_ids(s: &str, offset: u64) -> String {
    if offset == 0 {
        return s.to_string();
    }
    map_ids(s, |n| n + offset)
}

/// Replace every `Id="N"` (non-negative integers only) with `f(N)`.
fn map_ids(s: &str, mut f: impl FnMut(u64) -> u64) -> String {
    let bytes = s.as_bytes();
    let needle = b"Id=\"";
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(needle) {
            let start = i + needle.len();
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'"' {
                j += 1;
            }
            out.extend_from_slice(needle);
            match s[start..j].parse::<u64>() {
                Ok(n) => out.extend_from_slice(f(n).to_string().as_bytes()),
                Err(_) => out.extend_from_slice(&bytes[start..j]),
            }
            out.push(b'"');
            i = j + 1;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("map_ids split on ASCII boundaries")
}

/// One `<PerNoteEventList>` (dimension `CC="-2"` = pitch) applying `curve` to
/// `note_id`, values scaled to the MPE range.
fn pitch_list(list_id: u64, note_id: u32, curve: &[(f64, f32)]) -> String {
    let events: String = curve
        .iter()
        .map(|(offset, semitones)| {
            let value = (semitones / MPE_BEND_RANGE * 8192.0) as f64;
            format!(
                "<PerNoteEvent TimeOffset=\"{}\" Value=\"{}\" />",
                fmt(*offset),
                fmt(value)
            )
        })
        .collect();
    format!("<PerNoteEventList Id=\"{list_id}\" NoteId=\"{note_id}\" CC=\"-2\"><Events>{events}</Events></PerNoteEventList>")
}

/// Build one `<MidiClip>` spanning `clip.length` beats (at least one), notes
/// grouped into per-pitch KeyTracks.
fn build_clip(clip: &Clip, clip_id: u64) -> String {
    let length = clip.length.max(1.0);
    let start = clip.start;
    let end = start + length;
    let name = escape_xml(&clip.name);

    // Group notes by pitch, preserving a stable (ascending) order.
    let mut pitches: Vec<u8> = clip.notes.iter().map(|n| n.pitch).collect();
    pitches.sort_unstable();
    pitches.dedup();

    let mut key_tracks = String::new();
    let mut note_id = 1u32;
    // (NoteId, curve) for notes with a pitch curve.
    let mut curved: Vec<(u32, &[(f64, f32)])> = Vec::new();
    for (k, pitch) in pitches.iter().enumerate() {
        let mut events = String::new();
        for n in clip.notes.iter().filter(|n| n.pitch == *pitch) {
            events.push_str(&format!(
                "<MidiNoteEvent Time=\"{}\" Duration=\"{}\" Velocity=\"{}\" \
OffVelocity=\"64\" NoteId=\"{note_id}\" />",
                fmt(n.start),
                fmt(n.dur),
                n.velocity,
            ));
            if !n.pitch_curve.is_empty() {
                curved.push((note_id, &n.pitch_curve));
            }
            note_id += 1;
        }
        key_tracks.push_str(&format!(
            "<KeyTrack Id=\"{}\"><Notes>{events}</Notes><MidiKey Value=\"{pitch}\" /></KeyTrack>",
            clip_id + 1 + k as u64,
        ));
    }

    // Per-note pitch curves (MPE) keyed by NoteId: independent per note, so
    // overlapping notes never share a bend.
    let per_note = if curved.is_empty() {
        "<EventLists />".to_string()
    } else {
        let lists: String = curved
            .iter()
            .enumerate()
            .map(|(i, &(nid, curve))| pitch_list(i as u64 + 1, nid, curve))
            .collect();
        format!("<EventLists>{lists}</EventLists>")
    };

    format!(
        "<MidiClip Id=\"{clip_id}\" Time=\"{start}\">\
<LomId Value=\"0\" /><LomIdView Value=\"0\" />\
<CurrentStart Value=\"{start}\" /><CurrentEnd Value=\"{end}\" />\
<Loop><LoopStart Value=\"{start}\" /><LoopEnd Value=\"{end}\" /><StartRelative Value=\"0\" />\
<LoopOn Value=\"false\" /><OutMarker Value=\"{end}\" /><HiddenLoopStart Value=\"{start}\" />\
<HiddenLoopEnd Value=\"{end}\" /></Loop>\
<Name Value=\"{name}\" /><Annotation Value=\"\" /><Color Value=\"-1\" />\
<LaunchMode Value=\"0\" /><LaunchQuantisation Value=\"0\" />\
<TimeSignature><TimeSignatures><RemoteableTimeSignature Id=\"0\">\
<Numerator Value=\"{num}\" /><Denominator Value=\"{den}\" /><Time Value=\"0\" />\
</RemoteableTimeSignature></TimeSignatures></TimeSignature>\
<Envelopes><Envelopes /></Envelopes>\
<ScrollerTimePreserver><LeftTime Value=\"{start}\" /><RightTime Value=\"{end}\" /></ScrollerTimePreserver>\
<TimeSelection><AnchorTime Value=\"0\" /><OtherTime Value=\"0\" /></TimeSelection>\
<Legato Value=\"false\" /><Ram Value=\"false\" />\
<GrooveSettings><GrooveId Value=\"-1\" /></GrooveSettings>\
<Disabled Value=\"false\" /><VelocityAmount Value=\"0\" />\
<FollowAction><FollowTime Value=\"4\" /><IsLinked Value=\"true\" /><LoopIterations Value=\"1\" />\
<FollowActionA Value=\"4\" /><FollowActionB Value=\"0\" /><FollowChanceA Value=\"100\" />\
<FollowChanceB Value=\"0\" /><JumpIndexA Value=\"2\" /><JumpIndexB Value=\"2\" />\
<FollowActionEnabled Value=\"false\" /></FollowAction>\
<Grid><FixedNumerator Value=\"1\" /><FixedDenominator Value=\"16\" /><GridIntervalPixel Value=\"20\" />\
<Ntoles Value=\"2\" /><SnapToGrid Value=\"true\" /><Fixed Value=\"false\" /></Grid>\
<FreezeStart Value=\"0\" /><FreezeEnd Value=\"0\" /><IsWarped Value=\"true\" /><TakeId Value=\"0\" />\
<IsInKey Value=\"false\" /><ScaleInformation><Root Value=\"0\" /><Name Value=\"0\" /></ScaleInformation>\
<Notes><KeyTracks>{key_tracks}</KeyTracks>\
<PerNoteEventStore>{per_note}</PerNoteEventStore>\
<NoteProbabilityGroups /><ProbabilityGroupIdGenerator><NextId Value=\"1\" /></ProbabilityGroupIdGenerator>\
<NoteIdGenerator><NextId Value=\"{next_note_id}\" /></NoteIdGenerator></Notes>\
<BankSelectCoarse Value=\"-1\" /><BankSelectFine Value=\"-1\" /><ProgramChange Value=\"-1\" />\
<NoteEditorFoldInZoom Value=\"-1\" /><NoteEditorFoldInScroll Value=\"0\" />\
<NoteEditorFoldOutZoom Value=\"0\" /><NoteEditorFoldOutScroll Value=\"0\" />\
<NoteEditorFoldScaleZoom Value=\"-1\" /><NoteEditorFoldScaleScroll Value=\"0\" />\
<NoteSpellingPreference Value=\"0\" /><AccidentalSpellingPreference Value=\"3\" />\
<PreferFlatRootNote Value=\"false\" />\
<ExpressionGrid><FixedNumerator Value=\"1\" /><FixedDenominator Value=\"16\" />\
<GridIntervalPixel Value=\"20\" /><Ntoles Value=\"2\" /><SnapToGrid Value=\"false\" /><Fixed Value=\"false\" />\
</ExpressionGrid></MidiClip>",
        start = fmt(start),
        end = fmt(end),
        num = clip.time_sig.numerator,
        den = clip.time_sig.denominator,
        next_note_id = note_id,
    )
}

/// Format an f64 the way Live does: plain decimal, integers without a point.
fn fmt(x: f64) -> String {
    if x.fract() == 0.0 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

/// Set the `Value="…"` that follows `anchor`. With `open = Some(o)`, the value
/// is the one inside the first `o` after `anchor` (e.g. the `<Manual Value="…"`
/// inside a `<Tempo>` block); with `None`, `anchor` itself ends just before the
/// value (e.g. `<NextPointeeId Value="`). No-op if the markers aren't found.
fn set_value_after(s: &str, anchor: &str, new: &str, open: Option<&str>) -> String {
    let Some(a) = s.find(anchor) else {
        return s.to_string();
    };
    let vstart = match open {
        None => a + anchor.len(),
        Some(o) => match s[a..].find(o) {
            Some(rel) => a + rel + o.len(),
            None => return s.to_string(),
        },
    };
    let vend = vstart + s[vstart..].find('"').unwrap_or(0);
    format!("{}{}{}", &s[..vstart], new, &s[vend..])
}

/// Replace everything from `open` to the end of `close` (inclusive) with
/// `replacement`. No-op if either marker is missing.
fn strip_between(s: &str, open: &str, close: &str, replacement: &str) -> String {
    match (s.find(open), s.find(close)) {
        (Some(a), Some(b)) if b >= a => {
            format!("{}{}{}", &s[..a], replacement, &s[b + close.len()..])
        }
        _ => s.to_string(),
    }
}

/// Compiles and runs the README's example as a doc-test.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
