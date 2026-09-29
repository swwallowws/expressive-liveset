//! A tuning system carried by the set, the way Live 12 stores one it has
//! loaded (see `docs/live12-tuning.md`).
//!
//! Live keeps the loaded tuning in `<LiveSet>/<TuningSystems>`: an empty
//! element in 12-TET, one `<TuningSystem Id="0">` otherwise. With a tuning
//! loaded, MIDI note numbers are steps of the tuning: MIDI note 0 plays the
//! first note of [`TuningSystem::lowest`], and each MIDI note up is one step.

/// A note of the tuning: its pseudo-octave number and its index within that
/// pseudo-octave (0 = the 1/1). Octaves count the way Live names notes, where
/// middle C's octave is 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotePosition {
    pub octave: i32,
    pub index: u32,
}

/// An exact frequency ratio, `numerator / denominator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ratio {
    pub numerator: u64,
    pub denominator: u64,
}

/// One note of the pseudo-octave.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteTuning {
    /// Cents above the 1/1.
    pub cents: f64,
    /// The exact ratio, when the scale gives one. Live writes `1/1` for the
    /// first note whatever the scale says.
    pub ratio: Option<Ratio>,
}

/// Where a scale sits on Live's notes, the way Starling's `.ascl` export pins
/// it: the scale's 1/1 sounds at `hz` in pseudo-octave `octave`, and MIDI note
/// 0 plays the step `lowest_step` counted from that 1/1 (negative = below).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub octave: i32,
    pub hz: f64,
    pub lowest_step: i64,
}

/// Number of MIDI notes; the allowed range spans this many steps.
pub const MIDI_NOTES: i64 = 128;

/// A tuning system as Live 12 saves it in a set.
#[derive(Debug, Clone, PartialEq)]
pub struct TuningSystem {
    /// Shown as the tuning's name in Live (`TuningSystemName`).
    pub name: String,
    /// `ShortDescription`: the scale's description line.
    pub description: String,
    /// One entry per note of the pseudo-octave, ascending, `[0]` at 0 cents.
    pub notes: Vec<NoteTuning>,
    /// The pseudo-octave (period) in cents: 1200 for an octave.
    pub pseudo_octave_cents: f64,
    pub pseudo_octave_ratio: Option<Ratio>,
    /// The note pinned to [`TuningSystem::reference_hz`].
    pub reference: NotePosition,
    pub reference_hz: f64,
    /// The note on MIDI note 0. The allowed range runs [`MIDI_NOTES`] steps up
    /// from here.
    pub lowest: NotePosition,
    /// One name per note; empty names each note by its index.
    pub note_names: Vec<String>,
}

impl TuningSystem {
    /// A tuning from cents: `degrees` are the notes of one pseudo-octave
    /// (`[0]` = 0, the 1/1; sorted here), `period` its size in cents.
    pub fn from_cents(
        name: &str,
        description: &str,
        degrees: &[f64],
        period: f64,
        anchor: Anchor,
    ) -> Result<TuningSystem, String> {
        let notes = degrees
            .iter()
            .map(|&cents| NoteTuning { cents, ratio: None })
            .collect();
        TuningSystem::build(name, description, notes, (period, None), anchor)
    }

    /// A tuning from the text of a Scala `.scl` file. Ratio entries keep their
    /// exact ratio; cents entries (with a `.`) have none.
    pub fn from_scala(name: &str, scl: &str, anchor: Anchor) -> Result<TuningSystem, String> {
        let data: Vec<&str> = scl
            .lines()
            .map(str::trim)
            .filter(|l| !l.starts_with('!'))
            .collect();
        if data.len() < 2 {
            return Err("scl needs a description line and a note count".into());
        }
        let count: usize = data[1]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .parse()
            .map_err(|e| format!("bad note count '{}': {e}", data[1]))?;
        if count == 0 {
            return Err("scl has no notes".into());
        }
        let pitches = data.get(2..2 + count).ok_or_else(|| {
            format!("declared {count} notes, found {}", data.len().saturating_sub(2))
        })?;
        let mut entries = pitches
            .iter()
            .map(|l| parse_pitch(l))
            .collect::<Result<Vec<_>, _>>()?;
        let period = entries.pop().expect("count >= 1");
        let mut notes = vec![NoteTuning {
            cents: 0.0,
            ratio: None,
        }];
        notes.extend(
            entries
                .into_iter()
                .map(|(cents, ratio)| NoteTuning { cents, ratio }),
        );
        TuningSystem::build(name, data[0], notes, period, anchor)
    }

    fn build(
        name: &str,
        description: &str,
        mut notes: Vec<NoteTuning>,
        (period, period_ratio): (f64, Option<Ratio>),
        anchor: Anchor,
    ) -> Result<TuningSystem, String> {
        if notes.is_empty() {
            return Err("a tuning needs at least one note".into());
        }
        if !(period > 0.0) || notes.iter().any(|n| !n.cents.is_finite()) {
            return Err("tuning cents must be finite and the period positive".into());
        }
        if !(anchor.hz > 0.0) {
            return Err("the reference frequency must be positive".into());
        }
        notes.sort_by(|a, b| a.cents.total_cmp(&b.cents));
        let n = notes.len() as i64;
        let at = |step: i64| NotePosition {
            octave: anchor.octave + step.div_euclid(n) as i32,
            index: step.rem_euclid(n) as u32,
        };
        // Live writes a 2/1 pseudo-octave for an octave scale.
        let pseudo_octave_ratio = period_ratio.or((period == 1200.0).then_some(Ratio {
            numerator: 2,
            denominator: 1,
        }));
        Ok(TuningSystem {
            name: name.to_string(),
            description: description.to_string(),
            notes,
            pseudo_octave_cents: period,
            pseudo_octave_ratio,
            reference: at(0),
            reference_hz: anchor.hz,
            lowest: at(anchor.lowest_step),
            note_names: Vec::new(),
        })
    }

    /// The highest note Live allows: [`MIDI_NOTES`] - 1 steps above
    /// [`TuningSystem::lowest`] (MIDI note 127).
    pub fn highest(&self) -> NotePosition {
        let n = self.notes.len().max(1) as i64;
        let step = self.lowest.octave as i64 * n + self.lowest.index as i64 + MIDI_NOTES - 1;
        NotePosition {
            octave: step.div_euclid(n) as i32,
            index: step.rem_euclid(n) as u32,
        }
    }

    /// The `<TuningSystems>` element Live writes with this tuning loaded.
    pub fn to_xml(&self) -> String {
        let mut o = String::from("<TuningSystems><TuningSystem Id=\"0\"><NoteTunings>");
        for (i, note) in self.notes.iter().enumerate() {
            let ratio = if i == 0 {
                Some(Ratio {
                    numerator: 1,
                    denominator: 1,
                })
            } else {
                note.ratio
            };
            o.push_str(&format!(
                "<NoteTuning Id=\"{i}\"><Cents Value=\"{}\" /><TuningRatio>{}</TuningRatio></NoteTuning>",
                cents(note.cents),
                ratio_value(ratio)
            ));
        }
        o.push_str("</NoteTunings>");
        o.push_str(&format!(
            "<PseudoOctaveInCents Value=\"{}\" /><PseudoOctaveAsRatio>{}</PseudoOctaveAsRatio>",
            cents(self.pseudo_octave_cents),
            ratio_value(self.pseudo_octave_ratio)
        ));
        let pitch = format!(
            "<Octave Value=\"{}\" /><NoteIndexWithinOctave Value=\"{}\" /><FrequencyInHz Value=\"{}\" />",
            self.reference.octave,
            self.reference.index,
            // Full precision: a single-precision 261.625565 would be off.
            crate::fmt(self.reference_hz)
        );
        o.push_str(&format!(
            "<ReferencePitch>{pitch}</ReferencePitch><DefaultReferencePitch>{pitch}</DefaultReferencePitch>"
        ));
        let hi = self.highest();
        o.push_str(&format!(
            "<AllowedNoteRange><MinOctave Value=\"{}\" /><MinNoteIndexWithinOctave Value=\"{}\" />\
<MaxOctave Value=\"{}\" /><MaxNoteIndexWithinOctave Value=\"{}\" /></AllowedNoteRange>",
            self.lowest.octave, self.lowest.index, hi.octave, hi.index
        ));
        o.push_str("<NoteNames>");
        for i in 0..self.notes.len() {
            let name = match self.note_names.get(i) {
                Some(n) => crate::escape_xml(n),
                None => i.to_string(),
            };
            o.push_str(&format!("<RemoteableString Id=\"{i}\" Value=\"{name}\" />"));
        }
        o.push_str("</NoteNames>");
        o.push_str(&format!(
            "<TuningSystemName Value=\"{}\" /><Scales /><ShortDescription Value=\"{}\" />\
<TuningSystemSource Value=\"\" /><Link Value=\"\" /><LomId Value=\"0\" />",
            crate::escape_xml(&self.name),
            crate::escape_xml(&self.description)
        ));
        o.push_str("</TuningSystem></TuningSystems>");
        o
    }
}

/// `<Value>` of a `<TuningRatio>` or `<PseudoOctaveAsRatio>`: empty without a
/// ratio.
fn ratio_value(r: Option<Ratio>) -> String {
    match r {
        Some(r) => format!(
            "<Value><TuningRatio Id=\"0\"><Numerator Value=\"{}\" /><Denominator Value=\"{}\" /></TuningRatio></Value>",
            r.numerator, r.denominator
        ),
        None => "<Value />".to_string(),
    }
}

/// A number the way Live writes tuning values: the single-precision value to
/// five decimals, trailing zeros dropped (`22.64151`, `520.7547`, `1200`).
fn cents(x: f64) -> String {
    let s = format!("{:.5}", x as f32 as f64);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// One Scala pitch line: cents if it has a `.`, else a ratio `a/b` or `a`.
fn parse_pitch(line: &str) -> Result<(f64, Option<Ratio>), String> {
    let tok = line.split_whitespace().next().unwrap_or("");
    if tok.contains('.') {
        let c: f64 = tok
            .parse()
            .map_err(|e| format!("bad cents '{tok}': {e}"))?;
        return Ok((c, None));
    }
    let (n, d) = tok.split_once('/').unwrap_or((tok, "1"));
    let numerator: u64 = n.parse().map_err(|e| format!("bad ratio '{tok}': {e}"))?;
    let denominator: u64 = d.parse().map_err(|e| format!("bad ratio '{tok}': {e}"))?;
    if numerator == 0 || denominator == 0 {
        return Err(format!("zero in ratio '{tok}'"));
    }
    let cents = 1200.0 * (numerator as f64 / denominator as f64).log2();
    Ok((
        cents,
        Some(Ratio {
            numerator,
            denominator,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_written_like_live() {
        assert_eq!(cents(1200.0 / 53.0), "22.64151");
        assert_eq!(cents(1200.0 * 23.0 / 53.0), "520.7547");
        assert_eq!(cents(1200.0 * 51.0 / 53.0), "1154.71704");
        assert_eq!(cents(1200.0), "1200");
        assert_eq!(cents(0.0), "0");
    }

    #[test]
    fn scala_ratios_and_cents() {
        assert_eq!(parse_pitch(" 100.0 ! x").unwrap(), (100.0, None));
        let (c, r) = parse_pitch("3/2").unwrap();
        assert!((c - 701.955).abs() < 1e-3);
        assert_eq!(
            r,
            Some(Ratio {
                numerator: 3,
                denominator: 2
            })
        );
        assert_eq!(parse_pitch("2").unwrap().1.unwrap().numerator, 2);
        assert!(parse_pitch("1/0").is_err());
    }
}
