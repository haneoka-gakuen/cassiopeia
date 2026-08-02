#![forbid(unsafe_code)]

//! Deterministic contracts shared by Cassiopeia's web and native hosts.
//!
//! This crate intentionally owns no window, GPU, audio-device, filesystem, or
//! network APIs. Hosts inject those capabilities and use the same timing,
//! judgement primitives, and chart contracts on every platform.

pub mod assist;
pub mod camera;
pub mod chart;
pub mod judgement;
pub mod live;
pub mod runtime;
pub mod scoring;
pub mod session;
pub mod timing;

pub use assist::{ASSIST_LEVEL_COUNT, AssistLevel, TIMING_WINDOWS_PER_LEVEL};
pub use camera::{
    CameraClip, CameraCurve, CameraCurveKey, CameraEvaluation, CameraEvaluationError, CameraState,
    CameraTimelineProfile, CameraTimelineResource, evaluate_camera_curve,
};
pub use chart::{CassiopeiaChart, ChartEvent, ChartHeader, ChartNote, NoteKind};
pub use judgement::{
    JudgeResult, JudgeTiming, Judgement, JudgementProfile, JudgementWindow, NoteJudgementType,
    is_within_window, is_within_window_with_assist, judge, judge_with_assist, judgement_windows,
    maximum_early_ms, maximum_early_ms_with_assist, maximum_late_ms, maximum_late_ms_with_assist,
};
pub use live::{
    CameraProfile, DEFAULT_SCREEN_MODE, LivePerformanceError, PENLIGHT_COLUMNS,
    PENLIGHT_INSTANCE_COUNT, PENLIGHT_ROWS, PLAYBACK_RATE_SCALE, PenlightColorSlot,
    PenlightInstance, PenlightScheduler, PerformanceClock, PerformanceSnapshot, PlaybackRate,
    PlaybackState, ScreenFallback, ScreenMode, ScreenResources, ScreenSelection,
    resolve_screen_mode,
};
pub use runtime::{
    AREA_OFFSETS, AREA_OFFSETS_PER_LEVEL, JudgementAreaOffsetType, LANE_UNITS_PER_LANE,
    LanePosition, NoteDirection, RUNTIME_CHART_FORMAT, RUNTIME_CHART_VERSION, RUNTIME_LANE_COUNT,
    RuntimeChartError, RuntimeChartV1, RuntimeLineKind, RuntimeLineV1, RuntimeNoteV1, area_offset,
    is_target_lane, is_target_lane_with_assist, judgement_area_offset,
    judgement_area_offset_with_assist, notes_overlap,
};
pub use scoring::{
    ComboAction, LIFE_BASE, LIFE_DANGER, MAX_NORMALIZED_SCORE, NoteOperateType, ScoreError,
    ScoreUnits, breaks_combo, combo_action, combo_bonus_basis_points, contribution,
    contribution_scale, increments_combo, judgement_factor_milli, life_damage, normalize_score,
    note_weight_milli, perfect_ceiling, preserves_combo,
};
pub use session::{
    FixedSessionSnapshot, GameplayNote, GameplaySession, InputAction, InputEvent, InputVector,
    JudgementEvent, RuntimeInputEvent, SessionError, SessionMode, SessionSnapshot,
};
pub use timing::{
    RoundingProfile, TempoEvent, TempoMap, TempoMapError, Tick, TimeMicros, TimeMillis,
};
