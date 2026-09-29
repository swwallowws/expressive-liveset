//! A set carrying a tuning: the `<TuningSystems>` subtree matches what Live
//! 12 saved for 53-EDO (tests/fixtures/live12-53edo-tuning.xml, cut from a set
//! Live 12.4 saved with its 53-EDO tuning loaded; Ableton's note names and
//! description replaced by step numbers and ""), and no tuning leaves the
//! output byte-for-byte as it was.

use std::io::Read;

use expressive_liveset::{
    build_xml, build_xml_tuned, template_tracks, write, write_tuned, Anchor, Clip, Note,
    NotePosition, Ratio, Scene, Set, TimeSig, Track, TuningSystem, TEMPLATE,
};
use flate2::read::GzDecoder;
use roxmltree::{Document, ParsingOptions};

const LIVE_53EDO: &str = include_str!("fixtures/live12-53edo-tuning.xml");

/// The 53-EDO .scl Starling ships.
fn scl_53() -> String {
    let mut s = String::from("! 53-edo.scl\nTurkish makam, 53 commas (53-EDO)\n 53\n");
    for i in 1..53 {
        s.push_str(&format!(" {:.6}\n", 1200.0 * i as f64 / 53.0));
    }
    s.push_str(" 2/1\n");
    s
}

/// XML without the whitespace between tags, for comparing layouts.
fn compact(xml: &str) -> String {
    xml.lines().map(str::trim).collect::<Vec<_>>().join("")
}

fn gunzip(bytes: &[u8]) -> String {
    let mut s = String::new();
    GzDecoder::new(bytes).read_to_string(&mut s).unwrap();
    s
}

fn doc(xml: &str) -> Document<'_> {
    let opts = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    Document::parse_with_options(xml, opts).expect("well-formed XML")
}

fn set(pitches: &[u8]) -> Set<'static> {
    let notes = pitches
        .iter()
        .enumerate()
        .map(|(i, &pitch)| Note {
            pitch,
            start: i as f64,
            dur: 1.0,
            velocity: 100,
            pitch_curve: vec![(0.0, 0.0), (0.5, 0.3)],
        })
        .collect();
    let clip = Clip {
        name: "take".into(),
        start: 0.0,
        length: 8.0,
        time_sig: TimeSig {
            numerator: 4,
            denominator: 4,
        },
        notes,
    };
    Set {
        tempo: 120.0,
        tempo_changes: vec![],
        scenes: vec![Scene {
            name: "take".into(),
            tempo: 120.0,
        }],
        tracks: vec![Track {
            template: template_tracks(TEMPLATE)[0],
            name: "take".into(),
            session: vec![Some(clip.clone())],
            arrangement: Some(clip),
        }],
    }
}

/// Live's own 53-EDO: A (index 40 of octave 3) at 440 Hz, MIDI note 0 on
/// index 24 of octave 2, so MIDI note 69 is the A.
fn live_53edo(t: TuningSystem) -> TuningSystem {
    TuningSystem {
        name: "53-EDO".into(),
        description: String::new(),
        reference: NotePosition {
            octave: 3,
            index: 40,
        },
        reference_hz: 440.0,
        ..t
    }
}

#[test]
fn fifty_three_edo_from_cents_is_what_live_saved() {
    let degrees: Vec<f64> = (0..53).map(|i| 1200.0 * i as f64 / 53.0).collect();
    // 1/1 = C of octave 3; MIDI note 0 = 40 - 69 = 29 steps below it.
    let anchor = Anchor {
        octave: 3,
        hz: 261.6,
        lowest_step: -29,
    };
    let t = live_53edo(TuningSystem::from_cents("x", "", &degrees, 1200.0, anchor).unwrap());
    assert_eq!(
        t.lowest,
        NotePosition {
            octave: 2,
            index: 24
        }
    );
    assert_eq!(compact(&t.to_xml()), compact(LIVE_53EDO));
}

#[test]
fn fifty_three_edo_from_scala_is_what_live_saved() {
    let anchor = Anchor {
        octave: 3,
        hz: 261.6,
        lowest_step: -29,
    };
    let t = TuningSystem::from_scala("53-edo", &scl_53(), anchor).unwrap();
    assert_eq!(t.description, "Turkish makam, 53 commas (53-EDO)");
    assert_eq!(
        t.pseudo_octave_ratio,
        Some(Ratio {
            numerator: 2,
            denominator: 1
        })
    );
    assert_eq!(compact(&live_53edo(t).to_xml()), compact(LIVE_53EDO));
}

/// Same numbers as Starling's .ascl: `REFERENCE_PITCH 3 0 261.6256` and
/// `NOTE_RANGE_BY_INDEX 3 24 5 45` for a take whose lowest step is 24.
#[test]
fn anchor_places_the_reference_and_the_range_like_the_ascl() {
    let t = TuningSystem::from_scala(
        "53-edo",
        &scl_53(),
        Anchor {
            octave: 3,
            hz: 261.625565,
            lowest_step: 24,
        },
    )
    .unwrap();
    assert_eq!(t.reference, NotePosition { octave: 3, index: 0 });
    assert_eq!(t.lowest, NotePosition { octave: 3, index: 24 });
    assert_eq!(t.highest(), NotePosition { octave: 5, index: 45 });
    let xml = t.to_xml();
    assert!(xml.contains("<FrequencyInHz Value=\"261.625565\" />"));
    // 12 notes, 1/1 on MIDI note 60: note 0 is step -60, five octaves down.
    let tet = TuningSystem::from_cents(
        "12",
        "",
        &(0..12).map(|i| i as f64 * 100.0).collect::<Vec<_>>(),
        1200.0,
        Anchor {
            octave: 3,
            hz: 261.625565,
            lowest_step: -60,
        },
    )
    .unwrap();
    assert_eq!(tet.lowest, NotePosition { octave: -2, index: 0 });
    assert_eq!(tet.highest(), NotePosition { octave: 8, index: 7 });
}

#[test]
fn ratios_are_kept_and_the_first_note_is_one_to_one() {
    let scl = "! ji\nJI major\n 7\n 9/8\n 5/4\n 4/3\n 3/2\n 5/3\n 15/8\n 2/1\n";
    let t = TuningSystem::from_scala(
        "ji",
        scl,
        Anchor {
            octave: 3,
            hz: 261.625565,
            lowest_step: -35,
        },
    )
    .unwrap();
    let xml = format!("<r>{}</r>", t.to_xml());
    let d = doc(&xml);
    let notes: Vec<_> = d
        .descendants()
        .filter(|n| n.has_tag_name("NoteTuning"))
        .collect();
    assert_eq!(notes.len(), 7);
    let ratio = |n: &roxmltree::Node| {
        let v = |tag| {
            n.descendants()
                .find(|c| c.has_tag_name(tag))
                .and_then(|c| c.attribute("Value"))
                .map(str::to_string)
        };
        (v("Numerator"), v("Denominator"))
    };
    assert_eq!(ratio(&notes[0]), (Some("1".into()), Some("1".into())));
    assert_eq!(ratio(&notes[2]), (Some("5".into()), Some("4".into())));
    assert!(xml.contains("<Cents Value=\"386.31372\" />"), "5/4 in cents");
}

#[test]
fn cents_scales_get_sorted_and_names_escaped() {
    let mut t = TuningSystem::from_cents(
        "a <b>",
        "c & d",
        &[0.0, 700.0, 300.0],
        1200.0,
        Anchor {
            octave: 3,
            hz: 440.0,
            lowest_step: 0,
        },
    )
    .unwrap();
    assert_eq!(
        t.notes.iter().map(|n| n.cents).collect::<Vec<_>>(),
        [0.0, 300.0, 700.0]
    );
    t.note_names = vec!["C".into(), "E\u{266d}".into(), "\"G\"".into()];
    let xml = t.to_xml();
    assert!(xml.contains("<TuningSystemName Value=\"a &lt;b&gt;\" />"));
    assert!(xml.contains("<ShortDescription Value=\"c &amp; d\" />"));
    assert!(xml.contains("<RemoteableString Id=\"2\" Value=\"&quot;G&quot;\" />"));
    doc(&format!("<r>{xml}</r>"));
}

/// No tuning: the same bytes as before the tuning API existed.
#[test]
fn no_tuning_is_byte_identical() {
    let s = set(&[60, 64, 67]);
    assert_eq!(write_tuned(&s, TEMPLATE, None).unwrap(), write(&s, TEMPLATE).unwrap());
    assert_eq!(
        build_xml_tuned(&s, TEMPLATE, None).unwrap(),
        build_xml(&s, TEMPLATE)
    );
    assert!(build_xml(&s, TEMPLATE).contains("<TuningSystems />"));
}

/// A tuned set: one TuningSystem under LiveSet, in place of the empty element,
/// the rest of the set unchanged, and the whole thing well-formed.
#[test]
fn a_tuned_set_carries_the_tuning_in_place_of_the_empty_element() {
    let s = set(&[29, 69, 98]);
    let t = TuningSystem::from_scala(
        "53-edo",
        &scl_53(),
        Anchor {
            octave: 3,
            hz: 261.625565,
            lowest_step: 24,
        },
    )
    .unwrap();
    let xml = gunzip(&write_tuned(&s, TEMPLATE, Some(&t)).unwrap());
    let d = doc(&xml);
    let systems: Vec<_> = d
        .descendants()
        .filter(|n| n.has_tag_name("TuningSystems"))
        .collect();
    assert_eq!(systems.len(), 1);
    assert_eq!(systems[0].parent().unwrap().tag_name().name(), "LiveSet");
    assert_eq!(
        systems[0]
            .children()
            .filter(|n| n.has_tag_name("TuningSystem"))
            .count(),
        1
    );
    assert!(!xml.contains("<TuningSystems />"));
    let plain = build_xml(&s, TEMPLATE);
    assert_eq!(
        xml.replacen(&t.to_xml(), "<TuningSystems />", 1),
        plain,
        "only the tuning differs"
    );
    // Replacing an existing tuning, not adding a second one.
    let again = build_xml_tuned(&s, &xml, Some(&t)).unwrap();
    assert_eq!(again.matches("<TuningSystem Id=").count(), 1);
}

#[test]
fn a_template_without_tuning_systems_is_an_error() {
    let s = set(&[60]);
    let old = TEMPLATE.replacen("<TuningSystems />", "", 1);
    let t = TuningSystem::from_cents(
        "x",
        "",
        &[0.0],
        1200.0,
        Anchor {
            octave: 3,
            hz: 440.0,
            lowest_step: 0,
        },
    )
    .unwrap();
    assert!(build_xml_tuned(&s, &old, Some(&t)).is_err());
}
