use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

use crate::assist::AssistLevel;
use crate::judgement::{
    JudgeResult, JudgeTiming, Judgement, NoteJudgementType, is_within_window_with_assist,
    judge_with_assist, maximum_early_ms_with_assist, maximum_late_ms_with_assist,
};
use crate::runtime::{
    LanePosition, NoteDirection, RuntimeChartError, RuntimeChartV1, RuntimeLineKind, RuntimeLineV1,
    RuntimeNoteV1, is_target_lane_with_assist, notes_overlap,
};
use crate::scoring::{
    ComboAction, LIFE_BASE, NoteOperateType, ScoreError, ScoreUnits, combo_action, contribution,
    life_damage, normalize_score, perfect_ceiling,
};
use crate::timing::TimeMicros;

const REWIND_RESET_TOLERANCE_MICROS: i64 = 5_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SessionMode {
    Chart,
    #[default]
    Watch,
    Play,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GameplayNote {
    pub id: String,
    pub time: TimeMicros,
    pub judgement_type: NoteJudgementType,
    pub operate_type: NoteOperateType,
}

/// Quantized pointer movement retained with flick input for deterministic replay.
///
/// The host owns the unit and target selection. No implicit `f64` to `f32`
/// conversion occurs in this crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputVector {
    pub delta_x: i32,
    pub delta_y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    deny_unknown_fields,
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum InputAction {
    Tap,
    Release,
    Flick { movement: InputVector },
    Trace,
    Cancel,
}

/// Compatibility input carrying a host-resolved gameplay target.
///
/// Browser hosts quantize their millisecond clock to signed integer
/// microseconds exactly once before constructing this value. New integrations
/// use [`RuntimeInputEvent`]; this form remains available for differential
/// traces and staged host migration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InputEvent {
    pub sequence: u64,
    pub time: TimeMicros,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_note_id: Option<String>,
    pub action: InputAction,
}

/// Primary unresolved input consumed by the Rust candidate selector.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RuntimeInputEvent {
    pub sequence: u64,
    pub time: TimeMicros,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer_id: Option<u32>,
    pub lane: LanePosition,
    pub action: InputAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct JudgementEvent {
    pub note_id: String,
    pub judgement: Judgement,
    pub timing: JudgeTiming,
    pub difference: TimeMicros,
    pub judged_at: TimeMicros,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_sequence: Option<u64>,
    pub combo: u32,
    pub max_combo: u32,
    pub score: u32,
    pub score_delta: u32,
    pub life: u32,
}

/// Copy-only state for low-allocation render and foreign-function boundaries.
///
/// Hosts receive judgement events separately, so this form deliberately omits
/// the variable-width last-event payload retained by [`SessionSnapshot`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedSessionSnapshot {
    pub mode: SessionMode,
    pub assist_level: AssistLevel,
    pub time: TimeMicros,
    pub judgement_offset: TimeMicros,
    pub combo: u32,
    pub perfect_combo: bool,
    pub all_perfect: bool,
    pub full_combo: bool,
    pub max_combo: u32,
    pub score: u32,
    pub life: u32,
    pub processed: u32,
    pub total: u32,
    pub last_input_sequence: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub mode: SessionMode,
    #[serde(default)]
    pub assist_level: AssistLevel,
    pub time: TimeMicros,
    pub judgement_offset: TimeMicros,
    pub combo: u32,
    /// Compatibility HUD state: active combo and an unbroken All-Perfect run.
    pub perfect_combo: bool,
    /// All-Perfect state for the entire run, including before the first note.
    pub all_perfect: bool,
    /// Full-Combo state for the entire run; only Bad or Miss clears it.
    pub full_combo: bool,
    pub max_combo: u32,
    pub score: u32,
    pub life: u32,
    pub processed: u32,
    pub total: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_judgement: Option<JudgementEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_input_sequence: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionError {
    EmptyNoteId(usize),
    DuplicateNoteId(String),
    UnsortedNotes(usize),
    UnknownNoteId(String),
    NonMonotonicInput { previous: u64, received: u64 },
    TimeOverflow,
    CounterOverflow,
    RuntimeUnavailable,
    Runtime(RuntimeChartError),
    Score(ScoreError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyNoteId(index) => write!(formatter, "gameplay note {index} has an empty ID"),
            Self::DuplicateNoteId(id) => write!(formatter, "duplicate gameplay note ID: {id}"),
            Self::UnsortedNotes(index) => {
                write!(formatter, "gameplay note {index} is out of time order")
            }
            Self::UnknownNoteId(id) => write!(formatter, "unknown gameplay note ID: {id}"),
            Self::NonMonotonicInput { previous, received } => write!(
                formatter,
                "input sequence {received} does not follow sequence {previous}"
            ),
            Self::TimeOverflow => formatter.write_str("session time calculation overflowed"),
            Self::CounterOverflow => formatter.write_str("session counter overflowed"),
            Self::RuntimeUnavailable => {
                formatter.write_str("this session has no RuntimeChartV1 candidate state")
            }
            Self::Runtime(error) => error.fmt(formatter),
            Self::Score(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<ScoreError> for SessionError {
    fn from(error: ScoreError) -> Self {
        Self::Score(error)
    }
}

impl From<RuntimeChartError> for SessionError {
    fn from(error: RuntimeChartError) -> Self {
        Self::Runtime(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PointerToken(u64);

impl PointerToken {
    const DEFAULT: Self = Self(u64::MAX);

    const fn from_id(pointer_id: Option<u32>) -> Self {
        match pointer_id {
            Some(id) => Self(id as u64),
            None => Self::DEFAULT,
        }
    }
}

#[derive(Clone, Debug)]
struct RuntimeNoteState {
    note: RuntimeNoteV1,
    line_indices: Vec<usize>,
    start_line_indices: Vec<usize>,
    end_line_indices: Vec<usize>,
}

#[derive(Clone, Debug)]
struct RuntimeLineState {
    id: u32,
    member_note_indices: Vec<usize>,
    start_note_index: Option<usize>,
}

#[derive(Clone, Debug)]
struct RuntimeState {
    notes: Vec<RuntimeNoteState>,
    lines: Vec<RuntimeLineState>,
    line_owners: Vec<Option<PointerToken>>,
    pointer_bindings: Vec<(PointerToken, usize)>,
    maximum_early_micros: i64,
    maximum_late_micros: i64,
}

impl RuntimeState {
    fn new(
        playable_notes: Vec<RuntimeNoteV1>,
        source_lines: &[RuntimeLineV1],
        assist_level: AssistLevel,
    ) -> Self {
        let note_index: BTreeMap<_, _> = playable_notes
            .iter()
            .enumerate()
            .map(|(index, note)| (note.id, index))
            .collect();
        let mut notes: Vec<_> = playable_notes
            .into_iter()
            .map(|note| RuntimeNoteState {
                note,
                line_indices: Vec::new(),
                start_line_indices: Vec::new(),
                end_line_indices: Vec::new(),
            })
            .collect();
        let mut lines = Vec::new();
        for source in source_lines
            .iter()
            .filter(|line| line.kind == RuntimeLineKind::Long)
        {
            let line_index = lines.len();
            let member_note_indices: Vec<_> = source
                .note_ids
                .iter()
                .filter_map(|note_id| note_index.get(note_id).copied())
                .collect();
            for note_index in &member_note_indices {
                notes[*note_index].line_indices.push(line_index);
            }
            let start_note_index = source
                .note_ids
                .first()
                .and_then(|note_id| note_index.get(note_id).copied());
            if let Some(note_index) = start_note_index {
                notes[note_index].start_line_indices.push(line_index);
            }
            if let Some(note_index) = source
                .note_ids
                .last()
                .and_then(|note_id| note_index.get(note_id).copied())
            {
                notes[note_index].end_line_indices.push(line_index);
            }
            lines.push(RuntimeLineState {
                id: source.id,
                member_note_indices,
                start_note_index,
            });
        }
        let maximum_early_micros = notes
            .iter()
            .map(|note| {
                i64::from(maximum_early_ms_with_assist(
                    assist_level,
                    note.note.judgement_type,
                )) * 1_000
            })
            .max()
            .unwrap_or(0);
        let maximum_late_micros = notes
            .iter()
            .map(|note| {
                i64::from(maximum_late_ms_with_assist(
                    assist_level,
                    note.note.judgement_type,
                )) * 1_000
            })
            .max()
            .unwrap_or(0);
        let line_count = lines.len();
        Self {
            notes,
            lines,
            line_owners: vec![None; line_count],
            pointer_bindings: Vec::with_capacity(line_count),
            maximum_early_micros,
            maximum_late_micros,
        }
    }

    fn clear_ownership(&mut self) {
        self.line_owners.fill(None);
        self.pointer_bindings.clear();
    }

    fn pointer_line_index(&self, pointer: PointerToken) -> Option<usize> {
        self.pointer_bindings
            .iter()
            .find_map(|(candidate, line_index)| (*candidate == pointer).then_some(*line_index))
    }

    fn is_available_to_pointer(&self, note_index: usize, pointer: PointerToken) -> bool {
        let note = &self.notes[note_index];
        if note.line_indices.is_empty() {
            return true;
        }
        if !note.start_line_indices.is_empty() {
            return self.available_start_line(note_index, pointer).is_some();
        }
        let Some(line_index) = self.pointer_line_index(pointer) else {
            return false;
        };
        note.line_indices.contains(&line_index) && self.line_owners[line_index] == Some(pointer)
    }

    fn available_start_line(&self, note_index: usize, pointer: PointerToken) -> Option<usize> {
        let starts = &self.notes[note_index].start_line_indices;
        let current_line = self.pointer_line_index(pointer);
        if let Some(line_index) = current_line
            && starts.contains(&line_index)
            && self.line_owners[line_index] == Some(pointer)
        {
            return Some(line_index);
        }
        if current_line.is_some() {
            return None;
        }
        starts.iter().copied().find(|line_index| {
            self.line_owners[*line_index].is_none()
                || self.line_owners[*line_index] == Some(pointer)
        })
    }

    fn bind_pointer(&mut self, pointer: PointerToken, line_index: usize) {
        self.unbind_pointer(pointer);
        self.unbind_line(line_index);
        self.line_owners[line_index] = Some(pointer);
        self.pointer_bindings.push((pointer, line_index));
    }

    fn unbind_pointer(&mut self, pointer: PointerToken) {
        let Some(binding_index) = self
            .pointer_bindings
            .iter()
            .position(|(candidate, _)| *candidate == pointer)
        else {
            return;
        };
        let (_, line_index) = self.pointer_bindings.swap_remove(binding_index);
        if self.line_owners[line_index] == Some(pointer) {
            self.line_owners[line_index] = None;
        }
    }

    fn unbind_line(&mut self, line_index: usize) {
        let Some(pointer) = self.line_owners[line_index].take() else {
            return;
        };
        if let Some(binding_index) =
            self.pointer_bindings
                .iter()
                .position(|(candidate, candidate_line)| {
                    *candidate == pointer && *candidate_line == line_index
                })
        {
            self.pointer_bindings.swap_remove(binding_index);
        }
    }

    fn unbind_ending_lines(&mut self, note_index: usize) {
        for index in 0..self.notes[note_index].end_line_indices.len() {
            let line_index = self.notes[note_index].end_line_indices[index];
            self.unbind_line(line_index);
        }
    }

    fn line_is_owned(&self, line_id: u32) -> bool {
        self.lines
            .iter()
            .position(|line| line.id == line_id)
            .is_some_and(|index| self.line_owners[index].is_some())
    }

    fn pointer_line_id(&self, pointer: PointerToken) -> Option<u32> {
        self.pointer_line_index(pointer)
            .map(|line_index| self.lines[line_index].id)
    }
}

/// Deterministic judgement, combo, life, and score state.
///
/// Target selection is deliberately injected through [`InputEvent`]. This
/// keeps the kernel independent of renderer coordinates while preserving the
/// exact ordering and state transitions of a resolved input stream.
#[derive(Clone, Debug)]
pub struct GameplaySession {
    notes: Vec<GameplayNote>,
    note_index: BTreeMap<String, usize>,
    processed: Vec<bool>,
    pending_last_timing: Vec<bool>,
    processed_count: u32,
    total: u32,
    update_cursor: usize,
    ceiling: ScoreUnits,
    contribution_sum: ScoreUnits,
    mode: SessionMode,
    assist_level: AssistLevel,
    judgement_offset: TimeMicros,
    time: TimeMicros,
    combo: u32,
    all_perfect: bool,
    full_combo: bool,
    max_combo: u32,
    score: u32,
    life: u32,
    last_judgement: Option<JudgementEvent>,
    last_input_sequence: Option<u64>,
    runtime: Option<RuntimeState>,
}

impl GameplaySession {
    pub fn new(
        notes: Vec<GameplayNote>,
        mode: SessionMode,
        judgement_offset: TimeMicros,
    ) -> Result<Self, SessionError> {
        Self::new_with_assist(notes, mode, judgement_offset, AssistLevel::Level0)
    }

    pub fn new_with_assist(
        notes: Vec<GameplayNote>,
        mode: SessionMode,
        judgement_offset: TimeMicros,
        assist_level: AssistLevel,
    ) -> Result<Self, SessionError> {
        let total = u32::try_from(notes.len()).map_err(|_| SessionError::CounterOverflow)?;
        let mut note_index = BTreeMap::new();
        for (index, note) in notes.iter().enumerate() {
            if note.id.is_empty() {
                return Err(SessionError::EmptyNoteId(index));
            }
            if index > 0 && note.time < notes[index - 1].time {
                return Err(SessionError::UnsortedNotes(index));
            }
            if note_index.insert(note.id.clone(), index).is_some() {
                return Err(SessionError::DuplicateNoteId(note.id.clone()));
            }
        }
        let ceiling = perfect_ceiling(notes.iter().map(|note| note.operate_type))?;
        let processed = vec![false; notes.len()];
        let pending_last_timing = vec![false; notes.len()];
        Ok(Self {
            notes,
            note_index,
            processed,
            pending_last_timing,
            processed_count: 0,
            total,
            update_cursor: 0,
            ceiling,
            contribution_sum: ScoreUnits::ZERO,
            mode,
            assist_level,
            judgement_offset,
            time: TimeMicros(0),
            combo: 0,
            all_perfect: true,
            full_combo: true,
            max_combo: 0,
            score: 0,
            life: LIFE_BASE,
            last_judgement: None,
            last_input_sequence: None,
            runtime: None,
        })
    }

    pub fn from_runtime_chart(
        chart: RuntimeChartV1,
        mode: SessionMode,
        judgement_offset: TimeMicros,
    ) -> Result<Self, SessionError> {
        chart.validate()?;
        let assist_level = chart.assist_level;
        let playable_notes: Vec<_> = chart
            .notes
            .iter()
            .filter(|note| note.judged)
            .cloned()
            .collect();
        let notes = playable_notes
            .iter()
            .map(|note| GameplayNote {
                id: note.id.to_string(),
                time: note.time,
                judgement_type: note.judgement_type,
                operate_type: note.operate_type,
            })
            .collect();
        let runtime = RuntimeState::new(playable_notes, &chart.lines, assist_level);
        let mut session = Self::new_with_assist(notes, mode, judgement_offset, assist_level)?;
        session.runtime = Some(runtime);
        Ok(session)
    }

    pub fn mode(&self) -> SessionMode {
        self.mode
    }

    pub fn assist_level(&self) -> AssistLevel {
        self.assist_level
    }

    pub fn set_mode(&mut self, mode: SessionMode) -> Result<SessionSnapshot, SessionError> {
        if self.mode == mode {
            return Ok(self.snapshot());
        }
        self.mode = mode;
        self.reset(self.time)
    }

    pub fn judgement_offset(&self) -> TimeMicros {
        self.judgement_offset
    }

    pub fn set_judgement_offset(&mut self, offset: TimeMicros) {
        self.judgement_offset = offset;
    }

    pub fn is_processed(&self, note_id: &str) -> bool {
        self.note_index
            .get(note_id)
            .is_some_and(|index| self.processed[*index])
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        let fixed = self.fixed_snapshot();
        SessionSnapshot {
            mode: fixed.mode,
            assist_level: fixed.assist_level,
            time: fixed.time,
            judgement_offset: fixed.judgement_offset,
            combo: fixed.combo,
            perfect_combo: fixed.perfect_combo,
            all_perfect: fixed.all_perfect,
            full_combo: fixed.full_combo,
            max_combo: fixed.max_combo,
            score: fixed.score,
            life: fixed.life,
            processed: fixed.processed,
            total: fixed.total,
            last_judgement: self.last_judgement.clone(),
            last_input_sequence: fixed.last_input_sequence,
        }
    }

    pub fn fixed_snapshot(&self) -> FixedSessionSnapshot {
        FixedSessionSnapshot {
            mode: self.mode,
            assist_level: self.assist_level,
            time: self.time,
            judgement_offset: self.judgement_offset,
            combo: self.combo,
            perfect_combo: self.combo > 0 && self.all_perfect,
            all_perfect: self.all_perfect,
            full_combo: self.full_combo,
            max_combo: self.max_combo,
            score: self.score,
            life: self.life,
            processed: self.processed_count,
            total: self.total,
            last_input_sequence: self.last_input_sequence,
        }
    }

    /// Selects and consumes a candidate entirely inside the Rust kernel.
    pub fn consume_runtime_input(
        &mut self,
        input: &RuntimeInputEvent,
    ) -> Result<Option<JudgementEvent>, SessionError> {
        if self.mode != SessionMode::Play {
            return Ok(None);
        }
        self.validate_sequence(input.sequence)?;
        if self.runtime.is_none() {
            return Err(SessionError::RuntimeUnavailable);
        }
        let pointer = PointerToken::from_id(input.pointer_id);
        if input.action == InputAction::Cancel {
            self.runtime
                .as_mut()
                .expect("runtime presence was checked")
                .unbind_pointer(pointer);
            self.last_input_sequence = Some(input.sequence);
            return Ok(None);
        }

        let adjusted_time = input
            .time
            .0
            .checked_add(self.judgement_offset.0)
            .map(TimeMicros)
            .ok_or(SessionError::TimeOverflow)?;
        let selection =
            self.select_runtime_candidate(input.lane, adjusted_time, pointer, Some(input.action))?;
        let Some(index) = selection else {
            if input.action == InputAction::Release {
                self.runtime
                    .as_mut()
                    .expect("runtime presence was checked")
                    .unbind_pointer(pointer);
            }
            self.last_input_sequence = Some(input.sequence);
            return Ok(None);
        };

        let (difference, result, start_line, line_end) = {
            let runtime = self.runtime.as_ref().expect("runtime presence was checked");
            let note = &runtime.notes[index].note;
            let difference = adjusted_time
                .0
                .checked_sub(note.time.0)
                .map(TimeMicros)
                .ok_or(SessionError::TimeOverflow)?;
            (
                difference,
                judge_with_assist(self.assist_level, note.judgement_type, difference),
                runtime.available_start_line(index, pointer),
                is_line_end(note.operate_type),
            )
        };
        let event =
            self.apply_judgement(index, result, difference, input.time, Some(input.sequence))?;
        let runtime = self.runtime.as_mut().expect("runtime presence was checked");
        if let Some(line_index) = start_line
            && result.judgement != Judgement::Miss
        {
            runtime.bind_pointer(pointer, line_index);
        }
        if line_end {
            runtime.unbind_ending_lines(index);
        }
        if input.action == InputAction::Release {
            runtime.unbind_pointer(pointer);
        }
        self.last_input_sequence = Some(input.sequence);
        Ok(Some(event))
    }

    /// Read-only candidate lookup used before gesture classification.
    pub fn has_runtime_input_candidate(
        &self,
        lane: LanePosition,
        input_time: TimeMicros,
        pointer_id: Option<u32>,
    ) -> Result<bool, SessionError> {
        if self.mode != SessionMode::Play {
            return Ok(false);
        }
        let adjusted_time = input_time
            .0
            .checked_add(self.judgement_offset.0)
            .map(TimeMicros)
            .ok_or(SessionError::TimeOverflow)?;
        Ok(self
            .select_runtime_candidate(lane, adjusted_time, PointerToken::from_id(pointer_id), None)?
            .is_some())
    }

    pub fn runtime_pointer_line(&self, pointer_id: Option<u32>) -> Option<u32> {
        self.runtime
            .as_ref()
            .and_then(|runtime| runtime.pointer_line_id(PointerToken::from_id(pointer_id)))
    }

    pub fn runtime_line_is_owned(&self, line_id: u32) -> bool {
        self.runtime
            .as_ref()
            .is_some_and(|runtime| runtime.line_is_owned(line_id))
    }

    fn select_runtime_candidate(
        &self,
        lane: LanePosition,
        adjusted_time: TimeMicros,
        pointer: PointerToken,
        action: Option<InputAction>,
    ) -> Result<Option<usize>, SessionError> {
        let runtime = self
            .runtime
            .as_ref()
            .ok_or(SessionError::RuntimeUnavailable)?;
        let earliest_candidate = adjusted_time
            .0
            .checked_sub(runtime.maximum_late_micros)
            .ok_or(SessionError::TimeOverflow)?;
        let mut low = 0;
        let mut high = runtime.notes.len();
        while low < high {
            let middle = (low + high) / 2;
            if runtime.notes[middle].note.time.0 < earliest_candidate {
                low = middle + 1;
            } else {
                high = middle;
            }
        }

        let mut end = runtime.notes.len();
        let mut candidate = None;
        let mut candidate_distance = i64::MAX;
        for index in low..runtime.notes.len() {
            let note = &runtime.notes[index].note;
            let difference = adjusted_time
                .0
                .checked_sub(note.time.0)
                .ok_or(SessionError::TimeOverflow)?;
            if difference < -runtime.maximum_early_micros {
                end = index;
                break;
            }
            let Some(distance) =
                self.runtime_candidate_distance(runtime, index, difference, lane, pointer, action)
            else {
                continue;
            };
            // Equal distance keeps the first item in stable chart traversal.
            if distance < candidate_distance {
                candidate = Some(index);
                candidate_distance = distance;
            }
        }

        let Some(fallback) = candidate else {
            return Ok(None);
        };
        let Some(InputAction::Flick { movement }) = action else {
            return Ok(Some(fallback));
        };
        for index in low..end {
            let note = &runtime.notes[index].note;
            let difference = adjusted_time
                .0
                .checked_sub(note.time.0)
                .ok_or(SessionError::TimeOverflow)?;
            if self.runtime_candidate_distance(runtime, index, difference, lane, pointer, action)
                != Some(candidate_distance)
            {
                continue;
            }
            if note.time != runtime.notes[fallback].note.time
                || !notes_overlap(note, &runtime.notes[fallback].note)
                || !is_target_direction_flick(note.direction, movement)
            {
                continue;
            }
            return Ok(Some(index));
        }
        Ok(Some(fallback))
    }

    fn runtime_candidate_distance(
        &self,
        runtime: &RuntimeState,
        index: usize,
        difference: i64,
        lane: LanePosition,
        pointer: PointerToken,
        action: Option<InputAction>,
    ) -> Option<i64> {
        let note = &runtime.notes[index].note;
        if self.processed[index]
            || self.pending_last_timing[index]
            || action.is_some_and(|action| !accepts_runtime_input(note, action))
            || !is_within_window_with_assist(
                self.assist_level,
                note.judgement_type,
                TimeMicros(difference),
            )
            || !is_target_lane_with_assist(self.assist_level, note, lane)
            || !runtime.is_available_to_pointer(index, pointer)
        {
            return None;
        }
        Some(difference.abs())
    }

    /// Compatibility path for a host-resolved target. A valid but non-matching input returns
    /// `Ok(None)` and still advances the replay sequence.
    pub fn apply_input(
        &mut self,
        input: &InputEvent,
    ) -> Result<Option<JudgementEvent>, SessionError> {
        if self.mode != SessionMode::Play {
            return Ok(None);
        }
        self.validate_sequence(input.sequence)?;
        if input.action == InputAction::Cancel {
            self.last_input_sequence = Some(input.sequence);
            return Ok(None);
        }
        let Some(note_id) = input.target_note_id.as_deref() else {
            self.last_input_sequence = Some(input.sequence);
            return Ok(None);
        };
        let index = *self
            .note_index
            .get(note_id)
            .ok_or_else(|| SessionError::UnknownNoteId(note_id.into()))?;
        if self.processed[index]
            || self.pending_last_timing[index]
            || !accepts_input(&self.notes[index], input.action)
        {
            self.last_input_sequence = Some(input.sequence);
            return Ok(None);
        }
        let adjusted_time = input
            .time
            .0
            .checked_add(self.judgement_offset.0)
            .map(TimeMicros)
            .ok_or(SessionError::TimeOverflow)?;
        let difference = adjusted_time
            .0
            .checked_sub(self.notes[index].time.0)
            .map(TimeMicros)
            .ok_or(SessionError::TimeOverflow)?;
        if !is_within_window_with_assist(
            self.assist_level,
            self.notes[index].judgement_type,
            difference,
        ) {
            self.last_input_sequence = Some(input.sequence);
            return Ok(None);
        }
        let result = judge_with_assist(
            self.assist_level,
            self.notes[index].judgement_type,
            difference,
        );
        let event =
            self.apply_judgement(index, result, difference, input.time, Some(input.sequence))?;
        self.last_input_sequence = Some(input.sequence);
        Ok(Some(event))
    }

    /// Advances automatic watch judgements or play-mode late misses.
    pub fn advance(&mut self, time: TimeMicros) -> Result<Vec<JudgementEvent>, SessionError> {
        if time.0 < self.time.0.saturating_sub(REWIND_RESET_TOLERANCE_MICROS) {
            self.reset(time)?;
            return Ok(Vec::new());
        }
        self.time = time;
        self.advance_update_cursor();
        let mut events = Vec::new();
        match self.mode {
            SessionMode::Watch => {
                while self.update_cursor < self.notes.len() {
                    let index = self.update_cursor;
                    if self.notes[index].time >= self.time {
                        break;
                    }
                    if self.processed[index] {
                        self.update_cursor += 1;
                        continue;
                    }
                    events.push(self.apply_judgement(
                        index,
                        JudgeResult {
                            judgement: Judgement::Perfect,
                            timing: JudgeTiming::Auto,
                        },
                        TimeMicros(0),
                        self.time,
                        None,
                    )?);
                }
            }
            SessionMode::Play => {
                for index in 0..self.notes.len() {
                    if !self.pending_last_timing[index] {
                        continue;
                    }
                    self.pending_last_timing[index] = false;
                    if self.processed[index] {
                        continue;
                    }
                    let event = self.apply_judgement(
                        index,
                        JudgeResult {
                            judgement: Judgement::Miss,
                            timing: JudgeTiming::LastTiming,
                        },
                        TimeMicros(2_147_483_647_000),
                        self.time,
                        None,
                    )?;
                    if let Some(runtime) = &mut self.runtime {
                        runtime.unbind_ending_lines(index);
                    }
                    events.push(event);
                }
                let adjusted_time = self
                    .time
                    .0
                    .checked_add(self.judgement_offset.0)
                    .map(TimeMicros)
                    .ok_or(SessionError::TimeOverflow)?;
                for index in self.update_cursor..self.notes.len() {
                    if self.notes[index].time >= adjusted_time {
                        break;
                    }
                    if self.processed[index] || self.pending_last_timing[index] {
                        continue;
                    }
                    let late = i64::from(maximum_late_ms_with_assist(
                        self.assist_level,
                        self.notes[index].judgement_type,
                    )) * 1_000;
                    if i128::from(self.notes[index].time.0) + i128::from(late)
                        >= i128::from(adjusted_time.0)
                    {
                        continue;
                    }
                    self.pending_last_timing[index] = true;
                }
            }
            SessionMode::Chart => {}
        }
        Ok(events)
    }

    /// Settles exact-tail and post-music note states without skipping the
    /// two-stage transition used by natural terminal judgements.
    pub fn finish(&mut self, time: TimeMicros) -> Result<Vec<JudgementEvent>, SessionError> {
        let terminal_time = time.0.saturating_add(1_000);
        let last_note_time = self.notes.last().map_or(terminal_time, |note| note.time.0);
        let settle_time = self
            .time
            .0
            .max(terminal_time)
            .max(last_note_time.saturating_add(2_000_000));
        let mut events = self.advance(TimeMicros(settle_time))?;
        if self.pending_last_timing.iter().any(|pending| *pending) {
            events.extend(self.advance(TimeMicros(settle_time))?);
        }
        Ok(events)
    }

    /// Clears gameplay state and reconstructs deterministic seek state without
    /// emitting historical input events.
    pub fn reset(&mut self, time: TimeMicros) -> Result<SessionSnapshot, SessionError> {
        self.processed.fill(false);
        self.pending_last_timing.fill(false);
        self.processed_count = 0;
        self.update_cursor = 0;
        self.contribution_sum = ScoreUnits::ZERO;
        self.combo = 0;
        self.all_perfect = true;
        self.full_combo = true;
        self.max_combo = 0;
        self.score = 0;
        self.life = LIFE_BASE;
        self.last_judgement = None;
        self.last_input_sequence = None;
        self.time = time;
        if let Some(runtime) = &mut self.runtime {
            runtime.clear_ownership();
        }

        if time.0 > 0 {
            match self.mode {
                SessionMode::Watch => {
                    while self.update_cursor < self.notes.len()
                        && self.notes[self.update_cursor].time < time
                    {
                        let index = self.update_cursor;
                        self.apply_judgement(
                            index,
                            JudgeResult {
                                judgement: Judgement::Perfect,
                                timing: JudgeTiming::Auto,
                            },
                            TimeMicros(0),
                            time,
                            None,
                        )?;
                    }
                }
                SessionMode::Play => {
                    let adjusted_time = time
                        .0
                        .checked_add(self.judgement_offset.0)
                        .ok_or(SessionError::TimeOverflow)?;
                    for index in 0..self.notes.len() {
                        let late = i64::from(maximum_late_ms_with_assist(
                            self.assist_level,
                            self.notes[index].judgement_type,
                        )) * 1_000;
                        if i128::from(self.notes[index].time.0) + i128::from(late)
                            >= i128::from(adjusted_time)
                        {
                            break;
                        }
                        self.processed[index] = true;
                        self.processed_count = self
                            .processed_count
                            .checked_add(1)
                            .ok_or(SessionError::CounterOverflow)?;
                    }
                    if let Some(runtime) = &self.runtime {
                        for line in &runtime.lines {
                            if line
                                .start_note_index
                                .is_some_and(|index| self.processed[index])
                            {
                                for note_index in &line.member_note_indices {
                                    self.processed[*note_index] = true;
                                }
                            }
                        }
                        self.processed_count = u32::try_from(
                            self.processed
                                .iter()
                                .filter(|processed| **processed)
                                .count(),
                        )
                        .map_err(|_| SessionError::CounterOverflow)?;
                    }
                    self.advance_update_cursor();
                }
                SessionMode::Chart => {}
            }
        }
        Ok(self.snapshot())
    }

    fn validate_sequence(&self, sequence: u64) -> Result<(), SessionError> {
        if let Some(previous) = self.last_input_sequence
            && sequence <= previous
        {
            return Err(SessionError::NonMonotonicInput {
                previous,
                received: sequence,
            });
        }
        Ok(())
    }

    fn apply_judgement(
        &mut self,
        index: usize,
        result: JudgeResult,
        difference: TimeMicros,
        judged_at: TimeMicros,
        input_sequence: Option<u64>,
    ) -> Result<JudgementEvent, SessionError> {
        let (combo, all_perfect, full_combo) = match combo_action(result.judgement) {
            ComboAction::Ignore => (self.combo, self.all_perfect, self.full_combo),
            ComboAction::Increment => {
                let combo = self
                    .combo
                    .checked_add(1)
                    .ok_or(SessionError::CounterOverflow)?;
                let all_perfect = self.all_perfect
                    && matches!(result.judgement, Judgement::Perfect | Judgement::Just);
                (combo, all_perfect, self.full_combo)
            }
            ComboAction::Break => (0, false, false),
        };
        let max_combo = self.max_combo.max(combo);
        let life = self.life.saturating_sub(life_damage(result.judgement));
        let contribution_sum = self.contribution_sum.checked_add(contribution(
            result.judgement,
            self.notes[index].operate_type,
            combo,
        ))?;
        let score = normalize_score(contribution_sum, self.ceiling)?;
        let event = JudgementEvent {
            note_id: self.notes[index].id.clone(),
            judgement: result.judgement,
            timing: result.timing,
            difference,
            judged_at,
            input_sequence,
            combo,
            max_combo,
            score,
            score_delta: score.saturating_sub(self.score),
            life,
        };

        self.pending_last_timing[index] = false;
        self.processed[index] = true;
        self.processed_count = self
            .processed_count
            .checked_add(1)
            .ok_or(SessionError::CounterOverflow)?;
        self.combo = combo;
        self.all_perfect = all_perfect;
        self.full_combo = full_combo;
        self.max_combo = max_combo;
        self.life = life;
        self.contribution_sum = contribution_sum;
        self.score = score;
        self.last_judgement = Some(event.clone());
        self.advance_update_cursor();
        Ok(event)
    }

    fn advance_update_cursor(&mut self) {
        while self.update_cursor < self.notes.len() && self.processed[self.update_cursor] {
            self.update_cursor += 1;
        }
    }
}

fn accepts_input(note: &GameplayNote, action: InputAction) -> bool {
    match action {
        InputAction::Cancel => false,
        InputAction::Flick { .. } => is_flick(note.operate_type),
        InputAction::Release => note.operate_type == NoteOperateType::SlideEnd,
        InputAction::Trace => matches!(
            note.judgement_type,
            NoteJudgementType::Trace | NoteJudgementType::SlideEndTrace
        ),
        InputAction::Tap => {
            !is_flick(note.operate_type)
                && note.operate_type != NoteOperateType::SlideEnd
                && !matches!(
                    note.judgement_type,
                    NoteJudgementType::Trace | NoteJudgementType::SlideEndTrace
                )
        }
    }
}

fn accepts_runtime_input(note: &RuntimeNoteV1, action: InputAction) -> bool {
    match action {
        InputAction::Cancel => false,
        InputAction::Flick { .. } => is_flick(note.operate_type),
        InputAction::Release => note.operate_type == NoteOperateType::SlideEnd,
        InputAction::Trace => matches!(
            note.judgement_type,
            NoteJudgementType::Trace | NoteJudgementType::SlideEndTrace
        ),
        InputAction::Tap => {
            !is_flick(note.operate_type)
                && note.operate_type != NoteOperateType::SlideEnd
                && !matches!(
                    note.judgement_type,
                    NoteJudgementType::Trace | NoteJudgementType::SlideEndTrace
                )
        }
    }
}

const fn is_line_end(note_type: NoteOperateType) -> bool {
    matches!(
        note_type,
        NoteOperateType::SlideEnd | NoteOperateType::SlideEndFlick | NoteOperateType::SlideEndTrace
    )
}

/// The current compatibility profile uses 180 degrees and compares the unit
/// dot product against `atan(pi)`, which is greater than one. Directional
/// candidates therefore cannot satisfy it; Normal remains unconditional.
const fn is_target_direction_flick(direction: NoteDirection, _movement: InputVector) -> bool {
    matches!(direction, NoteDirection::Normal)
}

const fn is_flick(note_type: NoteOperateType) -> bool {
    matches!(
        note_type,
        NoteOperateType::Flick
            | NoteOperateType::SlideBeginFlick
            | NoteOperateType::SlideEndFlick
            | NoteOperateType::GuideBeginFlick
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{
        JudgementAreaOffsetType, LANE_UNITS_PER_LANE, RUNTIME_CHART_FORMAT, RUNTIME_CHART_VERSION,
        RUNTIME_LANE_COUNT,
    };

    fn note(
        id: &str,
        time_micros: i64,
        judgement_type: NoteJudgementType,
        operate_type: NoteOperateType,
    ) -> GameplayNote {
        GameplayNote {
            id: id.into(),
            time: TimeMicros(time_micros),
            judgement_type,
            operate_type,
        }
    }

    fn input(sequence: u64, note_id: &str, time_micros: i64) -> InputEvent {
        InputEvent {
            sequence,
            time: TimeMicros(time_micros),
            pointer_id: Some(0),
            target_note_id: Some(note_id.into()),
            action: InputAction::Tap,
        }
    }

    fn runtime_note(
        id: u32,
        time_micros: i64,
        position: i32,
        size: i32,
        judgement_type: NoteJudgementType,
        operate_type: NoteOperateType,
    ) -> RuntimeNoteV1 {
        RuntimeNoteV1 {
            id,
            time: TimeMicros(time_micros),
            position: LanePosition(position * LANE_UNITS_PER_LANE),
            size: LanePosition(size * LANE_UNITS_PER_LANE),
            operate_type,
            judgement_type,
            judgement_area_offset_type: JudgementAreaOffsetType::Default,
            direction: NoteDirection::Normal,
            judged: true,
        }
    }

    fn runtime_chart(notes: Vec<RuntimeNoteV1>, lines: Vec<RuntimeLineV1>) -> RuntimeChartV1 {
        RuntimeChartV1 {
            format: RUNTIME_CHART_FORMAT.into(),
            version: RUNTIME_CHART_VERSION,
            lane_count: RUNTIME_LANE_COUNT,
            assist_level: AssistLevel::Level0,
            notes,
            lines,
        }
    }

    fn runtime_input(
        sequence: u64,
        time_micros: i64,
        lane_micros: i32,
        pointer_id: Option<u32>,
        action: InputAction,
    ) -> RuntimeInputEvent {
        RuntimeInputEvent {
            sequence,
            time: TimeMicros(time_micros),
            pointer_id,
            lane: LanePosition(lane_micros),
            action,
        }
    }

    #[test]
    fn rejects_duplicate_and_unsorted_notes() {
        let duplicate = vec![
            note("n", 0, NoteJudgementType::Normal, NoteOperateType::Normal),
            note("n", 1, NoteJudgementType::Normal, NoteOperateType::Normal),
        ];
        assert!(matches!(
            GameplaySession::new(duplicate, SessionMode::Play, TimeMicros(0)),
            Err(SessionError::DuplicateNoteId(_))
        ));

        let unsorted = vec![
            note(
                "late",
                1,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
            note(
                "early",
                0,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
        ];
        assert!(matches!(
            GameplaySession::new(unsorted, SessionMode::Play, TimeMicros(0)),
            Err(SessionError::UnsortedNotes(1))
        ));
    }

    #[test]
    fn input_updates_combo_score_life_and_run_flags_in_order() {
        let notes = vec![
            note(
                "perfect",
                1_000_000,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
            note(
                "great",
                2_000_000,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
            note(
                "good",
                3_000_000,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
            note(
                "bad",
                4_000_000,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
            note(
                "miss",
                5_000_000,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
        ];
        let mut session = GameplaySession::new(notes, SessionMode::Play, TimeMicros(0)).unwrap();
        let initial = session.snapshot();
        assert!(!initial.perfect_combo);
        assert!(initial.all_perfect);
        assert!(initial.full_combo);

        let perfect = session
            .apply_input(&input(1, "perfect", 1_002_000))
            .unwrap()
            .unwrap();
        assert_eq!(perfect.judgement, Judgement::Perfect);
        assert_eq!(perfect.combo, 1);
        assert!(session.snapshot().perfect_combo);
        assert!(session.snapshot().all_perfect);
        assert!(session.snapshot().full_combo);

        let great = session
            .apply_input(&input(2, "great", 2_043_000))
            .unwrap()
            .unwrap();
        assert_eq!(great.judgement, Judgement::Great);
        assert_eq!(great.combo, 2);
        assert!(!session.snapshot().perfect_combo);
        assert!(!session.snapshot().all_perfect);
        assert!(session.snapshot().full_combo);

        let good = session
            .apply_input(&input(3, "good", 3_084_000))
            .unwrap()
            .unwrap();
        assert_eq!(good.judgement, Judgement::Good);
        assert_eq!(good.combo, 3);
        assert!(!session.snapshot().perfect_combo);
        assert!(session.snapshot().full_combo);

        let bad = session
            .apply_input(&input(4, "bad", 4_109_000))
            .unwrap()
            .unwrap();
        assert_eq!(bad.judgement, Judgement::Bad);
        assert_eq!(bad.life, 950);
        assert!(!session.snapshot().full_combo);

        let miss = session
            .apply_input(&input(5, "miss", 5_126_000))
            .unwrap()
            .unwrap();
        assert_eq!(miss.judgement, Judgement::Miss);
        assert_eq!(miss.life, 850);
        assert_eq!(session.snapshot().processed, 5);
        assert_eq!(session.snapshot().max_combo, 3);
        assert!(!session.snapshot().perfect_combo);
        assert!(!session.snapshot().all_perfect);
        assert!(!session.snapshot().full_combo);
    }

    #[test]
    fn combo_counter_ignores_wait_and_pass_and_run_flags_never_recover() {
        let judgements = [
            Judgement::Perfect,
            Judgement::Wait,
            Judgement::Pass,
            Judgement::Good,
            Judgement::Bad,
            Judgement::Perfect,
        ];
        let notes = judgements
            .iter()
            .enumerate()
            .map(|(index, _)| {
                note(
                    &format!("n{index}"),
                    index as i64,
                    NoteJudgementType::Normal,
                    NoteOperateType::Normal,
                )
            })
            .collect();
        let mut session = GameplaySession::new(notes, SessionMode::Play, TimeMicros(0)).unwrap();

        for (index, judgement) in judgements.into_iter().enumerate() {
            session
                .apply_judgement(
                    index,
                    JudgeResult {
                        judgement,
                        timing: JudgeTiming::Auto,
                    },
                    TimeMicros(0),
                    TimeMicros(index as i64),
                    None,
                )
                .unwrap();
            let snapshot = session.snapshot();
            match index {
                0..=2 => {
                    assert_eq!(snapshot.combo, 1);
                    assert_eq!(snapshot.max_combo, 1);
                    assert!(snapshot.perfect_combo);
                    assert!(snapshot.all_perfect);
                    assert!(snapshot.full_combo);
                }
                3 => {
                    assert_eq!(snapshot.combo, 2);
                    assert_eq!(snapshot.max_combo, 2);
                    assert!(!snapshot.perfect_combo);
                    assert!(!snapshot.all_perfect);
                    assert!(snapshot.full_combo);
                }
                4 => {
                    assert_eq!(snapshot.combo, 0);
                    assert_eq!(snapshot.max_combo, 2);
                    assert!(!snapshot.perfect_combo);
                    assert!(!snapshot.all_perfect);
                    assert!(!snapshot.full_combo);
                }
                5 => {
                    assert_eq!(snapshot.combo, 1);
                    assert_eq!(snapshot.max_combo, 2);
                    assert!(!snapshot.perfect_combo);
                    assert!(!snapshot.all_perfect);
                    assert!(!snapshot.full_combo);
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn play_terminal_miss_uses_two_updates_after_the_inclusive_late_boundary() {
        let notes = vec![note(
            "n",
            0,
            NoteJudgementType::Normal,
            NoteOperateType::Normal,
        )];
        let mut session = GameplaySession::new(notes, SessionMode::Play, TimeMicros(0)).unwrap();
        assert!(session.advance(TimeMicros(130_000)).unwrap().is_empty());
        assert!(session.advance(TimeMicros(130_001)).unwrap().is_empty());
        assert_eq!(session.snapshot().processed, 0);
        let events = session.advance(TimeMicros(130_001)).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].judgement, Judgement::Miss);
        assert_eq!(events[0].timing, JudgeTiming::LastTiming);
        assert_eq!(events[0].difference, TimeMicros(2_147_483_647_000));
        assert_eq!(events[0].life, 900);
    }

    #[test]
    fn finish_includes_exact_tail_and_settles_playing_last() {
        let notes = vec![note(
            "tail",
            1_000_000,
            NoteJudgementType::Normal,
            NoteOperateType::Normal,
        )];
        let mut watch =
            GameplaySession::new(notes.clone(), SessionMode::Watch, TimeMicros(0)).unwrap();
        assert!(watch.advance(TimeMicros(1_000_000)).unwrap().is_empty());
        let events = watch.finish(TimeMicros(1_000_000)).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].judgement, Judgement::Perfect);
        assert_eq!(events[0].timing, JudgeTiming::Auto);

        let mut play = GameplaySession::new(notes, SessionMode::Play, TimeMicros(0)).unwrap();
        assert!(play.advance(TimeMicros(1_000_000)).unwrap().is_empty());
        let events = play.finish(TimeMicros(1_000_000)).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].judgement, Judgement::Miss);
        assert_eq!(events[0].timing, JudgeTiming::LastTiming);
        assert_eq!(play.snapshot().processed, 1);
    }

    #[test]
    fn watch_cursor_is_strict_and_uses_automatic_perfect() {
        let notes = vec![note(
            "n",
            1_000,
            NoteJudgementType::Normal,
            NoteOperateType::Normal,
        )];
        let mut session = GameplaySession::new(notes, SessionMode::Watch, TimeMicros(0)).unwrap();
        assert!(session.advance(TimeMicros(1_000)).unwrap().is_empty());
        assert_eq!(session.snapshot().processed, 0);
        let events = session.advance(TimeMicros(1_001)).unwrap();
        assert_eq!(events[0].judgement, Judgement::Perfect);
        assert_eq!(events[0].timing, JudgeTiming::Auto);
        assert_eq!(events[0].judged_at, TimeMicros(1_001));
    }

    #[test]
    fn watch_seek_rebuild_uses_the_same_strict_time_boundary() {
        let notes = vec![note(
            "n",
            1_000,
            NoteJudgementType::Normal,
            NoteOperateType::Normal,
        )];
        let mut session = GameplaySession::new(notes, SessionMode::Watch, TimeMicros(0)).unwrap();
        let exact = session.reset(TimeMicros(1_000)).unwrap();
        assert_eq!(exact.processed, 0);
        assert!(exact.last_judgement.is_none());

        let after = session.reset(TimeMicros(1_001)).unwrap();
        assert_eq!(after.processed, 1);
        assert_eq!(
            after.last_judgement.as_ref().map(|event| event.judgement),
            Some(Judgement::Perfect)
        );
    }

    #[test]
    fn play_seek_skips_expired_notes_without_gameplay_side_effects() {
        let notes = vec![
            note(
                "expired",
                0,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
            note(
                "available",
                1_000_000,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
        ];
        let mut session = GameplaySession::new(notes, SessionMode::Play, TimeMicros(0)).unwrap();
        let snapshot = session.reset(TimeMicros(500_000)).unwrap();
        assert!(session.is_processed("expired"));
        assert!(!session.is_processed("available"));
        assert_eq!(snapshot.life, LIFE_BASE);
        assert_eq!(snapshot.score, 0);
        assert_eq!(snapshot.combo, 0);
        assert!(snapshot.last_judgement.is_none());
    }

    #[test]
    fn action_routing_and_sequence_order_are_deterministic() {
        let notes = vec![note(
            "flick",
            0,
            NoteJudgementType::Flick,
            NoteOperateType::Flick,
        )];
        let mut session = GameplaySession::new(notes, SessionMode::Play, TimeMicros(0)).unwrap();
        assert!(
            session
                .apply_input(&input(1, "flick", 0))
                .unwrap()
                .is_none()
        );
        let flick = InputEvent {
            sequence: 2,
            time: TimeMicros(0),
            pointer_id: Some(7),
            target_note_id: Some("flick".into()),
            action: InputAction::Flick {
                movement: InputVector {
                    delta_x: 1_000,
                    delta_y: 0,
                },
            },
        };
        assert!(session.apply_input(&flick).unwrap().is_some());
        assert!(matches!(
            session.apply_input(&flick),
            Err(SessionError::NonMonotonicInput {
                previous: 2,
                received: 2
            })
        ));
    }

    #[test]
    fn session_assist_level_defaults_and_snapshot_serde_are_compatible() {
        let session = GameplaySession::new(Vec::new(), SessionMode::Play, TimeMicros(0)).unwrap();
        assert_eq!(session.assist_level(), AssistLevel::Level0);
        let snapshot = session.snapshot();
        assert_eq!(snapshot.assist_level, AssistLevel::Level0);

        let mut value = serde_json::to_value(&snapshot).unwrap();
        assert_eq!(value["assistLevel"], 0);
        value.as_object_mut().unwrap().remove("assistLevel");
        assert_eq!(
            serde_json::from_value::<SessionSnapshot>(value).unwrap(),
            snapshot
        );
    }

    #[test]
    fn session_assist_profile_drives_timing_misses_and_resolved_input() {
        let notes = vec![note(
            "n",
            1_000_000,
            NoteJudgementType::Normal,
            NoteOperateType::Normal,
        )];
        let mut default =
            GameplaySession::new(notes.clone(), SessionMode::Play, TimeMicros(0)).unwrap();
        assert!(
            default
                .apply_input(&input(1, "n", 1_140_000))
                .unwrap()
                .is_none()
        );

        let mut assisted = GameplaySession::new_with_assist(
            notes,
            SessionMode::Play,
            TimeMicros(0),
            AssistLevel::Level5,
        )
        .unwrap();
        let event = assisted
            .apply_input(&input(1, "n", 1_140_000))
            .unwrap()
            .unwrap();
        assert_eq!(event.judgement, Judgement::Bad);
        assert_eq!(assisted.snapshot().assist_level, AssistLevel::Level5);

        let notes = vec![note(
            "late",
            0,
            NoteJudgementType::Normal,
            NoteOperateType::Normal,
        )];
        let mut assisted = GameplaySession::new_with_assist(
            notes,
            SessionMode::Play,
            TimeMicros(0),
            AssistLevel::Level5,
        )
        .unwrap();
        assert!(assisted.advance(TimeMicros(156_000)).unwrap().is_empty());
        assert!(assisted.advance(TimeMicros(156_001)).unwrap().is_empty());
        assert_eq!(assisted.advance(TimeMicros(156_001)).unwrap().len(), 1);
    }

    #[test]
    fn runtime_candidate_uses_chart_assist_timing_and_area_profiles() {
        let source = runtime_note(
            7,
            1_000_000,
            8,
            4,
            NoteJudgementType::Normal,
            NoteOperateType::Normal,
        );

        let mut timing_chart = runtime_chart(vec![source.clone()], vec![]);
        timing_chart.assist_level = AssistLevel::Level5;
        let mut timing =
            GameplaySession::from_runtime_chart(timing_chart, SessionMode::Play, TimeMicros(0))
                .unwrap();
        let event = timing
            .consume_runtime_input(&runtime_input(
                1,
                1_140_000,
                10_000_000,
                None,
                InputAction::Tap,
            ))
            .unwrap()
            .unwrap();
        assert_eq!(event.judgement, Judgement::Bad);
        assert_eq!(timing.assist_level(), AssistLevel::Level5);

        let mut area_chart = runtime_chart(vec![source.clone()], vec![]);
        area_chart.assist_level = AssistLevel::Level5;
        let mut area =
            GameplaySession::from_runtime_chart(area_chart, SessionMode::Play, TimeMicros(0))
                .unwrap();
        assert!(
            area.consume_runtime_input(&runtime_input(
                1,
                1_000_000,
                6_250_000,
                None,
                InputAction::Tap,
            ))
            .unwrap()
            .is_some()
        );

        let mut default = GameplaySession::from_runtime_chart(
            runtime_chart(vec![source], vec![]),
            SessionMode::Play,
            TimeMicros(0),
        )
        .unwrap();
        assert!(
            default
                .consume_runtime_input(&runtime_input(
                    1,
                    1_000_000,
                    6_250_000,
                    None,
                    InputAction::Tap,
                ))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn runtime_candidate_hit_test_is_inclusive_and_equal_distance_keeps_traversal_order() {
        let notes = vec![
            runtime_note(
                99,
                1_000_000,
                8,
                4,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
            runtime_note(
                1,
                1_000_000,
                8,
                4,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
        ];
        let mut session = GameplaySession::from_runtime_chart(
            runtime_chart(notes, vec![]),
            SessionMode::Play,
            TimeMicros(0),
        )
        .unwrap();
        let event = session
            .consume_runtime_input(&runtime_input(
                1,
                1_000_000,
                12_000_000,
                None,
                InputAction::Tap,
            ))
            .unwrap()
            .unwrap();
        assert_eq!(event.note_id, "99");

        let notes = vec![runtime_note(
            7,
            1_000_000,
            8,
            4,
            NoteJudgementType::Normal,
            NoteOperateType::Normal,
        )];
        let mut session = GameplaySession::from_runtime_chart(
            runtime_chart(notes, vec![]),
            SessionMode::Play,
            TimeMicros(0),
        )
        .unwrap();
        assert!(
            session
                .consume_runtime_input(&runtime_input(
                    1,
                    1_000_000,
                    12_500_001,
                    None,
                    InputAction::Tap,
                ))
                .unwrap()
                .is_none()
        );
        assert!(
            session
                .consume_runtime_input(&runtime_input(
                    2,
                    1_000_000,
                    12_500_000,
                    None,
                    InputAction::Tap,
                ))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn runtime_candidate_uses_nearest_time_from_the_sorted_window() {
        let notes = vec![
            runtime_note(
                0,
                1_000_000,
                8,
                4,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
            runtime_note(
                1,
                1_020_000,
                8,
                4,
                NoteJudgementType::Normal,
                NoteOperateType::Normal,
            ),
        ];
        let mut session = GameplaySession::from_runtime_chart(
            runtime_chart(notes, vec![]),
            SessionMode::Play,
            TimeMicros(0),
        )
        .unwrap();
        let event = session
            .consume_runtime_input(&runtime_input(
                1,
                1_015_000,
                10_000_000,
                Some(1),
                InputAction::Tap,
            ))
            .unwrap()
            .unwrap();
        assert_eq!(event.note_id, "1");
        assert_eq!(event.difference, TimeMicros(-5_000));
    }

    #[test]
    fn runtime_flick_direction_is_a_stable_tie_break_not_a_rejection() {
        let mut left = runtime_note(
            9,
            1_000_000,
            8,
            4,
            NoteJudgementType::Flick,
            NoteOperateType::Flick,
        );
        left.direction = NoteDirection::Left;
        let normal = runtime_note(
            2,
            1_000_000,
            8,
            4,
            NoteJudgementType::Flick,
            NoteOperateType::Flick,
        );
        let action = InputAction::Flick {
            movement: InputVector {
                delta_x: -1_000,
                delta_y: 0,
            },
        };
        let mut session = GameplaySession::from_runtime_chart(
            runtime_chart(vec![left.clone(), normal], vec![]),
            SessionMode::Play,
            TimeMicros(0),
        )
        .unwrap();
        let event = session
            .consume_runtime_input(&runtime_input(1, 1_000_000, 10_000_000, None, action))
            .unwrap()
            .unwrap();
        assert_eq!(event.note_id, "2");

        let mut session = GameplaySession::from_runtime_chart(
            runtime_chart(vec![left], vec![]),
            SessionMode::Play,
            TimeMicros(0),
        )
        .unwrap();
        let fallback = session
            .consume_runtime_input(&runtime_input(1, 1_000_000, 10_000_000, None, action))
            .unwrap()
            .unwrap();
        assert_eq!(fallback.note_id, "9");
    }

    #[test]
    fn runtime_long_line_requires_pointer_ownership_and_release_or_cancel_clears_it() {
        let mut start = runtime_note(
            0,
            1_000_000,
            8,
            4,
            NoteJudgementType::SlideBegin,
            NoteOperateType::SlideBegin,
        );
        start.judgement_area_offset_type = JudgementAreaOffsetType::SlideBegin;
        let mut trace_one = runtime_note(
            1,
            1_500_000,
            8,
            4,
            NoteJudgementType::Trace,
            NoteOperateType::Trace,
        );
        trace_one.judgement_area_offset_type = JudgementAreaOffsetType::Trace;
        let mut trace_two = runtime_note(
            2,
            1_800_000,
            8,
            4,
            NoteJudgementType::Trace,
            NoteOperateType::Trace,
        );
        trace_two.judgement_area_offset_type = JudgementAreaOffsetType::Trace;
        let mut end = runtime_note(
            3,
            2_000_000,
            8,
            4,
            NoteJudgementType::SlideEnd,
            NoteOperateType::SlideEnd,
        );
        end.judgement_area_offset_type = JudgementAreaOffsetType::SlideEnd;
        let chart = runtime_chart(
            vec![start, trace_one, trace_two, end],
            vec![RuntimeLineV1 {
                id: 7,
                kind: RuntimeLineKind::Long,
                note_ids: vec![0, 1, 2, 3],
            }],
        );
        let mut session =
            GameplaySession::from_runtime_chart(chart.clone(), SessionMode::Play, TimeMicros(0))
                .unwrap();

        assert!(
            session
                .consume_runtime_input(&runtime_input(
                    1,
                    1_500_000,
                    10_000_000,
                    Some(11),
                    InputAction::Trace,
                ))
                .unwrap()
                .is_none()
        );
        assert!(
            session
                .consume_runtime_input(&runtime_input(
                    2,
                    1_000_000,
                    10_000_000,
                    Some(11),
                    InputAction::Tap,
                ))
                .unwrap()
                .is_some()
        );
        assert_eq!(session.runtime_pointer_line(Some(11)), Some(7));
        assert!(session.runtime_line_is_owned(7));
        assert!(
            session
                .consume_runtime_input(&runtime_input(
                    3,
                    1_500_000,
                    10_000_000,
                    Some(12),
                    InputAction::Trace,
                ))
                .unwrap()
                .is_none()
        );
        assert!(
            session
                .consume_runtime_input(&runtime_input(
                    4,
                    1_500_000,
                    10_000_000,
                    Some(11),
                    InputAction::Trace,
                ))
                .unwrap()
                .is_some()
        );
        assert!(
            session
                .consume_runtime_input(&runtime_input(
                    5,
                    1_600_000,
                    10_000_000,
                    Some(11),
                    InputAction::Release,
                ))
                .unwrap()
                .is_none()
        );
        assert_eq!(session.runtime_pointer_line(Some(11)), None);
        assert!(
            session
                .consume_runtime_input(&runtime_input(
                    6,
                    1_800_000,
                    10_000_000,
                    Some(11),
                    InputAction::Trace,
                ))
                .unwrap()
                .is_none()
        );

        session.reset(TimeMicros(0)).unwrap();
        for event in [
            runtime_input(1, 1_000_000, 10_000_000, Some(11), InputAction::Tap),
            runtime_input(2, 1_500_000, 10_000_000, Some(11), InputAction::Trace),
            runtime_input(3, 1_800_000, 10_000_000, Some(11), InputAction::Trace),
            runtime_input(4, 2_000_000, 10_000_000, Some(11), InputAction::Release),
        ] {
            assert!(session.consume_runtime_input(&event).unwrap().is_some());
        }
        assert!(!session.runtime_line_is_owned(7));

        session.reset(TimeMicros(0)).unwrap();
        assert!(
            session
                .consume_runtime_input(&runtime_input(
                    1,
                    1_000_000,
                    10_000_000,
                    Some(11),
                    InputAction::Tap,
                ))
                .unwrap()
                .is_some()
        );
        assert!(
            session
                .consume_runtime_input(&runtime_input(
                    2,
                    1_100_000,
                    0,
                    Some(11),
                    InputAction::Cancel,
                ))
                .unwrap()
                .is_none()
        );
        assert!(!session.runtime_line_is_owned(7));
        assert!(
            !session
                .has_runtime_input_candidate(
                    LanePosition(10_000_000),
                    TimeMicros(1_500_000),
                    Some(11),
                )
                .unwrap()
        );

        let mut seek =
            GameplaySession::from_runtime_chart(chart, SessionMode::Play, TimeMicros(0)).unwrap();
        let snapshot = seek.reset(TimeMicros(1_700_000)).unwrap();
        assert_eq!(snapshot.processed, 4);
        assert_eq!(snapshot.score, 0);
        assert_eq!(snapshot.life, LIFE_BASE);
    }

    #[test]
    fn runtime_input_serde_has_no_host_resolved_target() {
        let event = runtime_input(4, 1_001, 12_000_000, Some(2), InputAction::Trace);
        let value = serde_json::to_value(&event).unwrap();
        assert!(value.get("targetNoteId").is_none());
        assert_eq!(value["lane"], 12_000_000);
        assert_eq!(
            serde_json::from_value::<RuntimeInputEvent>(value).unwrap(),
            event
        );
    }

    #[test]
    fn input_serde_contract_is_explicit_and_camel_case() {
        let event = InputEvent {
            sequence: 9,
            time: TimeMicros(42_000),
            pointer_id: Some(3),
            target_note_id: Some("n1".into()),
            action: InputAction::Flick {
                movement: InputVector {
                    delta_x: -10,
                    delta_y: 20,
                },
            },
        };
        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(value["targetNoteId"], "n1");
        assert_eq!(value["action"]["type"], "flick");
        assert_eq!(value["action"]["movement"]["deltaX"], -10);
        assert_eq!(serde_json::from_value::<InputEvent>(value).unwrap(), event);
    }
}
