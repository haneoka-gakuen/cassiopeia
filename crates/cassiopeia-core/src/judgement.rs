use serde::{Deserialize, Serialize};

use crate::assist::{AssistLevel, timing_windows};
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
#[repr(i8)]
#[serde(try_from = "i8", into = "i8")]
pub enum JudgeTiming {
    None = 0,
    Fast = 1,
    Late = 2,
    Auto = 3,
    OutOfTime = 4,
    LastTiming = 5,
    Force = 6,
}

impl From<JudgeTiming> for i8 {
    fn from(timing: JudgeTiming) -> Self {
        timing as Self
    }
}

impl TryFrom<i8> for JudgeTiming {
    type Error = String;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Fast),
            2 => Ok(Self::Late),
            3 => Ok(Self::Auto),
            4 => Ok(Self::OutOfTime),
            5 => Ok(Self::LastTiming),
            6 => Ok(Self::Force),
            _ => Err(format!("invalid judge timing value: {value}")),
        }
    }
}

/// Selects one of the built-in timing-window sets.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[repr(i16)]
#[serde(rename_all = "camelCase")]
pub enum NoteJudgementType {
    None = 0,
    Normal = 1,
    EasyNormal = 2,
    Flick = 5,
    SlideBegin = 10,
    SlideEnd = 11,
    SlideEndFlick = 12,
    SlideBeginEasy = 15,
    Trace = 21,
    SlideEndTrace = 22,
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
        Self::for_assist(AssistLevel::Level0, note_type)
    }

    pub fn for_assist(assist: AssistLevel, note_type: NoteJudgementType) -> Option<Self> {
        let windows = timing_windows(assist, note_type);
        (!windows.is_empty()).then(|| Self {
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
    judge_with_assist(AssistLevel::Level0, note_type, difference)
}

pub fn judge_with_assist(
    assist: AssistLevel,
    note_type: NoteJudgementType,
    difference: TimeMicros,
) -> JudgeResult {
    let windows = timing_windows(assist, note_type);
    if windows.is_empty() {
        return JudgeResult {
            judgement: Judgement::Miss,
            timing: JudgeTiming::OutOfTime,
        };
    }
    judge_windows(windows, difference)
}

pub fn judgement_windows(
    assist: AssistLevel,
    note_type: NoteJudgementType,
) -> &'static [JudgementWindow] {
    timing_windows(assist, note_type)
}

pub fn maximum_early_ms(note_type: NoteJudgementType) -> i32 {
    maximum_early_ms_with_assist(AssistLevel::Level0, note_type)
}

pub fn maximum_early_ms_with_assist(assist: AssistLevel, note_type: NoteJudgementType) -> i32 {
    timing_windows(assist, note_type)
        .iter()
        .map(|window| window.early_ms)
        .max()
        .unwrap_or(0)
}

pub fn maximum_late_ms(note_type: NoteJudgementType) -> i32 {
    maximum_late_ms_with_assist(AssistLevel::Level0, note_type)
}

pub fn maximum_late_ms_with_assist(assist: AssistLevel, note_type: NoteJudgementType) -> i32 {
    timing_windows(assist, note_type)
        .iter()
        .map(|window| window.late_ms)
        .max()
        .unwrap_or(0)
}

pub fn is_within_window(note_type: NoteJudgementType, difference: TimeMicros) -> bool {
    is_within_window_with_assist(AssistLevel::Level0, note_type, difference)
}

pub fn is_within_window_with_assist(
    assist: AssistLevel,
    note_type: NoteJudgementType,
    difference: TimeMicros,
) -> bool {
    let windows = timing_windows(assist, note_type);
    let early = i64::from(maximum_early_ms_with_assist(assist, note_type)) * 1_000;
    let late = i64::from(maximum_late_ms_with_assist(assist, note_type)) * 1_000;
    !windows.is_empty() && difference.0 >= -early && difference.0 <= late
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
    fn every_assist_level_has_its_normal_late_boundary() {
        let boundaries = [130, 133, 137, 143, 149, 156];
        for (assist, boundary) in AssistLevel::ALL.into_iter().zip(boundaries) {
            assert_eq!(
                judge_with_assist(
                    assist,
                    NoteJudgementType::Normal,
                    TimeMicros(i64::from(boundary) * 1_000)
                )
                .judgement,
                Judgement::Miss
            );
            assert_eq!(
                judge_with_assist(
                    assist,
                    NoteJudgementType::Normal,
                    TimeMicros(i64::from(boundary) * 1_000 + 1)
                )
                .timing,
                JudgeTiming::OutOfTime
            );
        }
    }

    #[test]
    fn level_five_slide_begin_keeps_its_non_monotonic_profile() {
        assert_eq!(
            judge_with_assist(
                AssistLevel::Level4,
                NoteJudgementType::SlideBegin,
                TimeMicros(109_000)
            )
            .judgement,
            Judgement::Good
        );
        assert_eq!(
            judge_with_assist(
                AssistLevel::Level5,
                NoteJudgementType::SlideBegin,
                TimeMicros(109_000)
            )
            .judgement,
            Judgement::Bad
        );
        assert_eq!(
            maximum_late_ms_with_assist(AssistLevel::Level5, NoteJudgementType::SlideBegin),
            130
        );
    }

    #[test]
    fn compatibility_wrappers_are_exactly_level_zero() {
        let note_types = [
            NoteJudgementType::Normal,
            NoteJudgementType::SlideBegin,
            NoteJudgementType::SlideEnd,
            NoteJudgementType::Flick,
            NoteJudgementType::SlideEndFlick,
            NoteJudgementType::Trace,
            NoteJudgementType::SlideEndTrace,
            NoteJudgementType::EasyNormal,
            NoteJudgementType::SlideBeginEasy,
        ];
        for note_type in note_types {
            assert_eq!(
                JudgementProfile::for_note_type(note_type),
                JudgementProfile::for_assist(AssistLevel::Level0, note_type)
            );
            assert_eq!(
                maximum_early_ms(note_type),
                maximum_early_ms_with_assist(AssistLevel::Level0, note_type)
            );
            assert_eq!(
                maximum_late_ms(note_type),
                maximum_late_ms_with_assist(AssistLevel::Level0, note_type)
            );
            for difference in [-250_000, -1_001, 0, 1_001, 250_000] {
                assert_eq!(
                    judge(note_type, TimeMicros(difference)),
                    judge_with_assist(AssistLevel::Level0, note_type, TimeMicros(difference))
                );
            }
        }
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

        let timing_codes = [
            (JudgeTiming::None, 0),
            (JudgeTiming::Fast, 1),
            (JudgeTiming::Late, 2),
            (JudgeTiming::Auto, 3),
            (JudgeTiming::OutOfTime, 4),
            (JudgeTiming::LastTiming, 5),
            (JudgeTiming::Force, 6),
        ];
        for (timing, code) in timing_codes {
            assert_eq!(i8::from(timing), code);
            assert_eq!(JudgeTiming::try_from(code).unwrap(), timing);
            assert_eq!(serde_json::to_value(timing).unwrap(), code);
        }
        assert!(JudgeTiming::try_from(7).is_err());

        let result = JudgeResult {
            judgement: Judgement::Perfect,
            timing: JudgeTiming::OutOfTime,
        };
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            serde_json::json!({
                "judgement": 5,
                "timing": 4
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
