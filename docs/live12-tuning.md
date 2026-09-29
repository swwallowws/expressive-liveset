# How Live 12 stores a loaded tuning

Findings from a set saved by Ableton Live 12.4 (schema `12.0_12402`, the same
as `templates/live12-base.als.xml`) with Live's built-in 53-EDO tuning loaded,
compared with the template, which has no tuning. Checked 2026-09-28.

## Where it lives

The tuning is a direct child of `<LiveSet>`, between `<ShowVideoWindow>` and
`<TrackHeaderWidth>`:

- no tuning (12-TET): `<TuningSystems />`
- a tuning loaded: `<TuningSystems>` holding one `<TuningSystem Id="0">`

Nothing else in the set changes with the tuning. There is no separate "active"
flag: a `TuningSystem` being present means the set has it loaded. Tracks,
clips and devices are the same in both sets; `MpePitchBendUsesTuning` is
`true` on every track in both, so it is not a tuning switch. Clips keep plain
`MidiKey` numbers, which Live reads as steps of the loaded tuning.

## Skeleton

Values are those Live wrote for its 53-EDO; `…` marks repeats.

```xml
<TuningSystems>
  <TuningSystem Id="0">
    <NoteTunings>
      <NoteTuning Id="0">
        <Cents Value="0" />
        <TuningRatio>
          <Value>
            <TuningRatio Id="0">
              <Numerator Value="1" />
              <Denominator Value="1" />
            </TuningRatio>
          </Value>
        </TuningRatio>
      </NoteTuning>
      <NoteTuning Id="1">
        <Cents Value="22.64151" />
        <TuningRatio>
          <Value />
        </TuningRatio>
      </NoteTuning>
      … one NoteTuning per note of the pseudo-octave (Id 0 to 52) …
    </NoteTunings>
    <PseudoOctaveInCents Value="1200" />
    <PseudoOctaveAsRatio>
      <Value>
        <TuningRatio Id="0">
          <Numerator Value="2" />
          <Denominator Value="1" />
        </TuningRatio>
      </Value>
    </PseudoOctaveAsRatio>
    <ReferencePitch>
      <Octave Value="3" />
      <NoteIndexWithinOctave Value="40" />
      <FrequencyInHz Value="440" />
    </ReferencePitch>
    <DefaultReferencePitch>
      … same three fields …
    </DefaultReferencePitch>
    <AllowedNoteRange>
      <MinOctave Value="2" />
      <MinNoteIndexWithinOctave Value="24" />
      <MaxOctave Value="4" />
      <MaxNoteIndexWithinOctave Value="45" />
    </AllowedNoteRange>
    <NoteNames>
      <RemoteableString Id="0" Value="(name of note 0)" />
      … one per note …
    </NoteNames>
    <TuningSystemName Value="53-EDO" />
    <Scales />
    <ShortDescription Value="(one line of text)" />
    <TuningSystemSource Value="" />
    <Link Value="(a URL for Live's built-in tunings)" />
    <LomId Value="0" />
  </TuningSystem>
</TuningSystems>
```

## What the fields mean

- `NoteTunings`: one entry per note of the pseudo-octave, the 1/1 first.
  `Cents` is always written, as a single-precision number to five decimals
  with trailing zeros dropped (`22.64151`, `520.7547`, `1154.71704`, `1200`).
  `TuningRatio/Value` holds an exact ratio when there is one; the first note
  has `1/1`, the other equal-division notes an empty `<Value />`.
- `PseudoOctaveInCents` / `PseudoOctaveAsRatio`: the period (the scale's
  last `.scl` line), in cents and, when exact, as a ratio.
- `ReferencePitch`: the note pinned to a frequency, as (pseudo-octave number,
  index within the pseudo-octave, Hz). Octave numbers follow Live's note
  names, where middle C's octave is 3: Live's 53-EDO pins index 40 (A) of
  octave 3 to 440 Hz, so index 0 of octave 3 is about 261.4 Hz. This is the
  `.ascl` line `@ABL REFERENCE_PITCH <octave> <index> <Hz>`.
- `DefaultReferencePitch`: the same three fields; what the reference resets to.
- `AllowedNoteRange`: the tuning's notes that MIDI notes reach, as (octave,
  index) of the lowest and highest. It spans exactly 128 steps
  ((4 - 2) x 53 + 45 - 24 = 127), and the reference A sits 69 steps above the
  lowest ((3 - 2) x 53 + 40 - 24 = 69), so MIDI note 69 plays A 440 Hz: MIDI
  note 0 is the lowest note of the range and each MIDI note up is one step.
  This is the `.ascl` line `@ABL NOTE_RANGE_BY_INDEX`.
- `NoteNames`: one display name per note.
- `TuningSystemName`, `ShortDescription`, `TuningSystemSource`, `Link`: text
  shown in Live's tuning browser. `Scales` was empty.

## What this crate writes

`TuningSystem::to_xml` writes the same elements in the same order.
`tests/fixtures/live12-53edo-tuning.xml` is the subtree from the saved set
with Ableton's note names replaced by step numbers and the description and
link emptied; `tests/tuning.rs` checks that the crate writes exactly that
subtree (whitespace aside) for 53-EDO.

Two things the saved set does not show, and how the crate handles them:

- Note names for a scale without any: the crate writes each note's index
  (`0`, `1`, …) unless you pass names.
- A non-octave period given in cents: `PseudoOctaveAsRatio` gets an empty
  `<Value />`, the same form Live uses for a note without a ratio.
