use serde::{Deserialize, Serialize};

use crate::timing::TimeMicros;

/// Stable gameplay judgement values shared by browser and native hosts.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[repr(i8)]
#[serde(try_from = "i8", into = "i8")]
pub enum Judgement {
    None = -1,
    Wait = 0,
    Miss = 1,
    Bad = 2,
    Good = 3,
    Great = 4,
    Perfect = 5,
    Just = 6,
    Pass = 7,
}

impl From<Judgement> for i8 {
    fn from(judgement: Judgement) -> Self {
        judgement as Self
    }
}

impl TryFrom<i8> for Judgement {
    type Error = String;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            -1 => Ok(Self::None),
            0 => Ok(Self::Wait),
            1 => Ok(Self::Miss),
            2 => Ok(Self::Bad),
            3 => Ok(Self::Good),
            4 => Ok(Self::Great),
            5 => Ok(Self::Perfect),
            6 => Ok(Self::Just),
            7 => Ok(Self::Pass),
            _ => Err(format!("invalid judgement value: {value}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JudgeTiming {
    None,
    Fast,
    Late,
    Auto,
    OutOfTime,
}

/// Selects one of the built-in timing-window sets.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteJudgementType {
    None,
    Normal,
    EasyNormal,
    Flick,
    SlideBegin,
    SlideEnd,
    SlideEndFlick,
    SlideBeginEasy,
    Trace,
    SlideEndTrace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct JudgementWindow {
    pub judgement: Judgement,
    /// Inclusive early allowance in integer milliseconds.
    pub early_ms: i32,
    /// Inclusive late allowance in integer milliseconds.
    pub late_ms: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct JudgeResult {
    pub judgement: Judgement,
    pub timing: JudgeTiming,
}

/// Ordered strictest-first judgement windows.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "JudgementProfileWire", rename_all = "camelCase")]
pub struct JudgementProfile {
    windows: Vec<JudgementWindow>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct JudgementProfileWire {
    windows: Vec<JudgementWindow>,
}

impl TryFrom<JudgementProfileWire> for JudgementProfile {
    type Error = String;

    fn try_from(wire: JudgementProfileWire) -> Result<Self, Self::Error> {
        Self::new(wire.windows)
    }
}

impl JudgementProfile {
    pub fn new(windows: impl IntoIterator<Item = JudgementWindow>) -> Result<Self, String> {
        let windows: Vec<_> = windows.into_iter().collect();
        if windows.is_empty() {
            return Err("a judgement profile needs at least one window".into());
        }
        if windows
            .iter()
            .any(|window| window.early_ms < 0 || window.late_ms < 0)
        {
            return Err("judgement windows cannot be negative".into());
        }
        Ok(Self { windows })
    }

    /// The normal timing profile retained for source compatibility.
    pub fn our_notes_normal() -> Self {
        Self::for_note_type(NoteJudgementType::Normal)
            .expect("the normal note type has a built-in profile")
    }

    pub fn for_note_type(note_type: NoteJudgementType) -> Option<Self> {
        built_in_windows(note_type).map(|windows| Self {
            windows: windows.to_vec(),
        })
    }

    pub fn windows(&self) -> &[JudgementWindow] {
        &self.windows
    }

    pub fn maximum_early_ms(&self) -> i32 {
        self.windows
            .iter()
            .map(|window| window.early_ms)
            .max()
            .unwrap_or(0)
    }

    pub fn maximum_late_ms(&self) -> i32 {
        self.windows
            .iter()
            .map(|window| window.late_ms)
            .max()
            .unwrap_or(0)
    }

    /// Judges an integer-millisecond difference for compatibility callers.
    pub fn judge(&self, difference_ms: i32) -> JudgeResult {
        self.judge_micros(TimeMicros(i64::from(difference_ms) * 1_000))
    }

    /// Judges a quantized time difference without host floating-point state.
    pub fn judge_micros(&self, difference: TimeMicros) -> JudgeResult {
        judge_windows(&self.windows, difference)
    }

    pub fn contains(&self, difference: TimeMicros) -> bool {
        let early = i64::from(self.maximum_early_ms()) * 1_000;
        let late = i64::from(self.maximum_late_ms()) * 1_000;
        difference.0 >= -early && difference.0 <= late
    }
}

pub fn judge(note_type: NoteJudgementType, difference: TimeMicros) -> JudgeResult {
    built_in_windows(note_type).map_or(
        JudgeResult {
            judgement: Judgement::Miss,
            timing: JudgeTiming::OutOfTime,
        },
        |windows| judge_windows(windows, difference),
    )
}

pub fn maximum_early_ms(note_type: NoteJudgementType) -> i32 {
    built_in_windows(note_type)
        .and_then(|windows| windows.iter().map(|window| window.early_ms).max())
        .unwrap_or(0)
}

pub fn maximum_late_ms(note_type: NoteJudgementType) -> i32 {
    built_in_windows(note_type)
        .and_then(|windows| windows.iter().map(|window| window.late_ms).max())
        .unwrap_or(0)
}

pub fn is_within_window(note_type: NoteJudgementType, difference: TimeMicros) -> bool {
    let early = i64::from(maximum_early_ms(note_type)) * 1_000;
    let late = i64::from(maximum_late_ms(note_type)) * 1_000;
    built_in_windows(note_type).is_some() && difference.0 >= -early && difference.0 <= late
}

fn built_in_windows(note_type: NoteJudgementType) -> Option<&'static [JudgementWindow]> {
    match note_type {
        NoteJudgementType::Normal | NoteJudgementType::SlideBegin => Some(&NORMAL),
        NoteJudgementType::Flick | NoteJudgementType::SlideEndFlick => Some(&FLICK),
        NoteJudgementType::SlideEnd => Some(&SLIDE_END),
        NoteJudgementType::EasyNormal | NoteJudgementType::SlideBeginEasy => Some(&EASY),
        NoteJudgementType::Trace | NoteJudgementType::SlideEndTrace => Some(&TRACE),
        NoteJudgementType::None => None,
    }
}

fn judge_windows(windows: &[JudgementWindow], difference: TimeMicros) -> JudgeResult {
    for window in windows {
        let early = i64::from(window.early_ms) * 1_000;
        let late = i64::from(window.late_ms) * 1_000;
        if difference.0 < -early || difference.0 > late {
            continue;
        }
        return JudgeResult {
            judgement: window.judgement,
            timing: timing_for_difference(difference),
        };
    }
    JudgeResult {
        judgement: Judgement::Miss,
        timing: JudgeTiming::OutOfTime,
    }
}

fn timing_for_difference(difference: TimeMicros) -> JudgeTiming {
    if (-1_000..=1_000).contains(&difference.0) {
        JudgeTiming::None
    } else if difference.0 < 0 {
        JudgeTiming::Fast
    } else {
        JudgeTiming::Late
    }
}

const fn symmetric(judgement: Judgement, milliseconds: i32) -> JudgementWindow {
    asymmetric(judgement, milliseconds, milliseconds)
}

const fn asymmetric(judgement: Judgement, early_ms: i32, late_ms: i32) -> JudgementWindow {
    JudgementWindow {
        judgement,
        early_ms,
        late_ms,
    }
}

const NORMAL: [JudgementWindow; 6] = [
    symmetric(Judgement::Just, 1),
    symmetric(Judgement::Perfect, 42),
    symmetric(Judgement::Great, 83),
    symmetric(Judgement::Good, 108),
    symmetric(Judgement::Bad, 125),
    symmetric(Judgement::Miss, 130),
];

const FLICK: [JudgementWindow; 6] = [
    symmetric(Judgement::Just, 1),
    asymmetric(Judgement::Perfect, 83, 58),
    asymmetric(Judgement::Great, 0, 83),
    asymmetric(Judgement::Good, 0, 108),
    asymmetric(Judgement::Bad, 0, 125),
    asymmetric(Judgement::Miss, 0, 130),
];

const SLIDE_END: [JudgementWindow; 5] = [
    asymmetric(Judgement::Perfect, 42, 66),
    asymmetric(Judgement::Great, 99, 166),
    asymmetric(Judgement::Good, 124, 191),
    asymmetric(Judgement::Bad, 141, 208),
    symmetric(Judgement::Miss, 150),
];

const EASY: [JudgementWindow; 3] = [
    symmetric(Judgement::Just, 1),
    asymmetric(Judgement::Perfect, 58, 66),
    asymmetric(Judgement::Miss, 58, 130),
];

const TRACE: [JudgementWindow; 2] = [
    asymmetric(Judgement::Perfect, 58, 66),
    asymmetric(Judgement::Miss, 58, 130),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_boundaries_are_inclusive() {
        let profile = JudgementProfile::our_notes_normal();
        assert_eq!(profile.judge(-42).judgement, Judgement::Perfect);
        assert_eq!(profile.judge(42).judgement, Judgement::Perfect);
        assert_eq!(profile.judge(43).judgement, Judgement::Great);
        assert_eq!(profile.judge(130).judgement, Judgement::Miss);
        assert_eq!(profile.judge(131).timing, JudgeTiming::OutOfTime);
    }

    #[test]
    fn sub_millisecond_differences_use_quantized_exact_timing() {
        let profile = JudgementProfile::our_notes_normal();
        assert_eq!(
            profile.judge_micros(TimeMicros(-1_001)).timing,
            JudgeTiming::Fast
        );
        assert_eq!(
            profile.judge_micros(TimeMicros(1_000)).timing,
            JudgeTiming::None
        );
        assert_eq!(
            profile.judge_micros(TimeMicros(1_001)).timing,
            JudgeTiming::Late
        );
    }

    #[test]
    fn asymmetric_flick_boundaries_match_the_profile() {
        assert_eq!(
            judge(NoteJudgementType::Flick, TimeMicros(-83_000)).judgement,
            Judgement::Perfect
        );
        assert_eq!(
            judge(NoteJudgementType::Flick, TimeMicros(58_000)).judgement,
            Judgement::Perfect
        );
        assert_eq!(
            judge(NoteJudgementType::Flick, TimeMicros(58_001)).judgement,
            Judgement::Great
        );
        assert_eq!(
            judge(NoteJudgementType::Flick, TimeMicros(-83_001)).timing,
            JudgeTiming::OutOfTime
        );
    }

    #[test]
    fn slide_end_late_windows_remain_strictest_first() {
        assert_eq!(
            judge(NoteJudgementType::SlideEnd, TimeMicros(166_000)).judgement,
            Judgement::Great
        );
        assert_eq!(
            judge(NoteJudgementType::SlideEnd, TimeMicros(166_001)).judgement,
            Judgement::Good
        );
        assert_eq!(
            judge(NoteJudgementType::SlideEnd, TimeMicros(208_000)).judgement,
            Judgement::Bad
        );
        assert_eq!(
            judge(NoteJudgementType::SlideEnd, TimeMicros(208_001)).timing,
            JudgeTiming::OutOfTime
        );
    }

    #[test]
    fn easy_and_trace_profiles_have_distinct_exact_results() {
        assert_eq!(
            judge(NoteJudgementType::EasyNormal, TimeMicros(0)).judgement,
            Judgement::Just
        );
        assert_eq!(
            judge(NoteJudgementType::Trace, TimeMicros(0)).judgement,
            Judgement::Perfect
        );
        assert_eq!(maximum_early_ms(NoteJudgementType::Flick), 83);
        assert_eq!(maximum_late_ms(NoteJudgementType::SlideEnd), 208);
    }

    #[test]
    fn judgement_codes_and_serde_match_the_stable_numeric_contract() {
        let codes = [
            (Judgement::None, -1),
            (Judgement::Wait, 0),
            (Judgement::Miss, 1),
            (Judgement::Bad, 2),
            (Judgement::Good, 3),
            (Judgement::Great, 4),
            (Judgement::Perfect, 5),
            (Judgement::Just, 6),
            (Judgement::Pass, 7),
        ];
        for (judgement, code) in codes {
            assert_eq!(i8::from(judgement), code);
            assert_eq!(Judgement::try_from(code).unwrap(), judgement);
            assert_eq!(serde_json::to_value(judgement).unwrap(), code);
        }
        assert!(Judgement::try_from(8).is_err());

        let result = JudgeResult {
            judgement: Judgement::Perfect,
            timing: JudgeTiming::OutOfTime,
        };
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            serde_json::json!({
                "judgement": 5,
                "timing": "outOfTime"
            })
        );
    }

    #[test]
    fn profile_deserialization_preserves_window_invariants() {
        let invalid = serde_json::json!({
            "windows": [{
                "judgement": 5,
                "earlyMs": -1,
                "lateMs": 42
            }]
        });
        assert!(serde_json::from_value::<JudgementProfile>(invalid).is_err());
    }
}
