use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Judgement {
    Miss,
    Bad,
    Good,
    Great,
    Perfect,
    Just,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JudgeTiming {
    Fast,
    Exact,
    Late,
    OutOfTime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JudgementWindow {
    pub judgement: Judgement,
    pub early_ms: i32,
    pub late_ms: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JudgeResult {
    pub judgement: Judgement,
    pub timing: JudgeTiming,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JudgementProfile {
    windows: Vec<JudgementWindow>,
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

    pub fn our_notes_normal() -> Self {
        use Judgement::{Bad, Good, Great, Just, Miss, Perfect};
        Self::new([
            symmetric(Just, 1),
            symmetric(Perfect, 42),
            symmetric(Great, 83),
            symmetric(Good, 108),
            symmetric(Bad, 125),
            symmetric(Miss, 130),
        ])
        .expect("the built-in profile is valid")
    }

    pub fn judge(&self, difference_ms: i32) -> JudgeResult {
        for window in &self.windows {
            if difference_ms < -window.early_ms || difference_ms > window.late_ms {
                continue;
            }
            return JudgeResult {
                judgement: window.judgement,
                timing: if difference_ms.abs() <= 1 {
                    JudgeTiming::Exact
                } else if difference_ms < 0 {
                    JudgeTiming::Fast
                } else {
                    JudgeTiming::Late
                },
            };
        }
        JudgeResult {
            judgement: Judgement::Miss,
            timing: JudgeTiming::OutOfTime,
        }
    }
}

const fn symmetric(judgement: Judgement, milliseconds: i32) -> JudgementWindow {
    JudgementWindow {
        judgement,
        early_ms: milliseconds,
        late_ms: milliseconds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn our_notes_boundaries_are_inclusive() {
        let profile = JudgementProfile::our_notes_normal();
        assert_eq!(profile.judge(-42).judgement, Judgement::Perfect);
        assert_eq!(profile.judge(42).judgement, Judgement::Perfect);
        assert_eq!(profile.judge(43).judgement, Judgement::Great);
        assert_eq!(profile.judge(131).timing, JudgeTiming::OutOfTime);
    }
}
