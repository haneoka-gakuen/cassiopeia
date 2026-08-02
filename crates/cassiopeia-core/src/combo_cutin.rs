use crate::TimeMicros;
use serde::{Deserialize, Serialize};
use std::{array, fmt};

pub const COMBO_CUT_IN_INTERVAL: u32 = 100;
pub const COMBO_CUT_IN_PARTICIPANT_COUNT: usize = 2;
pub const COMBO_CUT_IN_MEMBER_COUNT: u8 = 5;
pub const COMBO_CUT_IN_DURATION: TimeMicros = TimeMicros(4_000_000);
pub const COMBO_CUT_IN_CUE_TIMES: [TimeMicros; COMBO_CUT_IN_PARTICIPANT_COUNT] =
    [TimeMicros(0), TimeMicros(1_500_000)];
pub const COMBO_CUT_IN_EVENT_CAPACITY: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComboCutInFrame {
    pub combo_updated: bool,
    pub current_combo: u32,
    pub added_combo: u32,
    pub judged_judgement_notes: u32,
    pub total_judgement_notes: u32,
}

/// Returns the first hundred-combo milestone found while scanning the combo
/// additions from newest to oldest.
///
/// This deliberately mirrors the gameplay-frame predicate: it only examines a
/// frame that changed combo, suppresses the inclusive last-N-note region when
/// configured, and produces at most one milestone even if one frame crossed
/// several hundreds.
pub const fn combo_cutin_milestone(
    frame: ComboCutInFrame,
    disable_last_n_notes: u32,
) -> Option<u32> {
    if !frame.combo_updated {
        return None;
    }

    if disable_last_n_notes >= 1 {
        let remaining = frame.total_judgement_notes as i64 - frame.judged_judgement_notes as i64;
        if remaining <= disable_last_n_notes as i64 {
            return None;
        }
    }

    if frame.added_combo < 1 {
        return None;
    }

    let mut offset = 0;
    while offset < frame.added_combo {
        let Some(combo) = frame.current_combo.checked_sub(offset) else {
            break;
        };
        if combo % COMBO_CUT_IN_INTERVAL == 0 {
            return Some(combo);
        }
        offset += 1;
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
#[serde(rename_all = "camelCase")]
pub enum ComboCutInSource {
    Fixed = 0,
    Common = 1,
}

impl TryFrom<u8> for ComboCutInSource {
    type Error = ComboCutInSourceError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Fixed),
            1 => Ok(Self::Common),
            _ => Err(ComboCutInSourceError(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComboCutInSourceError(pub u8);

impl fmt::Display for ComboCutInSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid combo cut-in source: {}", self.0)
    }
}

impl std::error::Error for ComboCutInSourceError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
#[serde(rename_all = "camelCase")]
pub enum ComboCutInRole {
    First = 0,
    Second = 1,
    Call = 2,
    Response = 3,
}

impl TryFrom<u8> for ComboCutInRole {
    type Error = ComboCutInRoleError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::First),
            1 => Ok(Self::Second),
            2 => Ok(Self::Call),
            3 => Ok(Self::Response),
            _ => Err(ComboCutInRoleError(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComboCutInRoleError(pub u8);

impl fmt::Display for ComboCutInRoleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid combo cut-in role: {}", self.0)
    }
}

impl std::error::Error for ComboCutInRoleError {}

/// A host-resolved participant. The sequencer never performs a lottery.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComboCutInActor {
    pub role: ComboCutInRole,
    pub character_id: i64,
    pub member_index: u8,
    pub voice_id: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComboCutInCast {
    source: ComboCutInSource,
    actors: [ComboCutInActor; COMBO_CUT_IN_PARTICIPANT_COUNT],
}

impl ComboCutInCast {
    pub fn new(
        source: ComboCutInSource,
        actors: [ComboCutInActor; COMBO_CUT_IN_PARTICIPANT_COUNT],
    ) -> Result<Self, ComboCutInCastError> {
        for (actor_index, actor) in actors.iter().enumerate() {
            if actor.member_index >= COMBO_CUT_IN_MEMBER_COUNT {
                return Err(ComboCutInCastError {
                    actor_index,
                    member_index: actor.member_index,
                });
            }
        }
        Ok(Self { source, actors })
    }

    pub const fn source(self) -> ComboCutInSource {
        self.source
    }

    pub const fn actors(self) -> [ComboCutInActor; COMBO_CUT_IN_PARTICIPANT_COUNT] {
        self.actors
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComboCutInCastError {
    pub actor_index: usize,
    pub member_index: u8,
}

impl fmt::Display for ComboCutInCastError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "combo cut-in actor {} has member index {}; expected 0..{}",
            self.actor_index,
            self.member_index,
            COMBO_CUT_IN_MEMBER_COUNT - 1
        )
    }
}

impl std::error::Error for ComboCutInCastError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
#[serde(rename_all = "camelCase")]
pub enum ComboCutInEndReason {
    Completed = 0,
    Cancelled = 1,
    Replaced = 2,
    Shutdown = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ComboCutInEvent {
    Started {
        sequence_id: u64,
        milestone_combo: u32,
        source: ComboCutInSource,
        actors: [ComboCutInActor; COMBO_CUT_IN_PARTICIPANT_COUNT],
        duration: TimeMicros,
    },
    CueDispatched {
        sequence_id: u64,
        milestone_combo: u32,
        cut_in_index: u8,
        actor: ComboCutInActor,
        authored_at: TimeMicros,
        dispatched_at: TimeMicros,
    },
    Ended {
        sequence_id: u64,
        milestone_combo: u32,
        reason: ComboCutInEndReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComboCutInEvents {
    entries: [Option<ComboCutInEvent>; COMBO_CUT_IN_EVENT_CAPACITY],
    len: u8,
}

impl Default for ComboCutInEvents {
    fn default() -> Self {
        Self {
            entries: array::from_fn(|_| None),
            len: 0,
        }
    }
}

impl ComboCutInEvents {
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, index: usize) -> Option<&ComboCutInEvent> {
        if index >= self.len() {
            return None;
        }
        self.entries[index].as_ref()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ComboCutInEvent> {
        self.entries[..self.len()].iter().map(|entry| {
            entry
                .as_ref()
                .expect("all combo cut-in event slots below len are initialized")
        })
    }

    fn push(&mut self, event: ComboCutInEvent) {
        let index = self.len();
        assert!(
            index < COMBO_CUT_IN_EVENT_CAPACITY,
            "combo cut-in event bound is an internal invariant"
        );
        self.entries[index] = Some(event);
        self.len += 1;
    }
}

impl<'a> IntoIterator for &'a ComboCutInEvents {
    type Item = &'a ComboCutInEvent;
    type IntoIter = ComboCutInEventIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ComboCutInEventIter {
            events: self,
            index: 0,
        }
    }
}

pub struct ComboCutInEventIter<'a> {
    events: &'a ComboCutInEvents,
    index: usize,
}

impl<'a> Iterator for ComboCutInEventIter<'a> {
    type Item = &'a ComboCutInEvent;

    fn next(&mut self) -> Option<Self::Item> {
        let event = self.events.get(self.index)?;
        self.index += 1;
        Some(event)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.events.len().saturating_sub(self.index);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ComboCutInEventIter<'_> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActiveCutIn {
    sequence_id: u64,
    milestone_combo: u32,
    cast: ComboCutInCast,
    elapsed: TimeMicros,
    second_cue_reached: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingCue {
    sequence_id: u64,
    milestone_combo: u32,
    actor: ComboCutInActor,
    authored_at: TimeMicros,
    sequence_elapsed: TimeMicros,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComboCutInSequencer {
    disable_last_n_notes: u32,
    cast: ComboCutInCast,
    enabled: bool,
    paused: bool,
    next_sequence_id: Option<u64>,
    active: Option<ActiveCutIn>,
    pending: Option<PendingCue>,
}

impl ComboCutInSequencer {
    pub const fn new(disable_last_n_notes: u32, cast: ComboCutInCast) -> Self {
        Self {
            disable_last_n_notes,
            cast,
            enabled: true,
            paused: false,
            next_sequence_id: Some(1),
            active: None,
            pending: None,
        }
    }

    pub const fn disable_last_n_notes(&self) -> u32 {
        self.disable_last_n_notes
    }

    pub fn set_disable_last_n_notes(&mut self, value: u32) {
        self.disable_last_n_notes = value;
    }

    pub const fn cast(&self) -> ComboCutInCast {
        self.cast
    }

    /// Replaces only the cast used by future sequences.
    pub fn set_cast(&mut self, cast: ComboCutInCast) {
        self.cast = cast;
    }

    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Disabling prevents new starts but deliberately leaves current playback
    /// and a delayed second cue untouched.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub const fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub const fn has_pending_cue(&self) -> bool {
        self.pending.is_some()
    }

    pub const fn active_sequence_id(&self) -> Option<u64> {
        match self.active {
            Some(active) => Some(active.sequence_id),
            None => None,
        }
    }

    pub const fn active_elapsed(&self) -> Option<TimeMicros> {
        match self.active {
            Some(active) => Some(active.elapsed),
            None => None,
        }
    }

    /// Flushes an older delayed cue before observing the new combo frame.
    /// Cue zero is dispatched immediately even when another voice is busy.
    pub fn observe_frame(
        &mut self,
        frame: ComboCutInFrame,
        voice_busy: bool,
    ) -> Result<ComboCutInEvents, ComboCutInError> {
        let mut events = ComboCutInEvents::default();
        if self.paused {
            return Ok(events);
        }

        self.flush_pending(voice_busy, &mut events);
        if !self.enabled {
            return Ok(events);
        }
        let Some(milestone_combo) = combo_cutin_milestone(frame, self.disable_last_n_notes) else {
            return Ok(events);
        };

        let sequence_id = self.allocate_sequence_id()?;
        if let Some(active) = self.active.take() {
            events.push(ComboCutInEvent::Ended {
                sequence_id: active.sequence_id,
                milestone_combo: active.milestone_combo,
                reason: ComboCutInEndReason::Replaced,
            });
        }
        self.pending = None;

        let cast = self.cast;
        self.active = Some(ActiveCutIn {
            sequence_id,
            milestone_combo,
            cast,
            elapsed: TimeMicros(0),
            second_cue_reached: false,
        });
        events.push(ComboCutInEvent::Started {
            sequence_id,
            milestone_combo,
            source: cast.source(),
            actors: cast.actors(),
            duration: COMBO_CUT_IN_DURATION,
        });
        events.push(ComboCutInEvent::CueDispatched {
            sequence_id,
            milestone_combo,
            cut_in_index: 0,
            actor: cast.actors()[0],
            authored_at: COMBO_CUT_IN_CUE_TIMES[0],
            dispatched_at: COMBO_CUT_IN_CUE_TIMES[0],
        });
        Ok(events)
    }

    /// Advances pauseable sequence time. A pending cue is checked at the start
    /// of the frame, matching the presentation update order.
    pub fn advance(
        &mut self,
        delta: TimeMicros,
        voice_busy: bool,
    ) -> Result<ComboCutInEvents, ComboCutInError> {
        if delta.0 < 0 {
            return Err(ComboCutInError::NegativeDelta(delta));
        }

        let mut events = ComboCutInEvents::default();
        if self.paused {
            return Ok(events);
        }

        let next_elapsed = if let Some(active) = self.active {
            Some(
                active
                    .elapsed
                    .0
                    .checked_add(delta.0)
                    .map(TimeMicros)
                    .ok_or(ComboCutInError::TimeOverflow)?,
            )
        } else if voice_busy {
            self.pending
                .map(|pending| {
                    pending
                        .sequence_elapsed
                        .0
                        .checked_add(delta.0)
                        .map(TimeMicros)
                        .ok_or(ComboCutInError::TimeOverflow)
                })
                .transpose()?
        } else {
            None
        };

        self.flush_pending(voice_busy, &mut events);

        let Some(mut active) = self.active else {
            if let (Some(pending), Some(elapsed)) = (self.pending.as_mut(), next_elapsed) {
                pending.sequence_elapsed = elapsed;
            }
            return Ok(events);
        };
        let elapsed = next_elapsed.expect("active cut-in always has a validated next time");
        let previous_elapsed = active.elapsed;
        active.elapsed = elapsed;

        if let Some(pending) = self.pending.as_mut() {
            pending.sequence_elapsed = elapsed;
        }

        if !active.second_cue_reached
            && previous_elapsed < COMBO_CUT_IN_CUE_TIMES[1]
            && elapsed >= COMBO_CUT_IN_CUE_TIMES[1]
        {
            active.second_cue_reached = true;
            let actor = active.cast.actors()[1];
            if voice_busy {
                self.pending = Some(PendingCue {
                    sequence_id: active.sequence_id,
                    milestone_combo: active.milestone_combo,
                    actor,
                    authored_at: COMBO_CUT_IN_CUE_TIMES[1],
                    sequence_elapsed: elapsed,
                });
            } else {
                events.push(ComboCutInEvent::CueDispatched {
                    sequence_id: active.sequence_id,
                    milestone_combo: active.milestone_combo,
                    cut_in_index: 1,
                    actor,
                    authored_at: COMBO_CUT_IN_CUE_TIMES[1],
                    dispatched_at: COMBO_CUT_IN_CUE_TIMES[1],
                });
            }
        }

        if elapsed >= COMBO_CUT_IN_DURATION {
            events.push(ComboCutInEvent::Ended {
                sequence_id: active.sequence_id,
                milestone_combo: active.milestone_combo,
                reason: ComboCutInEndReason::Completed,
            });
            self.active = None;
        } else {
            self.active = Some(active);
        }
        Ok(events)
    }

    /// Cancels active playback and removes any delayed second cue.
    pub fn cancel(&mut self) -> ComboCutInEvents {
        self.end_now(ComboCutInEndReason::Cancelled)
    }

    /// Tears down active playback and removes any delayed second cue.
    pub fn shutdown(&mut self) -> ComboCutInEvents {
        self.end_now(ComboCutInEndReason::Shutdown)
    }

    fn end_now(&mut self, reason: ComboCutInEndReason) -> ComboCutInEvents {
        let mut events = ComboCutInEvents::default();
        self.pending = None;
        if let Some(active) = self.active.take() {
            events.push(ComboCutInEvent::Ended {
                sequence_id: active.sequence_id,
                milestone_combo: active.milestone_combo,
                reason,
            });
        }
        events
    }

    fn allocate_sequence_id(&mut self) -> Result<u64, ComboCutInError> {
        let sequence_id = self
            .next_sequence_id
            .ok_or(ComboCutInError::SequenceExhausted)?;
        self.next_sequence_id = sequence_id.checked_add(1);
        Ok(sequence_id)
    }

    fn flush_pending(&mut self, voice_busy: bool, events: &mut ComboCutInEvents) {
        if voice_busy {
            return;
        }
        let Some(pending) = self.pending.take() else {
            return;
        };
        events.push(ComboCutInEvent::CueDispatched {
            sequence_id: pending.sequence_id,
            milestone_combo: pending.milestone_combo,
            cut_in_index: 1,
            actor: pending.actor,
            authored_at: pending.authored_at,
            dispatched_at: pending.sequence_elapsed,
        });
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComboCutInError {
    NegativeDelta(TimeMicros),
    TimeOverflow,
    SequenceExhausted,
}

impl fmt::Display for ComboCutInError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeDelta(delta) => {
                write!(
                    formatter,
                    "combo cut-in delta must be non-negative: {}",
                    delta.0
                )
            }
            Self::TimeOverflow => formatter.write_str("combo cut-in time overflowed"),
            Self::SequenceExhausted => {
                formatter.write_str("combo cut-in sequence IDs are exhausted")
            }
        }
    }
}

impl std::error::Error for ComboCutInError {}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST_ACTOR: ComboCutInActor = ComboCutInActor {
        role: ComboCutInRole::Call,
        character_id: 101,
        member_index: 1,
        voice_id: 1_001,
    };
    const SECOND_ACTOR: ComboCutInActor = ComboCutInActor {
        role: ComboCutInRole::Response,
        character_id: 202,
        member_index: 4,
        voice_id: 2_002,
    };

    fn cast() -> ComboCutInCast {
        ComboCutInCast::new(ComboCutInSource::Fixed, [FIRST_ACTOR, SECOND_ACTOR]).unwrap()
    }

    fn frame(current_combo: u32, added_combo: u32) -> ComboCutInFrame {
        ComboCutInFrame {
            combo_updated: true,
            current_combo,
            added_combo,
            judged_judgement_notes: 200,
            total_judgement_notes: 1_000,
        }
    }

    fn copied(events: &ComboCutInEvents) -> Vec<ComboCutInEvent> {
        events.iter().copied().collect()
    }

    fn start(sequencer: &mut ComboCutInSequencer, voice_busy: bool) -> ComboCutInEvents {
        sequencer.observe_frame(frame(100, 1), voice_busy).unwrap()
    }

    #[test]
    fn milestone_predicate_requires_an_updated_positive_combo_delta() {
        let mut input = frame(100, 1);
        input.combo_updated = false;
        assert_eq!(combo_cutin_milestone(input, 0), None);

        input.combo_updated = true;
        input.added_combo = 0;
        assert_eq!(combo_cutin_milestone(input, 0), None);

        input.added_combo = 1;
        input.current_combo = 99;
        assert_eq!(combo_cutin_milestone(input, 0), None);
    }

    #[test]
    fn milestone_scan_is_newest_first_and_returns_only_one_crossed_hundred() {
        assert_eq!(combo_cutin_milestone(frame(201, 102), 0), Some(200));
        assert_eq!(combo_cutin_milestone(frame(100, 1), 0), Some(100));
        assert_eq!(combo_cutin_milestone(frame(100, 2), 0), Some(100));
    }

    #[test]
    fn last_n_note_suppression_is_inclusive_and_only_enabled_above_zero() {
        let mut input = frame(100, 1);
        input.total_judgement_notes = 1_000;
        input.judged_judgement_notes = 995;
        assert_eq!(combo_cutin_milestone(input, 5), None);

        input.judged_judgement_notes = 994;
        assert_eq!(combo_cutin_milestone(input, 5), Some(100));

        input.judged_judgement_notes = 1_001;
        assert_eq!(combo_cutin_milestone(input, 1), None);
        assert_eq!(combo_cutin_milestone(input, 0), Some(100));
    }

    #[test]
    fn cast_rejects_member_indices_outside_the_five_member_band() {
        let invalid = ComboCutInActor {
            member_index: 5,
            ..SECOND_ACTOR
        };
        assert_eq!(
            ComboCutInCast::new(ComboCutInSource::Common, [FIRST_ACTOR, invalid]),
            Err(ComboCutInCastError {
                actor_index: 1,
                member_index: 5,
            })
        );
    }

    #[test]
    fn start_emits_stable_sequence_cast_and_immediate_first_cue_even_when_voice_is_busy() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        assert_eq!(
            copied(&start(&mut sequencer, true)),
            vec![
                ComboCutInEvent::Started {
                    sequence_id: 1,
                    milestone_combo: 100,
                    source: ComboCutInSource::Fixed,
                    actors: [FIRST_ACTOR, SECOND_ACTOR],
                    duration: COMBO_CUT_IN_DURATION,
                },
                ComboCutInEvent::CueDispatched {
                    sequence_id: 1,
                    milestone_combo: 100,
                    cut_in_index: 0,
                    actor: FIRST_ACTOR,
                    authored_at: TimeMicros(0),
                    dispatched_at: TimeMicros(0),
                },
            ]
        );
        assert_eq!(sequencer.active_sequence_id(), Some(1));
    }

    #[test]
    fn second_cue_and_natural_end_fire_on_exact_authored_boundaries() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        start(&mut sequencer, false);

        assert!(
            sequencer
                .advance(TimeMicros(1_499_999), false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            copied(&sequencer.advance(TimeMicros(1), false).unwrap()),
            vec![ComboCutInEvent::CueDispatched {
                sequence_id: 1,
                milestone_combo: 100,
                cut_in_index: 1,
                actor: SECOND_ACTOR,
                authored_at: TimeMicros(1_500_000),
                dispatched_at: TimeMicros(1_500_000),
            }]
        );
        assert!(
            sequencer
                .advance(TimeMicros(2_499_999), false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            copied(&sequencer.advance(TimeMicros(1), false).unwrap()),
            vec![ComboCutInEvent::Ended {
                sequence_id: 1,
                milestone_combo: 100,
                reason: ComboCutInEndReason::Completed,
            }]
        );
        assert!(!sequencer.is_active());
    }

    #[test]
    fn busy_voice_defers_second_motion_and_voice_as_one_cue_event() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        start(&mut sequencer, false);

        assert!(
            sequencer
                .advance(TimeMicros(1_500_000), true)
                .unwrap()
                .is_empty()
        );
        assert!(sequencer.has_pending_cue());
        assert!(
            sequencer
                .advance(TimeMicros(250_000), true)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            copied(&sequencer.advance(TimeMicros(0), false).unwrap()),
            vec![ComboCutInEvent::CueDispatched {
                sequence_id: 1,
                milestone_combo: 100,
                cut_in_index: 1,
                actor: SECOND_ACTOR,
                authored_at: TimeMicros(1_500_000),
                dispatched_at: TimeMicros(1_750_000),
            }]
        );
        assert!(!sequencer.has_pending_cue());
    }

    #[test]
    fn natural_end_retains_a_busy_second_cue_for_compatibility() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        start(&mut sequencer, false);
        sequencer.advance(TimeMicros(1_500_000), true).unwrap();

        assert_eq!(
            copied(&sequencer.advance(TimeMicros(2_500_000), true).unwrap()),
            vec![ComboCutInEvent::Ended {
                sequence_id: 1,
                milestone_combo: 100,
                reason: ComboCutInEndReason::Completed,
            }]
        );
        assert!(!sequencer.is_active());
        assert!(sequencer.has_pending_cue());

        assert_eq!(
            copied(&sequencer.advance(TimeMicros(500_000), false).unwrap()),
            vec![ComboCutInEvent::CueDispatched {
                sequence_id: 1,
                milestone_combo: 100,
                cut_in_index: 1,
                actor: SECOND_ACTOR,
                authored_at: TimeMicros(1_500_000),
                dispatched_at: TimeMicros(4_000_000),
            }]
        );
        assert!(!sequencer.has_pending_cue());
    }

    #[test]
    fn pause_consumes_no_sequence_time_and_emits_no_cues() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        start(&mut sequencer, false);
        sequencer.advance(TimeMicros(1_000_000), false).unwrap();
        sequencer.pause();

        assert!(
            sequencer
                .advance(TimeMicros(9_000_000), false)
                .unwrap()
                .is_empty()
        );
        assert!(
            sequencer
                .observe_frame(frame(200, 1), false)
                .unwrap()
                .is_empty()
        );
        assert_eq!(sequencer.active_elapsed(), Some(TimeMicros(1_000_000)));

        sequencer.resume();
        assert_eq!(
            sequencer.advance(TimeMicros(500_000), false).unwrap().len(),
            1
        );
        assert_eq!(sequencer.active_elapsed(), Some(TimeMicros(1_500_000)));
    }

    #[test]
    fn pause_also_holds_a_delayed_cue_until_the_first_resumed_frame() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        start(&mut sequencer, false);
        sequencer.advance(TimeMicros(1_500_000), true).unwrap();
        sequencer.pause();

        assert!(
            sequencer
                .advance(TimeMicros(10_000_000), false)
                .unwrap()
                .is_empty()
        );
        assert!(sequencer.has_pending_cue());
        sequencer.resume();
        assert!(matches!(
            sequencer.advance(TimeMicros(0), false).unwrap().get(0),
            Some(ComboCutInEvent::CueDispatched {
                dispatched_at: TimeMicros(1_500_000),
                ..
            })
        ));
    }

    #[test]
    fn cancel_ends_active_playback_and_clears_a_delayed_cue() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        start(&mut sequencer, false);
        sequencer.advance(TimeMicros(1_500_000), true).unwrap();

        assert_eq!(
            copied(&sequencer.cancel()),
            vec![ComboCutInEvent::Ended {
                sequence_id: 1,
                milestone_combo: 100,
                reason: ComboCutInEndReason::Cancelled,
            }]
        );
        assert!(!sequencer.has_pending_cue());
        assert!(
            sequencer
                .advance(TimeMicros(10_000_000), false)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn cancel_after_natural_end_clears_pending_without_a_second_end_event() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        start(&mut sequencer, false);
        sequencer.advance(COMBO_CUT_IN_DURATION, true).unwrap();
        assert!(!sequencer.is_active());
        assert!(sequencer.has_pending_cue());

        assert!(sequencer.cancel().is_empty());
        assert!(!sequencer.has_pending_cue());
        assert!(sequencer.advance(TimeMicros(0), false).unwrap().is_empty());
    }

    #[test]
    fn replacement_flushes_an_idle_pending_cue_before_ending_and_starting() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        start(&mut sequencer, false);
        sequencer.advance(TimeMicros(1_500_000), true).unwrap();

        let events = sequencer.observe_frame(frame(200, 1), false).unwrap();
        assert_eq!(events.len(), COMBO_CUT_IN_EVENT_CAPACITY);
        assert!(matches!(
            events.get(0),
            Some(ComboCutInEvent::CueDispatched {
                sequence_id: 1,
                cut_in_index: 1,
                ..
            })
        ));
        assert_eq!(
            events.get(1),
            Some(&ComboCutInEvent::Ended {
                sequence_id: 1,
                milestone_combo: 100,
                reason: ComboCutInEndReason::Replaced,
            })
        );
        assert!(matches!(
            events.get(2),
            Some(ComboCutInEvent::Started {
                sequence_id: 2,
                milestone_combo: 200,
                ..
            })
        ));
        assert!(matches!(
            events.get(3),
            Some(ComboCutInEvent::CueDispatched {
                sequence_id: 2,
                cut_in_index: 0,
                ..
            })
        ));
    }

    #[test]
    fn replacement_while_voice_stays_busy_discards_the_old_pending_cue() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        start(&mut sequencer, false);
        sequencer.advance(TimeMicros(1_500_000), true).unwrap();

        let events = sequencer.observe_frame(frame(200, 1), true).unwrap();
        assert_eq!(events.len(), 3);
        assert!(!sequencer.has_pending_cue());
        assert_eq!(sequencer.active_sequence_id(), Some(2));
    }

    #[test]
    fn disabling_blocks_new_starts_without_stopping_active_or_pending_work() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        start(&mut sequencer, false);
        sequencer.advance(TimeMicros(1_500_000), true).unwrap();
        sequencer.set_enabled(false);

        let events = sequencer.observe_frame(frame(200, 1), false).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events.get(0),
            Some(ComboCutInEvent::CueDispatched { .. })
        ));
        assert_eq!(sequencer.active_sequence_id(), Some(1));

        assert_eq!(
            sequencer
                .advance(TimeMicros(2_500_000), false)
                .unwrap()
                .get(0),
            Some(&ComboCutInEvent::Ended {
                sequence_id: 1,
                milestone_combo: 100,
                reason: ComboCutInEndReason::Completed,
            })
        );
        assert!(
            sequencer
                .observe_frame(frame(300, 1), false)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn future_sequences_use_a_replaced_cast_without_mutating_the_active_one() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        start(&mut sequencer, false);
        let replacement_actor = ComboCutInActor {
            character_id: 303,
            ..SECOND_ACTOR
        };
        let replacement =
            ComboCutInCast::new(ComboCutInSource::Common, [FIRST_ACTOR, replacement_actor])
                .unwrap();
        sequencer.set_cast(replacement);

        let cue = sequencer.advance(TimeMicros(1_500_000), false).unwrap();
        assert!(matches!(
            cue.get(0),
            Some(ComboCutInEvent::CueDispatched {
                actor: SECOND_ACTOR,
                ..
            })
        ));
        let next = sequencer.observe_frame(frame(200, 1), false).unwrap();
        assert!(matches!(
            next.get(1),
            Some(ComboCutInEvent::Started {
                source: ComboCutInSource::Common,
                actors: [_, actor],
                ..
            }) if *actor == replacement_actor
        ));
    }

    #[test]
    fn negative_delta_is_rejected_without_advancing_state() {
        let mut sequencer = ComboCutInSequencer::new(0, cast());
        start(&mut sequencer, false);
        assert_eq!(
            sequencer.advance(TimeMicros(-1), false),
            Err(ComboCutInError::NegativeDelta(TimeMicros(-1)))
        );
        assert_eq!(sequencer.active_elapsed(), Some(TimeMicros(0)));
    }
}
