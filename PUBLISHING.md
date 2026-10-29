# Publishing checklist

`swwallowws/expressive-liveset` is public as of 2026-09-29. It was published
from a fresh history: one commit of the tree as it stood at `f96160e`. The
full earlier history (including the old template with return tracks, groove
timings and a third-party preset path) stays in the private
archive repo `swwallowws/expressive-liveset-archive`.

## Done

- [x] **LICENSE.** `Cargo.toml` already said `license = "MIT"`; added the MIT
  `LICENSE` file (Copyright (c) 2026 Bengisu Ozaydin).
- [x] **README.** What it is, a usage example, how to depend on it by git, and a
  licence note. The example is compiled and run by `cargo test` (a
  `#[cfg(doctest)]` hook at the end of `src/lib.rs`).
- [x] **`.gitignore`** covers `target/` and `.claude/`.
- [x] **No secrets.** Searched for `sk-`, `ghp_`, `hf_`, `AKIA`,
  `PRIVATE KEY`, `api_key`, `apikey`, `password`, `secret`, `token`: no
  matches.
- [x] **No binaries.** Only text files: Rust sources, Cargo files, docs and
  Live's XML (template, device assets, a tuning fixture). No `.als`, `.adg`,
  `.adv`, `.wav`, `.aif`, `.mid` or archives.
- [x] **None of your own paths or names in files.** No `/Users/` path, no
  real name outside `LICENSE`, no GitHub handle in any tracked file.
- [x] **Tests and wasm build.** `cargo test` passes (integration tests and the
  README doc-tests). `cargo build --target wasm32-unknown-unknown` builds.
- [x] **Clean template.** `templates/live12-base.als.xml` was re-saved in
  Ableton Live 12.4.6 after deleting both return tracks and clearing the
  groove pool. One MIDI track "Guitar" with a MIDI Effect Rack holding Live's
  built-in Pitch device, no instrument, no returns, sends or grooves. The only
  preset path is the built-in Pitch device inside the Live app. `tests/set.rs`
  checks written sets carry no return tracks, sends, groove or `/Users/` path.
- [x] **Opt-in instruments.** `with_instrument(template, track,
  Instrument::{None, Melodic, Drums})` puts a stock device (Tension or Drum
  Rack, default presets, extracted from a set saved in Live 12.4.6) after a
  track's Pitch rack. `None` is the default and leaves output byte-for-byte
  unchanged.
- [x] **Written sets open in Live.** Bengisu opened sets written with the
  clean template in Live 12.4.6: they load with no return tracks and no
  errors, and Tension plays the pitch bends.
- [x] **Tuning systems.** The 53-EDO tuning a set carries matched the one in
  a set Bengisu saved in Live (`tests/fixtures/live12-53edo-tuning.xml`).
- [x] **Old template history dropped.** Published from a fresh single-commit
  history; the old history is only in the private archive.
- [x] **Dependents pin the new public commit over HTTPS.** voxmpe (Starling)
  and tabridge (Ready Set) use
  `git = "https://github.com/swwallowws/expressive-liveset.git"` with the
  public commit as `rev`, and no longer need `git-fetch-with-cli`.
- [x] **Visibility.** Public.
- [x] **A playable default kit.** `Instrument::Drums` is Live's 909 Core Kit
  (`templates/devices/drum-kit.xml`), from a set Bengisu saved in Live 12.4 on
  2026-10-29 and cleaned by `examples/clean_kit.py`: every absolute path
  blanked (they named that machine's Live app, and Ableton staff folders left
  in the kit's effect presets), the samples kept as Core Library references.
  A set written by `examples/drum_kit_set.rs` opened in Live with no missing
  samples, the 909 kit on its 16 pads, and the beat sounding.
  `tests/set.rs` checks the kit has 16 pads, pack-relative samples and no
  absolute path.

## Known limitations

- **The kit is 1.3 MB of XML** in every build that writes sets (indentation
  already stripped), a few hundred KB once served compressed.
- **Template says `Creator="Ableton Live 12.4.6"`.** Not checked in an older
  12.4.x.
