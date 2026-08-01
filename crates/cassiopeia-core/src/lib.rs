#![forbid(unsafe_code)]

//! Deterministic contracts shared by Cassiopeia's web and native hosts.
//!
//! This crate intentionally owns no window, GPU, audio-device, filesystem, or
//! network APIs. Hosts inject those capabilities and use the same timing,
//! judgement primitives, and chart contracts on every platform.

pub mod chart;
pub mod judgement;
pub mod scoring;
pub mod session;
pub mod timing;

pub use chart::{CassiopeiaChart, ChartEvent, ChartHeader, ChartNote, NoteKind};
pub use judgement::{
    JudgeResult, JudgeTiming, Judgement, JudgementProfile, JudgementWindow, NoteJudgementType,
    is_within_window, judge, maximum_early_ms, maximum_late_ms,
};
pub use scoring::{
    ComboAction, LIFE_BASE, LIFE_DANGER, MAX_NORMALIZED_SCORE, NoteOperateType, ScoreError,
    ScoreUnits, breaks_combo, combo_action, combo_bonus_basis_points, contribution,
    contribution_scale, increments_combo, judgement_factor_milli, life_damage, normalize_score,
    note_weight_milli, perfect_ceiling, preserves_combo,
};
pub use session::{
    GameplayNote, GameplaySession, InputAction, InputEvent, InputVector, JudgementEvent,
    SessionError, SessionMode, SessionSnapshot,
};
pub use timing::{RoundingProfile, TempoEvent, TempoMap, TempoMapError, Tick, TimeMicros};
