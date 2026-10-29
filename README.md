# expressive-liveset

A small Rust crate that writes Ableton Live 12 sets (`.als`) whose MIDI clips
carry per-note pitch expression (MPE).

Live's MIDI file import drops MPE, so a set file is the way to hand Live notes
with their own pitch curves. `.als` is gzipped, undocumented, version-specific
XML. Instead of generating it from scratch, the crate clones a real set saved by
Live 12.4.6 (`templates/live12-base.als.xml`, exposed as `TEMPLATE`) and injects
your tracks, Session clips, an optional Arrangement clip per track, scenes, and
the tempo map.

It targets Live 12.4 (set schema `12.0_12402`). For another Live version,
re-save the template from that version.

Pure Rust with one dependency (`flate2`), so it also builds for
`wasm32-unknown-unknown`.

## Usage

```rust
use expressive_liveset::{template_tracks, write, Clip, Note, Scene, Set, TimeSig, Track, TEMPLATE};

// A C that bends up a whole tone over its first beat.
let note = Note {
    pitch: 60,
    start: 0.0,
    dur: 2.0,
    velocity: 100,
    pitch_curve: vec![(0.0, 0.0), (1.0, 2.0)], // (beats from note start, semitones)
};
let clip = Clip {
    name: "Bend".into(),
    start: 0.0,
    length: 4.0,
    time_sig: TimeSig { numerator: 4, denominator: 4 },
    notes: vec![note],
};

let set = Set {
    tempo: 120.0,
    tempo_changes: vec![],
    scenes: vec![Scene { name: "A".into(), tempo: 120.0 }],
    tracks: vec![Track {
        // Each track is cloned from a <MidiTrack> block of the template.
        template: template_tracks(TEMPLATE)[0],
        name: "Lead".into(),
        session: vec![Some(clip.clone())], // one slot per scene
        arrangement: Some(clip),
    }],
};

let bytes = write(&set, TEMPLATE).expect("write set");
std::fs::write(std::env::temp_dir().join("bend.als"), bytes).unwrap();
```

Pitch curves use Live's MPE per-note pitch with a 48 semitone bend range, which
matches the template's MIDI track.

You can pass your own template instead of `TEMPLATE`: any gunzipped Live 12.4
set with at least one MIDI track. Everything outside the MIDI tracks (return
tracks, main track, settings) is kept from the template.

## Instruments (opt-in)

Tracks cloned from `TEMPLATE` have no instrument. To make a track play out of
the box, pick one per track with `with_instrument` and pass the returned block
as the track's `template`:

```rust
use expressive_liveset::{template_tracks, with_instrument, Instrument, TEMPLATE};

let lead = with_instrument(TEMPLATE, template_tracks(TEMPLATE)[0], Instrument::Melodic).unwrap();
let kit = with_instrument(TEMPLATE, template_tracks(TEMPLATE)[0], Instrument::Drums).unwrap();
// Track { template: &lead, .. }, Track { template: &kit, .. }
```

- `Instrument::Melodic`: Live's stock Tension, default preset.
- `Instrument::Drums`: Live's stock 909 Core Kit, so drum parts sound as the
  set opens. Its samples are referenced only by their place in Live's Core
  Library, which every install has. (`DRUM_RACK` is still there for an empty
  Drum Rack.)
- `Instrument::None` (the default): the block is returned unchanged, so the
  written set is byte-for-byte what it was without the call.

The device goes right after the track's Pitch rack. Its ids are moved above
every id in the template, then offset per track like the rest of the block.
The devices live in `templates/devices/`, extracted from a set saved by Live
12.4.6 with `examples/extract_device.rs`. Their only paths are Live's own
built-in device references.

Suggested use: voxmpe gives its one track `Melodic`; tabridge gives drum parts
`Drums` and every other part `Melodic`.

## Tunings (opt-in)

A set can carry a Live 12 tuning system, so it opens with the tuning loaded.
With a tuning, note pitches are steps of the tuning (MIDI note 0 is the lowest
note of its range, each MIDI note up is one step) and pitch curves bend from
those steps.

```rust
use expressive_liveset::{write_tuned, Anchor, TuningSystem};
# use expressive_liveset::{template_tracks, Clip, Scene, Set, TimeSig, Track, TEMPLATE};
# let set = Set { tempo: 120.0, tempo_changes: vec![], scenes: vec![], tracks: vec![] };

let scl = "! 19-edo\n19-EDO\n 19\n 63.157895\n 126.315789\n 189.473684\n 252.631579\n 315.789474\n 378.947368\n 442.105263\n 505.263158\n 568.421053\n 631.578947\n 694.736842\n 757.894737\n 821.052632\n 884.210526\n 947.368421\n 1010.526316\n 1073.684211\n 1136.842105\n 2/1\n";
// The 1/1 at middle C (octave 3 in Live's naming), MIDI note 0 = 60 steps below it.
let anchor = Anchor { octave: 3, hz: 261.625565, lowest_step: -60 };
let tuning = TuningSystem::from_scala("19-edo", scl, anchor).unwrap();
let bytes = write_tuned(&set, TEMPLATE, Some(&tuning)).unwrap();
```

`TuningSystem::from_cents` takes the degrees in cents instead, and every field
is public for other layouts (a reference note other than the 1/1, note names).
`write_tuned(.., None)` is byte-for-byte `write`. See
[docs/live12-tuning.md](docs/live12-tuning.md) for how Live stores a tuning.

## Depending on it

There is no crates.io release. Depend on it by git and pin a commit:

```toml
[dependencies]
expressive-liveset = { git = "https://github.com/swwallowws/expressive-liveset.git", rev = "<commit>" }
```

## Licence

The code is MIT, see `LICENSE`.

`templates/live12-base.als.xml` is a set file saved by Ableton Live 12.4.6. It
is the file-format skeleton the writer needs: one MIDI track with a Pitch device
in a MIDI Effect Rack, and the main track. It has no return tracks, an empty
groove pool, no samples and no preset, rack or groove files. The only path it
names is Live's built-in Pitch device inside the Live application.
`templates/devices/` holds two stock Live devices (Tension and an empty Drum
Rack, default presets) as Live 12.4.6 saved them, with the same kind of
built-in reference and no samples.

This project is not affiliated with Ableton. Ableton and Live are trademarks of
Ableton AG.
