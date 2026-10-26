//! Build small sets, gunzip them, and check the XML Live will read: well-formed,
//! globally unique ids, and per-note pitch curves on the right notes.

use std::io::Read;

use expressive_liveset::{
    template_tracks, with_instrument, write, Clip, Instrument, Note, Scene, Set, TimeSig, Track,
    TEMPLATE,
};
use flate2::read::GzDecoder;
use roxmltree::{Document, ParsingOptions};

fn gunzip(bytes: &[u8]) -> String {
    let mut s = String::new();
    GzDecoder::new(bytes)
        .read_to_string(&mut s)
        .expect("gunzip");
    s
}

fn doc(xml: &str) -> Document<'_> {
    let opts = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    Document::parse_with_options(xml, opts).expect("well-formed XML")
}

fn note(pitch: u8, start: f64, curve: Vec<(f64, f32)>) -> Note {
    Note {
        pitch,
        start,
        dur: 1.0,
        velocity: 100,
        pitch_curve: curve,
    }
}

fn clip(notes: Vec<Note>) -> Clip {
    Clip {
        name: "Take & \"one\"".into(),
        start: 0.0,
        length: 8.0,
        time_sig: TimeSig {
            numerator: 4,
            denominator: 4,
        },
        notes,
    }
}

fn one_track_set(c: Clip) -> Set<'static> {
    let block = template_tracks(TEMPLATE)[0];
    Set {
        tempo: 120.0,
        tempo_changes: vec![],
        scenes: vec![Scene {
            name: "Take".into(),
            tempo: 120.0,
        }],
        tracks: vec![Track {
            template: block,
            name: "Voice".into(),
            session: vec![Some(c.clone())],
            arrangement: Some(c),
        }],
    }
}

#[test]
fn a_set_is_gzip_and_well_formed() {
    let bytes = write(&one_track_set(clip(vec![note(60, 0.0, vec![])])), TEMPLATE).unwrap();
    assert_eq!(&bytes[..2], &[0x1f, 0x8b]);
    let xml = gunzip(&bytes);
    let d = doc(&xml);
    assert_eq!(
        d.descendants()
            .filter(|n| n.has_tag_name("MidiTrack"))
            .count(),
        1
    );
    assert!(xml.contains("<EffectiveName Value=\"Voice\" />"));
    assert!(
        xml.contains("<Name Value=\"Take &amp; &quot;one&quot;\" />"),
        "clip name escaped"
    );
    // The note appears in the Session clip and in the Arrangement clip.
    assert_eq!(xml.matches("<MidiNoteEvent ").count(), 2);
}

#[test]
fn next_pointee_id_exceeds_every_id() {
    let xml = gunzip(
        &write(
            &one_track_set(clip(vec![note(60, 0.0, vec![(0.0, 1.0)])])),
            TEMPLATE,
        )
        .unwrap(),
    );
    let max = xml
        .match_indices("Id=\"")
        .filter_map(|(i, _)| {
            let s = i + 4;
            xml[s..s + xml[s..].find('"')?].parse::<u64>().ok()
        })
        .max()
        .unwrap();
    let a = "<NextPointeeId Value=\"";
    let p = xml.find(a).unwrap() + a.len();
    let next: u64 = xml[p..p + xml[p..].find('"').unwrap()].parse().unwrap();
    assert!(next > max, "NextPointeeId {next} <= max id {max}");
}

/// A curve becomes one pitch list (CC -2) keyed to its own note, scaled so
/// 48 semitones = 8192. A note without a curve gets no list.
#[test]
fn pitch_curves_attach_to_their_notes() {
    let c = clip(vec![
        note(64, 1.0, vec![(0.0, 0.0), (0.5, 12.0)]),
        note(60, 0.0, vec![]),
    ]);
    let xml = gunzip(&write(&one_track_set(c), TEMPLATE).unwrap());
    let d = doc(&xml);
    let clip = d
        .descendants()
        .find(|n| n.has_tag_name("MidiClip"))
        .unwrap();
    let lists: Vec<_> = clip
        .descendants()
        .filter(|n| n.has_tag_name("PerNoteEventList"))
        .collect();
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].attribute("CC"), Some("-2"));
    let nid = lists[0].attribute("NoteId").unwrap();
    let owner = clip
        .descendants()
        .find(|n| n.has_tag_name("MidiNoteEvent") && n.attribute("NoteId") == Some(nid))
        .unwrap();
    assert_eq!(
        owner.attribute("Time"),
        Some("1"),
        "the curve belongs to the E, not the C"
    );
    let values: Vec<_> = lists[0]
        .descendants()
        .filter(|n| n.has_tag_name("PerNoteEvent"))
        .map(|n| {
            (
                n.attribute("TimeOffset").unwrap(),
                n.attribute("Value").unwrap(),
            )
        })
        .collect();
    assert_eq!(values, [("0", "0"), ("0.5", "2048")]);
}

#[test]
fn tempo_is_set_and_changes_become_automation() {
    let mut set = one_track_set(clip(vec![note(60, 0.0, vec![])]));
    set.tempo = 90.0;
    set.tempo_changes = vec![(16.0, 140.0)];
    let xml = gunzip(&write(&set, TEMPLATE).unwrap());
    assert!(xml.contains("<FloatEvent Id=\"0\" Time=\"-63072000\" Value=\"90\" />"));
    assert!(xml.contains("<FloatEvent Id=\"1\" Time=\"16\" Value=\"140\" />"));
}

/// The bundled template is a clean skeleton: written sets carry no return
/// tracks (so no sends), no groove, and no machine-specific home paths.
#[test]
fn default_template_writes_no_returns_groove_or_home_paths() {
    let xml = gunzip(&write(&one_track_set(clip(vec![note(60, 0.0, vec![])])), TEMPLATE).unwrap());
    let d = doc(&xml);
    for tag in ["ReturnTrack", "TrackSendHolder", "SendPreBool", "Groove"] {
        assert_eq!(
            d.descendants().filter(|n| n.has_tag_name(tag)).count(),
            0,
            "unexpected <{tag}>"
        );
    }
    assert!(xml.contains("<DefaultGrooveId Value=\"-1\" />"));
    assert!(!xml.contains("/Users/"), "home-folder path in the set");
}

/// The Pitch rack's first macro (mapped across -128..+128 st) sits at its
/// centre, so a set plays at the pitch its notes are written in. It was saved
/// at 65.996 once, which made every set play 5 semitones sharp.
#[test]
fn the_pitch_macro_starts_at_zero_semitones() {
    let xml = gunzip(&write(&one_track_set(clip(vec![note(60, 0.0, vec![])])), TEMPLATE).unwrap());
    let d = doc(&xml);
    for macro0 in d.descendants().filter(|n| n.has_tag_name("MacroControls.0")) {
        let manual = macro0
            .children()
            .find(|n| n.has_tag_name("Manual"))
            .and_then(|n| n.attribute("Value"))
            .expect("macro value");
        assert_eq!(manual, "63.5", "Pitch macro off centre: {manual}");
    }
}

/// Ids that must be globally unique: those of `AutomationTarget`,
/// `ModulationTarget` (any `*Target`) and `Pointee` elements. Other `Id`s are
/// list-local indices (`<ClipSlot Id="0">`) that legitimately repeat. Sorted.
fn pointee_ids(xml: &str) -> Vec<u64> {
    // A whole set parses as is; a bare track block needs a root around it.
    let owned;
    let text = if xml.starts_with("<?xml") {
        xml
    } else {
        owned = format!("<r>{xml}</r>");
        &owned
    };
    let d = doc(text);
    let mut ids: Vec<u64> = d
        .descendants()
        .filter(|n| {
            let t = n.tag_name().name();
            t.ends_with("Target") || t == "Pointee"
        })
        .filter_map(|n| n.attribute("Id")?.parse().ok())
        .collect();
    ids.sort_unstable();
    ids
}

fn duplicates(ids: &[u64]) -> Vec<u64> {
    ids.windows(2)
        .filter(|w| w[0] == w[1])
        .map(|w| w[0])
        .collect()
}

fn instrument_set(tracks: &[(&str, Instrument)]) -> String {
    let base = template_tracks(TEMPLATE)[0];
    let blocks: Vec<String> = tracks
        .iter()
        .map(|(_, inst)| with_instrument(TEMPLATE, base, *inst).unwrap())
        .collect();
    let set = Set {
        tempo: 120.0,
        tempo_changes: vec![],
        scenes: vec![Scene {
            name: "A".into(),
            tempo: 120.0,
        }],
        tracks: tracks
            .iter()
            .zip(&blocks)
            .map(|((name, _), block)| Track {
                template: block,
                name: (*name).into(),
                session: vec![Some(clip(vec![note(
                    60,
                    0.0,
                    vec![(0.0, 0.0), (0.5, 1.0)],
                )]))],
                arrangement: None,
            })
            .collect(),
    };
    gunzip(&write(&set, TEMPLATE).unwrap())
}

/// `Instrument::None` changes nothing: the block and the written set are
/// byte-for-byte what they were without the call.
#[test]
fn no_instrument_writes_todays_output() {
    let block = template_tracks(TEMPLATE)[0];
    assert_eq!(
        with_instrument(TEMPLATE, block, Instrument::None).unwrap(),
        block
    );
    let c = clip(vec![note(60, 0.0, vec![(0.0, 0.0), (1.0, 2.0)])]);
    let plain = write(&one_track_set(c.clone()), TEMPLATE).unwrap();
    let same = with_instrument(TEMPLATE, block, Instrument::None).unwrap();
    let mut set = one_track_set(c);
    set.tracks[0].template = &same;
    assert_eq!(write(&set, TEMPLATE).unwrap(), plain);
}

/// Each device lands on its own track, right after the Pitch rack.
#[test]
fn instruments_land_on_their_tracks_after_the_pitch_rack() {
    let xml = instrument_set(&[
        ("Lead", Instrument::Melodic),
        ("Kit", Instrument::Drums),
        ("Plain", Instrument::None),
    ]);
    let d = doc(&xml);
    let tracks: Vec<_> = d
        .descendants()
        .filter(|n| n.has_tag_name("MidiTrack"))
        .collect();
    assert_eq!(tracks.len(), 3);
    let count =
        |t: &roxmltree::Node, tag: &str| t.descendants().filter(|n| n.has_tag_name(tag)).count();
    let name = |t: &roxmltree::Node| {
        t.descendants()
            .find(|n| n.has_tag_name("EffectiveName"))
            .and_then(|n| n.attribute("Value"))
            .unwrap()
            .to_string()
    };
    let expect = [("Lead", 1, 0), ("Kit", 0, 1), ("Plain", 0, 0)];
    for (t, (want_name, tension, drums)) in tracks.iter().zip(expect) {
        assert_eq!(name(t), want_name);
        assert_eq!(count(t, "StringStudio"), tension, "{want_name}: Tension");
        assert_eq!(count(t, "DrumGroupDevice"), drums, "{want_name}: Drum Rack");
        let rack = t
            .descendants()
            .find(|n| n.has_tag_name("MidiEffectGroupDevice"))
            .unwrap();
        let next = rack.next_sibling_element().map(|n| n.tag_name().name());
        let want = match (tension, drums) {
            (1, _) => Some("StringStudio"),
            (_, 1) => Some("DrumGroupDevice"),
            _ => None,
        };
        assert_eq!(next, want, "{want_name}: device after the Pitch rack");
    }
}

/// Pointee-style ids stay globally unique with devices on several tracks, and
/// no home-folder path gets in.
#[test]
fn instrument_ids_are_unique_and_pathless() {
    let xml = instrument_set(&[
        ("A", Instrument::Melodic),
        ("B", Instrument::Melodic),
        ("C", Instrument::Drums),
        ("D", Instrument::None),
    ]);
    let ids = pointee_ids(&xml);
    let plain = pointee_ids(&instrument_set(&[
        ("A", Instrument::None),
        ("B", Instrument::None),
        ("C", Instrument::None),
        ("D", Instrument::None),
    ]));
    assert!(ids.len() > plain.len(), "devices' targets counted");
    assert_eq!(duplicates(&ids), Vec::<u64>::new(), "duplicate pointee ids");
    assert!(!xml.contains("/Users/"));
    let a = "<NextPointeeId Value=\"";
    let p = xml.find(a).unwrap() + a.len();
    let next: u64 = xml[p..p + xml[p..].find('"').unwrap()].parse().unwrap();
    assert!(next > *ids.last().unwrap());
}

/// Two instruments on one block don't reuse each other's ids.
#[test]
fn stacking_instruments_keeps_ids_unique() {
    let base = template_tracks(TEMPLATE)[0];
    let one = with_instrument(TEMPLATE, base, Instrument::Melodic).unwrap();
    let two = with_instrument(TEMPLATE, &one, Instrument::Melodic).unwrap();
    assert_eq!(duplicates(&pointee_ids(&two)), Vec::<u64>::new());
}

#[test]
fn empty_session_slots_and_arrangement_are_allowed() {
    let block = template_tracks(TEMPLATE)[0];
    let set = Set {
        tempo: 120.0,
        tempo_changes: vec![],
        scenes: vec![
            Scene {
                name: "A".into(),
                tempo: 120.0,
            },
            Scene {
                name: "B".into(),
                tempo: 120.0,
            },
        ],
        tracks: vec![Track {
            template: block,
            name: "T".into(),
            session: vec![None, Some(clip(vec![note(60, 0.0, vec![])]))],
            arrangement: None,
        }],
    };
    let xml = gunzip(&write(&set, TEMPLATE).unwrap());
    doc(&xml);
    assert_eq!(xml.matches("<MidiNoteEvent ").count(), 1);
    assert_eq!(xml.matches("<Scene ").count(), 2);
}
