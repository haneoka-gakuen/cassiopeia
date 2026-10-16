//! Host-supplied automatic result data, independent from game-specific missions.
use crate::{Judgement, NoteJudgementType, TimeMicros};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AutomaticJudgementPolicy {
    #[serde(default)]
    pub rules: Vec<AutomaticJudgementRule>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AutomaticJudgementRule {
    pub start_micros: i64,
    pub end_micros: i64,
    pub judgement_types: Vec<NoteJudgementType>,
    pub judgement: Judgement,
}

impl AutomaticJudgementPolicy {
    pub fn validate(&self) -> Result<(), String> {
        for rule in &self.rules {
            if rule.start_micros >= rule.end_micros || rule.judgement_types.is_empty() {
                return Err(
                    "automatic result rule requires a nonempty interval and judgement types".into(),
                );
            }
            if matches!(rule.judgement, Judgement::None | Judgement::Wait) {
                return Err("automatic result must be a terminal judgement".into());
            }
        }
        Ok(())
    }

    /// Ordered first-match rules, evaluated at the real observation time.
    pub fn resolve(&self, kind: NoteJudgementType, observed: TimeMicros) -> Judgement {
        self.rules
            .iter()
            .find(|r| {
                r.start_micros <= observed.0
                    && observed.0 < r.end_micros
                    && r.judgement_types.contains(&kind)
            })
            .map_or(Judgement::Perfect, |r| r.judgement)
    }
}
