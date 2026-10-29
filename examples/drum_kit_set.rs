//! Writes a set with one drum track on the default kit playing a 4-bar beat,
//! to check in Live that the kit sounds on a machine other than the one that
//! saved it (no missing samples).
//!
//! Run: `cargo run --example drum_kit_set -- scratch/drum-kit-test.als`

use expressive_liveset::{
    template_tracks, with_instrument, write, Clip, Instrument, Note, Scene, Set, TimeSig, Track,
    TEMPLATE,
};

fn hit(pitch: u8, start: f64, velocity: u8) -> Note {
    Note { pitch, start, dur: 0.25, velocity, pitch_curve: vec![] }
}

fn main() {
    let out = std::env::args().nth(1).expect("usage: drum_kit_set <out.als>");
    // Kick on 1 and 3, snare on 2 and 4, closed hats on the eighths (GM notes).
    let mut notes = Vec::new();
    for bar in 0..4 {
        let b = bar as f64 * 4.0;
        for beat in [0.0, 2.0] {
            notes.push(hit(36, b + beat, 110));
        }
        for beat in [1.0, 3.0] {
            notes.push(hit(38, b + beat, 100));
        }
        for eighth in 0..8 {
            notes.push(hit(42, b + eighth as f64 * 0.5, if eighth % 2 == 0 { 90 } else { 70 }));
        }
    }
    let clip = Clip {
        name: "Beat".into(),
        start: 0.0,
        length: 16.0,
        time_sig: TimeSig { numerator: 4, denominator: 4 },
        notes,
    };
    let block = with_instrument(TEMPLATE, template_tracks(TEMPLATE)[0], Instrument::Drums)
        .expect("drum track");
    let set = Set {
        tempo: 110.0,
        tempo_changes: vec![],
        scenes: vec![Scene { name: "Beat".into(), tempo: 110.0 }],
        tracks: vec![Track {
            template: &block,
            name: "Drums".into(),
            session: vec![Some(clip.clone())],
            arrangement: Some(clip),
        }],
    };
    std::fs::write(&out, write(&set, TEMPLATE).expect("write set")).expect("write file");
    println!("{out}");
}
