use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

use crate::judgement::{
    JudgeResult, JudgeTiming, Judgement, NoteJudgementType, is_within_window, judge,
    maximum_late_ms,
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

/// Host-resolved gameplay input.
///
/// Browser hosts quantize their millisecond clock to signed integer
/// microseconds exactly once before constructing this value. Candidate geometry
/// remains a host concern until its floating-point compatibility profile is
/// represented explicitly; `target_note_id` therefore names the selected note.
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SessionSnapshot {
    pub mode: SessionMode,
    pub time: TimeMicros,
    pub judgement_offset: TimeMicros,
    pub combo: u32,
    /// All-Perfect state for the entire run; it never recovers after loss.
    pub perfect_combo: bool,
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
    processed_count: u32,
    total: u32,
    update_cursor: usize,
    ceiling: ScoreUnits,
    contribution_sum: ScoreUnits,
    mode: SessionMode,
    judgement_offset: TimeMicros,
    time: TimeMicros,
    combo: u32,
    perfect_combo: bool,
    full_combo: bool,
    max_combo: u32,
    score: u32,
    life: u32,
    last_judgement: Option<JudgementEvent>,
    last_input_sequence: Option<u64>,
}

impl GameplaySession {
    pub fn new(
        notes: Vec<GameplayNote>,
        mode: SessionMode,
        judgement_offset: TimeMicros,
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
        Ok(Self {
            notes,
            note_index,
            processed,
            processed_count: 0,
            total,
            update_cursor: 0,
            ceiling,
            contribution_sum: ScoreUnits::ZERO,
            mode,
            judgement_offset,
            time: TimeMicros(0),
            combo: 0,
            perfect_combo: true,
            full_combo: true,
            max_combo: 0,
            score: 0,
            life: LIFE_BASE,
            last_judgement: None,
            last_input_sequence: None,
        })
    }

    pub fn mode(&self) -> SessionMode {
        self.mode
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
        SessionSnapshot {
            mode: self.mode,
            time: self.time,
            judgement_offset: self.judgement_offset,
            combo: self.combo,
            perfect_combo: self.combo > 0 && self.perfect_combo,
            full_combo: self.full_combo,
            max_combo: self.max_combo,
            score: self.score,
            life: self.life,
            processed: self.processed_count,
            total: self.total,
            last_judgement: self.last_judgement.clone(),
            last_input_sequence: self.last_input_sequence,
        }
    }

    /// Applies a host-resolved input. A valid but non-matching input returns
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
        if self.processed[index] || !accepts_input(&self.notes[index], input.action) {
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
        if !is_within_window(self.notes[index].judgement_type, difference) {
            self.last_input_sequence = Some(input.sequence);
            return Ok(None);
        }
        let result = judge(self.notes[index].judgement_type, difference);
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
                let adjusted_time = self
                    .time
                    .0
                    .checked_add(self.judgement_offset.0)
                    .map(TimeMicros)
                    .ok_or(SessionError::TimeOverflow)?;
                while self.update_cursor < self.notes.len() {
                    let index = self.update_cursor;
                    if self.processed[index] {
                        self.update_cursor += 1;
                        continue;
                    }
                    let late = i64::from(maximum_late_ms(self.notes[index].judgement_type)) * 1_000;
                    if i128::from(self.notes[index].time.0) + i128::from(late)
                        >= i128::from(adjusted_time.0)
                    {
                        break;
                    }
                    let difference = adjusted_time
                        .0
                        .checked_sub(self.notes[index].time.0)
                        .map(TimeMicros)
                        .ok_or(SessionError::TimeOverflow)?;
                    events.push(self.apply_judgement(
                        index,
                        JudgeResult {
                            judgement: Judgement::Miss,
                            timing: JudgeTiming::OutOfTime,
                        },
                        difference,
                        self.time,
                        None,
                    )?);
                }
            }
            SessionMode::Chart => {}
        }
        Ok(events)
    }

    /// Clears gameplay state and reconstructs deterministic seek state without
    /// emitting historical input events.
    pub fn reset(&mut self, time: TimeMicros) -> Result<SessionSnapshot, SessionError> {
        self.processed.fill(false);
        self.processed_count = 0;
        self.update_cursor = 0;
        self.contribution_sum = ScoreUnits::ZERO;
        self.combo = 0;
        self.perfect_combo = true;
        self.full_combo = true;
        self.max_combo = 0;
        self.score = 0;
        self.life = LIFE_BASE;
        self.last_judgement = None;
        self.last_input_sequence = None;
        self.time = time;

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
                        let late =
                            i64::from(maximum_late_ms(self.notes[index].judgement_type)) * 1_000;
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
        let (combo, perfect_combo, full_combo) = match combo_action(result.judgement) {
            ComboAction::Ignore => (self.combo, self.perfect_combo, self.full_combo),
            ComboAction::Increment => {
                let combo = self
                    .combo
                    .checked_add(1)
                    .ok_or(SessionError::CounterOverflow)?;
                let perfect_combo = self.perfect_combo
                    && matches!(result.judgement, Judgement::Perfect | Judgement::Just);
                (combo, perfect_combo, self.full_combo)
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

        self.processed[index] = true;
        self.processed_count = self
            .processed_count
            .checked_add(1)
            .ok_or(SessionError::CounterOverflow)?;
        self.combo = combo;
        self.perfect_combo = perfect_combo;
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

        let perfect = session
            .apply_input(&input(1, "perfect", 1_002_000))
            .unwrap()
            .unwrap();
        assert_eq!(perfect.judgement, Judgement::Perfect);
        assert_eq!(perfect.combo, 1);
        assert!(session.snapshot().perfect_combo);
        assert!(session.snapshot().full_combo);

        let great = session
            .apply_input(&input(2, "great", 2_043_000))
            .unwrap()
            .unwrap();
        assert_eq!(great.judgement, Judgement::Great);
        assert_eq!(great.combo, 2);
        assert!(!session.snapshot().perfect_combo);
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
                    assert!(snapshot.full_combo);
                }
                3 => {
                    assert_eq!(snapshot.combo, 2);
                    assert_eq!(snapshot.max_combo, 2);
                    assert!(!snapshot.perfect_combo);
                    assert!(snapshot.full_combo);
                }
                4 => {
                    assert_eq!(snapshot.combo, 0);
                    assert_eq!(snapshot.max_combo, 2);
                    assert!(!snapshot.perfect_combo);
                    assert!(!snapshot.full_combo);
                }
                5 => {
                    assert_eq!(snapshot.combo, 1);
                    assert_eq!(snapshot.max_combo, 2);
                    assert!(!snapshot.perfect_combo);
                    assert!(!snapshot.full_combo);
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn play_miss_occurs_only_after_the_inclusive_late_boundary() {
        let notes = vec![note(
            "n",
            0,
            NoteJudgementType::Normal,
            NoteOperateType::Normal,
        )];
        let mut session = GameplaySession::new(notes, SessionMode::Play, TimeMicros(0)).unwrap();
        assert!(session.advance(TimeMicros(130_000)).unwrap().is_empty());
        let events = session.advance(TimeMicros(130_001)).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].judgement, Judgement::Miss);
        assert_eq!(events[0].timing, JudgeTiming::OutOfTime);
        assert_eq!(events[0].life, 900);
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
