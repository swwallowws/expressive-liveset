//! Writes `scratch/check-template.als`: a small set to open in Live and check
//! that the bundled template loads.
//!
//! What it holds: one MIDI track "Check" (Pitch rack, then Live's stock
//! Tension via `Instrument::Melodic`) with two scenes. Scene "Bend" has a
//! 4-beat clip: C4 bending up 2 semitones over its first beat, then E4 with no
//! bend. Scene "Plain" is empty. The Arrangement has the same clip at beat 0.
//! Tempo starts at 100 BPM and jumps to 140 BPM at beat 8.
//!
//! Run: `cargo run --example check_template`

use expressive_liveset::{
    template_tracks, with_instrument, write, Clip, Instrument, Note, Scene, Set, TimeSig, Track,
    TEMPLATE,
};

fn main() {
    let clip = Clip {
        name: "Bend check".into(),
        start: 0.0,
        length: 4.0,
        time_sig: TimeSig {
            numerator: 4,
            denominator: 4,
        },
        notes: vec![
            Note {
                pitch: 60,
                start: 0.0,
                dur: 2.0,
                velocity: 100,
                pitch_curve: vec![(0.0, 0.0), (1.0, 2.0)],
            },
            Note {
                pitch: 64,
                start: 2.0,
                dur: 2.0,
                velocity: 100,
                pitch_curve: vec![],
            },
        ],
    };

    let track = with_instrument(TEMPLATE, template_tracks(TEMPLATE)[0], Instrument::Melodic)
        .expect("add Tension");

    let set = Set {
        tempo: 100.0,
        tempo_changes: vec![(8.0, 140.0)],
        scenes: vec![
            Scene {
                name: "Bend".into(),
                tempo: 100.0,
            },
            Scene {
                name: "Plain".into(),
                tempo: 100.0,
            },
        ],
        tracks: vec![Track {
            template: &track,
            name: "Check".into(),
            session: vec![Some(clip.clone()), None],
            arrangement: Some(clip),
        }],
    };

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scratch");
    std::fs::create_dir_all(&dir).expect("create scratch/");
    let path = dir.join("check-template.als");
    std::fs::write(&path, write(&set, TEMPLATE).expect("write set")).expect("write file");
    println!("{}", path.display());
}
