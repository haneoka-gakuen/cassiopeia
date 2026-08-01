#![forbid(unsafe_code)]

//! Deterministic contracts shared by Cassiopeia's web and native hosts.
//!
//! This crate intentionally owns no window, GPU, audio-device, filesystem, or
//! network APIs. Hosts inject those capabilities and use the same timing,
//! judgement, chart, and replay semantics on every platform.

pub mod chart;
pub mod judgement;
pub mod timing;

pub use chart::{CassiopeiaChart, ChartEvent, ChartHeader, ChartNote, NoteKind};
pub use judgement::{JudgeResult, JudgeTiming, Judgement, JudgementProfile, JudgementWindow};
pub use timing::{RoundingProfile, TempoEvent, TempoMap, TempoMapError, Tick, TimeMicros};
