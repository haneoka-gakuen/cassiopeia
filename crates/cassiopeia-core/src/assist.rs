use serde::{Deserialize, Serialize};

use crate::judgement::{Judgement, JudgementWindow, NoteJudgementType};

pub const ASSIST_LEVEL_COUNT: usize = 6;
pub const TIMING_WINDOWS_PER_LEVEL: usize = 39;

#[derive(
    Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[repr(u8)]
#[serde(try_from = "u8", into = "u8")]
pub enum AssistLevel {
    #[default]
    Level0 = 0,
    Level1 = 1,
    Level2 = 2,
    Level3 = 3,
    Level4 = 4,
    Level5 = 5,
}

impl AssistLevel {
    pub const ALL: [Self; ASSIST_LEVEL_COUNT] = [
        Self::Level0,
        Self::Level1,
        Self::Level2,
        Self::Level3,
        Self::Level4,
        Self::Level5,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }
}

impl From<AssistLevel> for u8 {
    fn from(level: AssistLevel) -> Self {
        level as Self
    }
}

impl TryFrom<u8> for AssistLevel {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Level0),
            1 => Ok(Self::Level1),
            2 => Ok(Self::Level2),
            3 => Ok(Self::Level3),
            4 => Ok(Self::Level4),
            5 => Ok(Self::Level5),
            _ => Err(format!("invalid assist level: {value}")),
        }
    }
}

pub fn timing_windows(
    assist: AssistLevel,
    note_type: NoteJudgementType,
) -> &'static [JudgementWindow] {
    let base = assist.index() * TIMING_WINDOWS_PER_LEVEL;
    let (offset, length) = match note_type {
        NoteJudgementType::Normal => (0, 6),
        NoteJudgementType::SlideBegin => (6, 6),
        NoteJudgementType::SlideEnd => (12, 5),
        NoteJudgementType::Flick => (17, 6),
        NoteJudgementType::SlideEndFlick => (23, 6),
        NoteJudgementType::Trace => (29, 2),
        NoteJudgementType::SlideEndTrace => (31, 2),
        NoteJudgementType::EasyNormal => (33, 3),
        NoteJudgementType::SlideBeginEasy => (36, 3),
        NoteJudgementType::None => return &[],
    };
    &TIMING_WINDOWS[base + offset..base + offset + length]
}

const fn window(judgement: Judgement, early_ms: i32, late_ms: i32) -> JudgementWindow {
    JudgementWindow {
        judgement,
        early_ms,
        late_ms,
    }
}

// Priority-zero judgement-8 rows from the master are intentionally not part
// of ordinary judgement selection; their native scope is currently unknown.
pub const TIMING_WINDOWS: [JudgementWindow; ASSIST_LEVEL_COUNT * TIMING_WINDOWS_PER_LEVEL] = [
    // Assist level 0.
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 50, 50),
    window(Judgement::Great, 83, 83),
    window(Judgement::Good, 100, 100),
    window(Judgement::Bad, 125, 125),
    window(Judgement::Miss, 130, 130),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 50, 50),
    window(Judgement::Great, 83, 83),
    window(Judgement::Good, 100, 100),
    window(Judgement::Bad, 125, 125),
    window(Judgement::Miss, 130, 130),
    window(Judgement::Perfect, 84, 66),
    window(Judgement::Great, 0, 166),
    window(Judgement::Good, 0, 191),
    window(Judgement::Bad, 0, 208),
    window(Judgement::Miss, 0, 150),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 83, 67),
    window(Judgement::Great, 0, 83),
    window(Judgement::Good, 0, 117),
    window(Judgement::Bad, 0, 125),
    window(Judgement::Miss, 0, 130),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 83, 67),
    window(Judgement::Great, 0, 83),
    window(Judgement::Good, 0, 117),
    window(Judgement::Bad, 0, 125),
    window(Judgement::Miss, 0, 130),
    window(Judgement::Perfect, 67, 67),
    window(Judgement::Miss, 0, 130),
    window(Judgement::Perfect, 67, 67),
    window(Judgement::Miss, 0, 130),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 67, 67),
    window(Judgement::Miss, 58, 130),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 67, 67),
    window(Judgement::Miss, 58, 130),
    // Assist level 1.
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 50, 50),
    window(Judgement::Great, 83, 83),
    window(Judgement::Good, 111, 111),
    window(Judgement::Bad, 128, 128),
    window(Judgement::Miss, 133, 133),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 50, 50),
    window(Judgement::Great, 83, 83),
    window(Judgement::Good, 111, 111),
    window(Judgement::Bad, 128, 128),
    window(Judgement::Miss, 133, 133),
    window(Judgement::Perfect, 84, 66),
    window(Judgement::Great, 0, 166),
    window(Judgement::Good, 0, 196),
    window(Judgement::Bad, 0, 214),
    window(Judgement::Miss, 0, 216),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 88, 67),
    window(Judgement::Great, 0, 83),
    window(Judgement::Good, 0, 120),
    window(Judgement::Bad, 0, 130),
    window(Judgement::Miss, 0, 135),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 88, 67),
    window(Judgement::Great, 0, 83),
    window(Judgement::Good, 0, 120),
    window(Judgement::Bad, 0, 130),
    window(Judgement::Miss, 0, 135),
    window(Judgement::Perfect, 72, 72),
    window(Judgement::Miss, 0, 133),
    window(Judgement::Perfect, 72, 72),
    window(Judgement::Miss, 0, 133),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 72, 72),
    window(Judgement::Miss, 58, 133),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 72, 72),
    window(Judgement::Miss, 58, 133),
    // Assist level 2.
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 50, 50),
    window(Judgement::Great, 83, 83),
    window(Judgement::Good, 114, 114),
    window(Judgement::Bad, 132, 132),
    window(Judgement::Miss, 137, 137),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 50, 50),
    window(Judgement::Great, 83, 83),
    window(Judgement::Good, 114, 114),
    window(Judgement::Bad, 132, 132),
    window(Judgement::Miss, 137, 137),
    window(Judgement::Perfect, 84, 66),
    window(Judgement::Great, 0, 166),
    window(Judgement::Good, 0, 202),
    window(Judgement::Bad, 0, 220),
    window(Judgement::Miss, 0, 222),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 93, 67),
    window(Judgement::Great, 0, 83),
    window(Judgement::Good, 0, 124),
    window(Judgement::Bad, 0, 135),
    window(Judgement::Miss, 0, 140),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 93, 67),
    window(Judgement::Great, 0, 83),
    window(Judgement::Good, 0, 124),
    window(Judgement::Bad, 0, 135),
    window(Judgement::Miss, 0, 140),
    window(Judgement::Perfect, 77, 77),
    window(Judgement::Miss, 0, 137),
    window(Judgement::Perfect, 77, 77),
    window(Judgement::Miss, 0, 137),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 77, 77),
    window(Judgement::Miss, 58, 137),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 77, 77),
    window(Judgement::Miss, 58, 137),
    // Assist level 3.
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 50, 50),
    window(Judgement::Great, 83, 83),
    window(Judgement::Good, 118, 118),
    window(Judgement::Bad, 137, 137),
    window(Judgement::Miss, 143, 143),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 50, 50),
    window(Judgement::Great, 83, 83),
    window(Judgement::Good, 118, 118),
    window(Judgement::Bad, 137, 137),
    window(Judgement::Miss, 143, 143),
    window(Judgement::Perfect, 84, 66),
    window(Judgement::Great, 0, 166),
    window(Judgement::Good, 0, 210),
    window(Judgement::Bad, 0, 229),
    window(Judgement::Miss, 0, 231),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 98, 67),
    window(Judgement::Great, 0, 83),
    window(Judgement::Good, 0, 128),
    window(Judgement::Bad, 0, 140),
    window(Judgement::Miss, 0, 146),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 98, 58),
    window(Judgement::Great, 0, 83),
    window(Judgement::Good, 0, 128),
    window(Judgement::Bad, 0, 140),
    window(Judgement::Miss, 0, 146),
    window(Judgement::Perfect, 82, 82),
    window(Judgement::Miss, 0, 143),
    window(Judgement::Perfect, 82, 82),
    window(Judgement::Miss, 0, 143),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 82, 82),
    window(Judgement::Miss, 58, 143),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 82, 82),
    window(Judgement::Miss, 58, 143),
    // Assist level 4.
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 50, 50),
    window(Judgement::Great, 83, 83),
    window(Judgement::Good, 124, 124),
    window(Judgement::Bad, 143, 143),
    window(Judgement::Miss, 149, 149),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 50, 50),
    window(Judgement::Great, 83, 83),
    window(Judgement::Good, 124, 124),
    window(Judgement::Bad, 143, 143),
    window(Judgement::Miss, 149, 149),
    window(Judgement::Perfect, 84, 66),
    window(Judgement::Great, 0, 166),
    window(Judgement::Good, 0, 219),
    window(Judgement::Bad, 0, 239),
    window(Judgement::Miss, 0, 241),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 103, 67),
    window(Judgement::Great, 0, 83),
    window(Judgement::Good, 0, 133),
    window(Judgement::Bad, 0, 146),
    window(Judgement::Miss, 0, 152),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 103, 67),
    window(Judgement::Great, 0, 83),
    window(Judgement::Good, 0, 133),
    window(Judgement::Bad, 0, 146),
    window(Judgement::Miss, 0, 152),
    window(Judgement::Perfect, 87, 87),
    window(Judgement::Miss, 0, 149),
    window(Judgement::Perfect, 87, 87),
    window(Judgement::Miss, 0, 149),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 87, 87),
    window(Judgement::Miss, 58, 149),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 87, 87),
    window(Judgement::Miss, 58, 149),
    // Assist level 5.
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 50, 50),
    window(Judgement::Great, 83, 83),
    window(Judgement::Good, 129, 129),
    window(Judgement::Bad, 150, 150),
    window(Judgement::Miss, 156, 156),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 50, 50),
    window(Judgement::Great, 83, 83),
    window(Judgement::Good, 129, 129),
    window(Judgement::Bad, 150, 150),
    window(Judgement::Miss, 156, 156),
    window(Judgement::Perfect, 84, 66),
    window(Judgement::Great, 0, 166),
    window(Judgement::Good, 0, 229),
    window(Judgement::Bad, 0, 249),
    window(Judgement::Miss, 0, 251),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 108, 67),
    window(Judgement::Great, 0, 83),
    window(Judgement::Good, 0, 139),
    window(Judgement::Bad, 0, 153),
    window(Judgement::Miss, 0, 159),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 108, 67),
    window(Judgement::Great, 0, 83),
    window(Judgement::Good, 0, 139),
    window(Judgement::Bad, 0, 153),
    window(Judgement::Miss, 0, 159),
    window(Judgement::Perfect, 92, 92),
    window(Judgement::Miss, 0, 156),
    window(Judgement::Perfect, 92, 92),
    window(Judgement::Miss, 0, 156),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 92, 92),
    window(Judgement::Miss, 58, 156),
    window(Judgement::Just, 2, 2),
    window(Judgement::Perfect, 92, 92),
    window(Judgement::Miss, 58, 156),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    const NOTE_TYPES: [NoteJudgementType; 9] = [
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

    #[test]
    fn every_assist_level_has_39_unique_timing_rows() {
        for assist in AssistLevel::ALL {
            let mut count = 0;
            let mut keys = BTreeSet::new();
            for note_type in NOTE_TYPES {
                let windows = timing_windows(assist, note_type);
                count += windows.len();
                for window in windows {
                    assert!(keys.insert((note_type as i16, window.judgement as i8)));
                }
            }
            assert_eq!(count, TIMING_WINDOWS_PER_LEVEL);
        }
        assert_eq!(
            TIMING_WINDOWS.len(),
            ASSIST_LEVEL_COUNT * TIMING_WINDOWS_PER_LEVEL
        );
    }

    #[test]
    fn assist_level_numeric_serde_is_stable_and_rejects_unknown_values() {
        for level in AssistLevel::ALL {
            let code = level as u8;
            assert_eq!(serde_json::to_value(level).unwrap(), code);
            assert_eq!(
                serde_json::from_value::<AssistLevel>(code.into()).unwrap(),
                level
            );
        }
        assert!(serde_json::from_value::<AssistLevel>(6.into()).is_err());
    }
}
