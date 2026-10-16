//! Coordinate-free automatic-session loading shared by native and WASM hosts.
use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{
    AssistLevel, GameplayNote, GameplaySession, ScoringComboPolicy, SessionMode, TimeMicros,
    automatic::AutomaticJudgementPolicy,
};

/// Initialization data for automatic Watch/Chart playback. Source identifiers
/// remain canonical decimal `u32` values throughout fixed-width observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct WatchSessionLoad {
    pub notes: Vec<GameplayNote>,
    pub mode: SessionMode,
    #[serde(default)]
    pub assist_level: AssistLevel,
    #[serde(default)]
    pub offset_micros: i64,
    #[serde(default)]
    pub automatic_policy: AutomaticJudgementPolicy,
    #[serde(default)]
    pub scoring_combo_policy: ScoringComboPolicy,
}

impl WatchSessionLoad {
    pub fn into_session(self) -> Result<(GameplaySession, BTreeMap<String, u32>), String> {
        if self.mode == SessionMode::Play {
            return Err("loadNotes supports Watch and Chart; Play requires RuntimeChartV1".into());
        }
        let ids = self
            .notes
            .iter()
            .map(|note| {
                let id = note.id.parse::<u32>().map_err(|_| {
                    format!("note id must be an unsigned source identifier: {}", note.id)
                })?;
                if id.to_string() != note.id {
                    return Err(format!(
                        "note id must use canonical decimal form: {}",
                        note.id
                    ));
                }
                Ok((note.id.clone(), id))
            })
            .collect::<Result<_, _>>()?;
        let mut session = GameplaySession::new_with_assist(
            self.notes,
            self.mode,
            TimeMicros(self.offset_micros),
            self.assist_level,
        )
        .map_err(|e| e.to_string())?;
        session.set_automatic_policy(self.automatic_policy)?;
        session
            .set_scoring_combo_policy(self.scoring_combo_policy)
            .map_err(|e| e.to_string())?;
        Ok((session, ids))
    }
}
