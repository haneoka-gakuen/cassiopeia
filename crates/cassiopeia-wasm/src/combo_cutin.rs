use haneoka_cassiopeia_core::{
    COMBO_CUT_IN_EVENT_CAPACITY, ComboCutInActor, ComboCutInCast, ComboCutInEvent,
    ComboCutInEvents, ComboCutInFrame, ComboCutInRole, ComboCutInSequencer, ComboCutInSource,
    TimeMicros, combo_cutin_milestone,
};
use wasm_bindgen::prelude::*;

pub const COMBO_CUT_IN_EVENT_ABI_VERSION: i64 = 1;
pub const COMBO_CUT_IN_EVENT_STRIDE: usize = 18;
pub const COMBO_CUT_IN_EVENT_BUFFER_WORDS: usize =
    COMBO_CUT_IN_EVENT_STRIDE * COMBO_CUT_IN_EVENT_CAPACITY;

pub const COMBO_CUT_IN_EVENT_ABI: usize = 0;
pub const COMBO_CUT_IN_EVENT_KIND: usize = 1;
pub const COMBO_CUT_IN_EVENT_SEQUENCE_ID: usize = 2;
pub const COMBO_CUT_IN_EVENT_MILESTONE_COMBO: usize = 3;
pub const COMBO_CUT_IN_EVENT_SOURCE: usize = 4;
pub const COMBO_CUT_IN_EVENT_CUT_IN_INDEX: usize = 5;
pub const COMBO_CUT_IN_EVENT_END_REASON: usize = 6;
pub const COMBO_CUT_IN_EVENT_AUTHORED_AT: usize = 7;
pub const COMBO_CUT_IN_EVENT_DISPATCHED_AT: usize = 8;
pub const COMBO_CUT_IN_EVENT_DURATION: usize = 9;
pub const COMBO_CUT_IN_EVENT_ACTOR_ROLE: usize = 10;
pub const COMBO_CUT_IN_EVENT_ACTOR_CHARACTER_ID: usize = 11;
pub const COMBO_CUT_IN_EVENT_ACTOR_MEMBER_INDEX: usize = 12;
pub const COMBO_CUT_IN_EVENT_ACTOR_VOICE_ID: usize = 13;
pub const COMBO_CUT_IN_EVENT_PARTNER_ROLE: usize = 14;
pub const COMBO_CUT_IN_EVENT_PARTNER_CHARACTER_ID: usize = 15;
pub const COMBO_CUT_IN_EVENT_PARTNER_MEMBER_INDEX: usize = 16;
pub const COMBO_CUT_IN_EVENT_PARTNER_VOICE_ID: usize = 17;

const NO_COMBO_CUT_IN_VALUE: i64 = -1;

#[wasm_bindgen(js_name = ComboCutInEventWord)]
#[repr(u8)]
pub enum WasmComboCutInEventWord {
    AbiVersion = 0,
    Kind = 1,
    SequenceId = 2,
    MilestoneCombo = 3,
    Source = 4,
    CutInIndex = 5,
    EndReason = 6,
    AuthoredAtMicros = 7,
    DispatchedAtMicros = 8,
    DurationMicros = 9,
    ActorRole = 10,
    ActorCharacterId = 11,
    ActorMemberIndex = 12,
    ActorVoiceId = 13,
    PartnerRole = 14,
    PartnerCharacterId = 15,
    PartnerMemberIndex = 16,
    PartnerVoiceId = 17,
}

#[wasm_bindgen(js_name = ComboCutInEventKind)]
#[repr(u8)]
pub enum WasmComboCutInEventKind {
    Started = 0,
    CueDispatched = 1,
    Ended = 2,
}

#[wasm_bindgen(js_name = ComboCutInSource)]
#[repr(u8)]
pub enum WasmComboCutInSource {
    Fixed = 0,
    Common = 1,
}

#[wasm_bindgen(js_name = ComboCutInRole)]
#[repr(u8)]
pub enum WasmComboCutInRole {
    First = 0,
    Second = 1,
    Call = 2,
    Response = 3,
}

#[wasm_bindgen(js_name = ComboCutInEndReason)]
#[repr(u8)]
pub enum WasmComboCutInEndReason {
    Completed = 0,
    Cancelled = 1,
    Replaced = 2,
    Shutdown = 3,
}

/// Resolves one frame's newest hundred-combo milestone. `-1` means no trigger.
#[wasm_bindgen(js_name = resolveComboCutInMilestone)]
pub fn resolve_combo_cutin_milestone(
    combo_updated: bool,
    current_combo: u32,
    added_combo: u32,
    judged_judgement_notes: u32,
    total_judgement_notes: u32,
    disable_last_n_notes: u32,
) -> i64 {
    combo_cutin_milestone(
        ComboCutInFrame {
            combo_updated,
            current_combo,
            added_combo,
            judged_judgement_notes,
            total_judgement_notes,
        },
        disable_last_n_notes,
    )
    .map(i64::from)
    .unwrap_or(NO_COMBO_CUT_IN_VALUE)
}

/// Deterministic cut-in playback with a reusable `BigInt64Array` event region.
/// A cue event tells the host to dispatch its motion and voice together.
#[derive(Debug)]
#[wasm_bindgen(js_name = CassiopeiaComboCutInSequencer)]
pub struct WasmComboCutInSequencer {
    sequencer: ComboCutInSequencer,
    event_words: [i64; COMBO_CUT_IN_EVENT_BUFFER_WORDS],
    event_count: usize,
}

#[wasm_bindgen(js_class = CassiopeiaComboCutInSequencer)]
impl WasmComboCutInSequencer {
    #[wasm_bindgen(constructor)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        disable_last_n_notes: u32,
        source: u8,
        first_role: u8,
        first_character_id: i64,
        first_member_index: u8,
        first_voice_id: i64,
        second_role: u8,
        second_character_id: i64,
        second_member_index: u8,
        second_voice_id: i64,
    ) -> Result<WasmComboCutInSequencer, JsError> {
        Self::from_parts(
            disable_last_n_notes,
            source,
            first_role,
            first_character_id,
            first_member_index,
            first_voice_id,
            second_role,
            second_character_id,
            second_member_index,
            second_voice_id,
        )
        .map_err(super::js_error)
    }

    #[wasm_bindgen(js_name = setCast)]
    #[allow(clippy::too_many_arguments)]
    pub fn set_cast(
        &mut self,
        source: u8,
        first_role: u8,
        first_character_id: i64,
        first_member_index: u8,
        first_voice_id: i64,
        second_role: u8,
        second_character_id: i64,
        second_member_index: u8,
        second_voice_id: i64,
    ) -> Result<(), JsError> {
        self.clear_events();
        self.sequencer.set_cast(
            cast_from_parts(
                source,
                first_role,
                first_character_id,
                first_member_index,
                first_voice_id,
                second_role,
                second_character_id,
                second_member_index,
                second_voice_id,
            )
            .map_err(super::js_error)?,
        );
        Ok(())
    }

    #[wasm_bindgen(js_name = setDisableLastNNotes)]
    pub fn set_disable_last_n_notes(&mut self, value: u32) {
        self.clear_events();
        self.sequencer.set_disable_last_n_notes(value);
    }

    #[wasm_bindgen(js_name = setEnabled)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.clear_events();
        self.sequencer.set_enabled(enabled);
    }

    #[wasm_bindgen(js_name = observeFrame)]
    pub fn observe_frame(
        &mut self,
        combo_updated: bool,
        current_combo: u32,
        added_combo: u32,
        judged_judgement_notes: u32,
        total_judgement_notes: u32,
        voice_busy: bool,
    ) -> Result<u32, JsError> {
        self.observe_frame_inner(
            ComboCutInFrame {
                combo_updated,
                current_combo,
                added_combo,
                judged_judgement_notes,
                total_judgement_notes,
            },
            voice_busy,
        )
        .map_err(super::js_error)
    }

    pub fn advance(&mut self, delta_micros: i64, voice_busy: bool) -> Result<u32, JsError> {
        self.advance_inner(TimeMicros(delta_micros), voice_busy)
            .map_err(super::js_error)
    }

    pub fn pause(&mut self) {
        self.clear_events();
        self.sequencer.pause();
    }

    pub fn resume(&mut self) {
        self.clear_events();
        self.sequencer.resume();
    }

    pub fn cancel(&mut self) -> Result<u32, JsError> {
        let events = self.sequencer.cancel();
        self.write_events(&events).map_err(super::js_error)
    }

    pub fn shutdown(&mut self) -> Result<u32, JsError> {
        let events = self.sequencer.shutdown();
        self.write_events(&events).map_err(super::js_error)
    }

    #[wasm_bindgen(js_name = isActive)]
    pub fn is_active(&self) -> bool {
        self.sequencer.is_active()
    }

    #[wasm_bindgen(js_name = hasPendingCue)]
    pub fn has_pending_cue(&self) -> bool {
        self.sequencer.has_pending_cue()
    }

    #[wasm_bindgen(js_name = isPaused)]
    pub fn is_paused(&self) -> bool {
        self.sequencer.is_paused()
    }

    #[wasm_bindgen(js_name = isEnabled)]
    pub fn is_enabled(&self) -> bool {
        self.sequencer.is_enabled()
    }

    #[wasm_bindgen(js_name = eventBufferPtr)]
    pub fn event_buffer_ptr(&self) -> usize {
        self.event_words.as_ptr() as usize
    }

    #[wasm_bindgen(js_name = eventBufferLen)]
    pub fn event_buffer_len(&self) -> usize {
        COMBO_CUT_IN_EVENT_BUFFER_WORDS
    }

    #[wasm_bindgen(js_name = eventBufferStride)]
    pub fn event_buffer_stride(&self) -> usize {
        COMBO_CUT_IN_EVENT_STRIDE
    }

    #[wasm_bindgen(js_name = eventCount)]
    pub fn event_count(&self) -> usize {
        self.event_count
    }
}

impl WasmComboCutInSequencer {
    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        disable_last_n_notes: u32,
        source: u8,
        first_role: u8,
        first_character_id: i64,
        first_member_index: u8,
        first_voice_id: i64,
        second_role: u8,
        second_character_id: i64,
        second_member_index: u8,
        second_voice_id: i64,
    ) -> Result<Self, String> {
        let cast = cast_from_parts(
            source,
            first_role,
            first_character_id,
            first_member_index,
            first_voice_id,
            second_role,
            second_character_id,
            second_member_index,
            second_voice_id,
        )?;
        Ok(Self {
            sequencer: ComboCutInSequencer::new(disable_last_n_notes, cast),
            event_words: [NO_COMBO_CUT_IN_VALUE; COMBO_CUT_IN_EVENT_BUFFER_WORDS],
            event_count: 0,
        })
    }

    fn observe_frame_inner(
        &mut self,
        frame: ComboCutInFrame,
        voice_busy: bool,
    ) -> Result<u32, String> {
        self.clear_events();
        let events = self
            .sequencer
            .observe_frame(frame, voice_busy)
            .map_err(|error| error.to_string())?;
        self.write_events(&events)
    }

    fn advance_inner(&mut self, delta: TimeMicros, voice_busy: bool) -> Result<u32, String> {
        self.clear_events();
        let events = self
            .sequencer
            .advance(delta, voice_busy)
            .map_err(|error| error.to_string())?;
        self.write_events(&events)
    }

    fn clear_events(&mut self) {
        self.event_words.fill(NO_COMBO_CUT_IN_VALUE);
        self.event_count = 0;
    }

    fn write_events(&mut self, events: &ComboCutInEvents) -> Result<u32, String> {
        self.clear_events();
        if events.len() > COMBO_CUT_IN_EVENT_CAPACITY {
            return Err("combo cut-in event capacity exceeded".to_owned());
        }
        for (event_index, event) in events.iter().enumerate() {
            let base = event_index * COMBO_CUT_IN_EVENT_STRIDE;
            let words = &mut self.event_words[base..base + COMBO_CUT_IN_EVENT_STRIDE];
            words[COMBO_CUT_IN_EVENT_ABI] = COMBO_CUT_IN_EVENT_ABI_VERSION;
            match *event {
                ComboCutInEvent::Started {
                    sequence_id,
                    milestone_combo,
                    source,
                    actors,
                    duration,
                } => {
                    write_common_event(
                        words,
                        WasmComboCutInEventKind::Started,
                        sequence_id,
                        milestone_combo,
                    )?;
                    words[COMBO_CUT_IN_EVENT_SOURCE] = i64::from(source as u8);
                    words[COMBO_CUT_IN_EVENT_DURATION] = duration.0;
                    write_actor(words, COMBO_CUT_IN_EVENT_ACTOR_ROLE, actors[0]);
                    write_actor(words, COMBO_CUT_IN_EVENT_PARTNER_ROLE, actors[1]);
                }
                ComboCutInEvent::CueDispatched {
                    sequence_id,
                    milestone_combo,
                    cut_in_index,
                    actor,
                    authored_at,
                    dispatched_at,
                } => {
                    write_common_event(
                        words,
                        WasmComboCutInEventKind::CueDispatched,
                        sequence_id,
                        milestone_combo,
                    )?;
                    words[COMBO_CUT_IN_EVENT_CUT_IN_INDEX] = i64::from(cut_in_index);
                    words[COMBO_CUT_IN_EVENT_AUTHORED_AT] = authored_at.0;
                    words[COMBO_CUT_IN_EVENT_DISPATCHED_AT] = dispatched_at.0;
                    write_actor(words, COMBO_CUT_IN_EVENT_ACTOR_ROLE, actor);
                }
                ComboCutInEvent::Ended {
                    sequence_id,
                    milestone_combo,
                    reason,
                } => {
                    write_common_event(
                        words,
                        WasmComboCutInEventKind::Ended,
                        sequence_id,
                        milestone_combo,
                    )?;
                    words[COMBO_CUT_IN_EVENT_END_REASON] = i64::from(reason as u8);
                }
            }
        }
        self.event_count = events.len();
        u32::try_from(self.event_count)
            .map_err(|_| "combo cut-in event count overflowed".to_owned())
    }
}

#[allow(clippy::too_many_arguments)]
fn cast_from_parts(
    source: u8,
    first_role: u8,
    first_character_id: i64,
    first_member_index: u8,
    first_voice_id: i64,
    second_role: u8,
    second_character_id: i64,
    second_member_index: u8,
    second_voice_id: i64,
) -> Result<ComboCutInCast, String> {
    ComboCutInCast::new(
        ComboCutInSource::try_from(source).map_err(|error| error.to_string())?,
        [
            ComboCutInActor {
                role: ComboCutInRole::try_from(first_role).map_err(|error| error.to_string())?,
                character_id: first_character_id,
                member_index: first_member_index,
                voice_id: first_voice_id,
            },
            ComboCutInActor {
                role: ComboCutInRole::try_from(second_role).map_err(|error| error.to_string())?,
                character_id: second_character_id,
                member_index: second_member_index,
                voice_id: second_voice_id,
            },
        ],
    )
    .map_err(|error| error.to_string())
}

fn write_common_event(
    words: &mut [i64],
    kind: WasmComboCutInEventKind,
    sequence_id: u64,
    milestone_combo: u32,
) -> Result<(), String> {
    words[COMBO_CUT_IN_EVENT_KIND] = i64::from(kind as u8);
    words[COMBO_CUT_IN_EVENT_SEQUENCE_ID] = i64::try_from(sequence_id)
        .map_err(|_| "combo cut-in sequence ID exceeds the signed host ABI".to_owned())?;
    words[COMBO_CUT_IN_EVENT_MILESTONE_COMBO] = i64::from(milestone_combo);
    Ok(())
}

fn write_actor(words: &mut [i64], role_offset: usize, actor: ComboCutInActor) {
    words[role_offset] = i64::from(actor.role as u8);
    words[role_offset + 1] = actor.character_id;
    words[role_offset + 2] = i64::from(actor.member_index);
    words[role_offset + 3] = actor.voice_id;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sequencer() -> WasmComboCutInSequencer {
        WasmComboCutInSequencer::from_parts(
            0,
            ComboCutInSource::Fixed as u8,
            ComboCutInRole::Call as u8,
            101,
            1,
            1_001,
            ComboCutInRole::Response as u8,
            202,
            4,
            2_002,
        )
        .unwrap()
    }

    fn milestone_frame(combo: u32) -> ComboCutInFrame {
        ComboCutInFrame {
            combo_updated: true,
            current_combo: combo,
            added_combo: 1,
            judged_judgement_notes: 200,
            total_judgement_notes: 1_000,
        }
    }

    fn event(machine: &WasmComboCutInSequencer, index: usize) -> &[i64] {
        let base = index * COMBO_CUT_IN_EVENT_STRIDE;
        &machine.event_words[base..base + COMBO_CUT_IN_EVENT_STRIDE]
    }

    #[test]
    fn fixed_event_region_encodes_start_and_first_cue_without_allocating_again() {
        let mut machine = sequencer();
        let pointer = machine.event_buffer_ptr();
        assert_eq!(machine.event_buffer_len(), COMBO_CUT_IN_EVENT_BUFFER_WORDS);
        assert_eq!(machine.event_buffer_stride(), COMBO_CUT_IN_EVENT_STRIDE);

        assert_eq!(
            machine.observe_frame_inner(milestone_frame(100), true),
            Ok(2)
        );
        assert_eq!(machine.event_buffer_ptr(), pointer);
        assert_eq!(machine.event_count(), 2);

        let started = event(&machine, 0);
        assert_eq!(started[COMBO_CUT_IN_EVENT_ABI], 1);
        assert_eq!(started[COMBO_CUT_IN_EVENT_KIND], 0);
        assert_eq!(started[COMBO_CUT_IN_EVENT_SEQUENCE_ID], 1);
        assert_eq!(started[COMBO_CUT_IN_EVENT_MILESTONE_COMBO], 100);
        assert_eq!(started[COMBO_CUT_IN_EVENT_SOURCE], 0);
        assert_eq!(started[COMBO_CUT_IN_EVENT_DURATION], 4_000_000);
        assert_eq!(started[COMBO_CUT_IN_EVENT_ACTOR_ROLE], 2);
        assert_eq!(started[COMBO_CUT_IN_EVENT_ACTOR_CHARACTER_ID], 101);
        assert_eq!(started[COMBO_CUT_IN_EVENT_ACTOR_MEMBER_INDEX], 1);
        assert_eq!(started[COMBO_CUT_IN_EVENT_ACTOR_VOICE_ID], 1_001);
        assert_eq!(started[COMBO_CUT_IN_EVENT_PARTNER_ROLE], 3);
        assert_eq!(started[COMBO_CUT_IN_EVENT_PARTNER_CHARACTER_ID], 202);
        assert_eq!(started[COMBO_CUT_IN_EVENT_PARTNER_MEMBER_INDEX], 4);
        assert_eq!(started[COMBO_CUT_IN_EVENT_PARTNER_VOICE_ID], 2_002);

        let cue = event(&machine, 1);
        assert_eq!(cue[COMBO_CUT_IN_EVENT_KIND], 1);
        assert_eq!(cue[COMBO_CUT_IN_EVENT_CUT_IN_INDEX], 0);
        assert_eq!(cue[COMBO_CUT_IN_EVENT_AUTHORED_AT], 0);
        assert_eq!(cue[COMBO_CUT_IN_EVENT_DISPATCHED_AT], 0);
        assert_eq!(cue[COMBO_CUT_IN_EVENT_ACTOR_CHARACTER_ID], 101);
        assert!(
            machine.event_words[2 * COMBO_CUT_IN_EVENT_STRIDE..]
                .iter()
                .all(|word| *word == NO_COMBO_CUT_IN_VALUE)
        );
    }

    #[test]
    fn event_region_reuses_pointer_for_pending_completion_and_late_dispatch() {
        let mut machine = sequencer();
        machine
            .observe_frame_inner(milestone_frame(100), false)
            .unwrap();
        let pointer = machine.event_buffer_ptr();

        assert_eq!(machine.advance_inner(TimeMicros(1_500_000), true), Ok(0));
        assert!(machine.has_pending_cue());
        assert!(machine.event_words.iter().all(|word| *word == -1));

        assert_eq!(machine.advance_inner(TimeMicros(2_500_000), true), Ok(1));
        assert_eq!(machine.event_buffer_ptr(), pointer);
        assert_eq!(event(&machine, 0)[COMBO_CUT_IN_EVENT_KIND], 2);
        assert_eq!(event(&machine, 0)[COMBO_CUT_IN_EVENT_END_REASON], 0);
        assert!(machine.has_pending_cue());

        assert_eq!(machine.advance_inner(TimeMicros(0), false), Ok(1));
        let cue = event(&machine, 0);
        assert_eq!(cue[COMBO_CUT_IN_EVENT_KIND], 1);
        assert_eq!(cue[COMBO_CUT_IN_EVENT_CUT_IN_INDEX], 1);
        assert_eq!(cue[COMBO_CUT_IN_EVENT_AUTHORED_AT], 1_500_000);
        assert_eq!(cue[COMBO_CUT_IN_EVENT_DISPATCHED_AT], 4_000_000);
        assert_eq!(cue[COMBO_CUT_IN_EVENT_ACTOR_CHARACTER_ID], 202);
        assert_eq!(machine.event_buffer_ptr(), pointer);
    }

    #[test]
    fn cancel_writes_one_end_event_and_removes_pending_state() {
        let mut machine = sequencer();
        machine
            .observe_frame_inner(milestone_frame(100), false)
            .unwrap();
        machine.advance_inner(TimeMicros(1_500_000), true).unwrap();

        let events = machine.sequencer.cancel();
        assert_eq!(machine.write_events(&events), Ok(1));
        assert_eq!(event(&machine, 0)[COMBO_CUT_IN_EVENT_KIND], 2);
        assert_eq!(event(&machine, 0)[COMBO_CUT_IN_EVENT_END_REASON], 1);
        assert!(!machine.has_pending_cue());
    }

    #[test]
    fn milestone_resolver_uses_negative_one_as_the_no_event_sentinel() {
        assert_eq!(resolve_combo_cutin_milestone(true, 100, 1, 10, 20, 0), 100);
        assert_eq!(resolve_combo_cutin_milestone(false, 100, 1, 10, 20, 0), -1);
        assert_eq!(resolve_combo_cutin_milestone(true, 100, 1, 15, 20, 5), -1);
    }
}
