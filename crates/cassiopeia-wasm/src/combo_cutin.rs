use haneoka_cassiopeia_core::{
    COMBO_CUT_IN_EVENT_CAPACITY, ComboCharacterCommonVoice, ComboCharacterDialogueRole,
    ComboCharacterFixedPair, ComboCharacterLotteryError, ComboCharacterLotteryMachine,
    ComboCharacterLotteryOutcome, ComboCharacterLotteryPartialFailure, ComboCharacterLotteryRng,
    ComboCharacterVoice, ComboCutInActor, ComboCutInCast, ComboCutInEvent, ComboCutInEvents,
    ComboCutInFrame, ComboCutInRole, ComboCutInSequencer, ComboCutInSource, TimeMicros,
    combo_cutin_milestone,
};
use std::collections::VecDeque;
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

pub const COMBO_CHARACTER_COMMON_INPUT_STRIDE: usize = 4;
pub const COMBO_CHARACTER_FIXED_INPUT_STRIDE: usize = 5;
pub const COMBO_CHARACTER_LOTTERY_ABI_VERSION: i64 = 1;
pub const COMBO_CHARACTER_LOTTERY_WORDS: usize = 18;

pub const COMBO_CHARACTER_LOTTERY_ABI: usize = 0;
pub const COMBO_CHARACTER_LOTTERY_KIND: usize = 1;
pub const COMBO_CHARACTER_LOTTERY_SOURCE: usize = 2;
pub const COMBO_CHARACTER_LOTTERY_FAILURE: usize = 3;
pub const COMBO_CHARACTER_LOTTERY_FAILURE_ROLE: usize = 4;
pub const COMBO_CHARACTER_LOTTERY_IGNORED_CHARACTER_ID: usize = 5;
pub const COMBO_CHARACTER_LOTTERY_FIRST_ROLE: usize = 6;
pub const COMBO_CHARACTER_LOTTERY_FIRST_DIALOGUE_ID: usize = 7;
pub const COMBO_CHARACTER_LOTTERY_FIRST_CHARACTER_ID: usize = 8;
pub const COMBO_CHARACTER_LOTTERY_FIRST_VOICE_ID: usize = 9;
pub const COMBO_CHARACTER_LOTTERY_SECOND_ROLE: usize = 10;
pub const COMBO_CHARACTER_LOTTERY_SECOND_DIALOGUE_ID: usize = 11;
pub const COMBO_CHARACTER_LOTTERY_SECOND_CHARACTER_ID: usize = 12;
pub const COMBO_CHARACTER_LOTTERY_SECOND_VOICE_ID: usize = 13;
pub const COMBO_CHARACTER_LOTTERY_COMMON_REMAINING: usize = 14;
pub const COMBO_CHARACTER_LOTTERY_FIXED_REMAINING: usize = 15;
pub const COMBO_CHARACTER_LOTTERY_ERROR_UPPER_EXCLUSIVE: usize = 16;
pub const COMBO_CHARACTER_LOTTERY_ERROR_ACTUAL: usize = 17;

#[wasm_bindgen(js_name = ComboCharacterLotteryWord)]
#[repr(u8)]
pub enum WasmComboCharacterLotteryWord {
    AbiVersion = 0,
    Kind = 1,
    Source = 2,
    Failure = 3,
    FailureRole = 4,
    IgnoredCharacterId = 5,
    FirstRole = 6,
    FirstDialogueId = 7,
    FirstCharacterId = 8,
    FirstVoiceId = 9,
    SecondRole = 10,
    SecondDialogueId = 11,
    SecondCharacterId = 12,
    SecondVoiceId = 13,
    CommonRemaining = 14,
    FixedRemaining = 15,
    ErrorUpperExclusive = 16,
    ErrorActual = 17,
}

#[wasm_bindgen(js_name = ComboCharacterLotteryKind)]
#[repr(u8)]
pub enum WasmComboCharacterLotteryKind {
    Unavailable = 0,
    Fixed = 1,
    Common = 2,
    CommonPartial = 3,
    Error = 4,
}

#[wasm_bindgen(js_name = ComboCharacterLotteryFailure)]
#[repr(u8)]
pub enum WasmComboCharacterLotteryFailure {
    ResponseUnavailable = 0,
    CommonVoiceUnavailableAfterRefresh = 1,
    RandomIndexOutOfRange = 2,
}

#[wasm_bindgen(js_name = ComboCharacterDialogueRole)]
#[repr(u8)]
pub enum WasmComboCharacterDialogueRole {
    Call = 1,
    Response = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WasmComboCharacterRng {
    state: u64,
    injected_indices: VecDeque<usize>,
    injected_mode: bool,
}

impl WasmComboCharacterRng {
    fn new(seed: u32) -> Self {
        Self {
            state: seed as u64,
            injected_indices: VecDeque::new(),
            injected_mode: false,
        }
    }

    fn reseed(&mut self, seed: u32) {
        self.state = seed as u64;
    }

    fn load_indices(&mut self, indices: &[u32]) {
        self.injected_indices.clear();
        self.injected_indices
            .extend(indices.iter().map(|index| *index as usize));
        self.injected_mode = true;
    }

    fn use_seeded(&mut self) {
        self.injected_indices.clear();
        self.injected_mode = false;
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }
}

impl ComboCharacterLotteryRng for WasmComboCharacterRng {
    fn range(&mut self, upper_exclusive: usize) -> usize {
        if self.injected_mode {
            return self.injected_indices.pop_front().unwrap_or(upper_exclusive);
        }
        if upper_exclusive <= 1 {
            return 0;
        }

        let bound = upper_exclusive as u64;
        let rejection_threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.next_u64();
            if value >= rejection_threshold {
                return (value % bound) as usize;
            }
        }
    }
}

/// Preloaded combo-character lottery with seeded in-WASM shuffling and a
/// reusable `BigInt64Array` result region.
///
/// Common rows are `[role, dialogueId, characterId, voiceId]`, where role uses
/// the master values `1 = Call` and `2 = Response`. Fixed rows are
/// `[pairId, firstCharacterId, firstVoiceId, secondCharacterId, secondVoiceId]`.
#[derive(Debug)]
#[wasm_bindgen(js_name = CassiopeiaComboCharacterLotteryMachine)]
pub struct WasmComboCharacterLotteryMachine {
    machine: ComboCharacterLotteryMachine,
    rng: WasmComboCharacterRng,
    result_words: [i64; COMBO_CHARACTER_LOTTERY_WORDS],
}

#[wasm_bindgen(js_class = CassiopeiaComboCharacterLotteryMachine)]
impl WasmComboCharacterLotteryMachine {
    #[wasm_bindgen(constructor)]
    pub fn new(
        common_words: &[i64],
        fixed_words: &[i64],
        seed: u32,
    ) -> Result<WasmComboCharacterLotteryMachine, JsError> {
        Self::from_flat_words(common_words, fixed_words, seed).map_err(super::js_error)
    }

    /// Draws fixed or common call/response data into the stable result region.
    pub fn draw(&mut self) -> u32 {
        self.draw_with_shortage_refresh(true)
    }

    /// Primitive host control for diagnostics and compatibility fixtures.
    /// Production playback should call `draw()`, which always refreshes a
    /// common shortage.
    #[wasm_bindgen(js_name = drawWithShortageRefresh)]
    pub fn draw_with_shortage_refresh(&mut self, shortage_refresh: bool) -> u32 {
        let outcome = self.machine.draw(shortage_refresh, &mut self.rng);
        self.write_outcome(outcome) as u32
    }

    /// Replaces only the PRNG state used by future queue refreshes.
    pub fn reseed(&mut self, seed: u32) {
        self.rng.reseed(seed);
    }

    /// Replaces the pending exact `Random.Range` indices used by future queue
    /// refreshes. Exhaustion is reported through the result ABI instead of
    /// silently falling back to the seeded generator.
    #[wasm_bindgen(js_name = loadRandomIndices)]
    pub fn load_random_indices(&mut self, indices: &[u32]) {
        self.rng.load_indices(indices);
    }

    /// Returns future refreshes to the in-WASM seeded generator.
    #[wasm_bindgen(js_name = useSeededRandom)]
    pub fn use_seeded_random(&mut self) {
        self.rng.use_seeded();
    }

    #[wasm_bindgen(js_name = randomIndicesRemaining)]
    pub fn random_indices_remaining(&self) -> usize {
        self.rng.injected_indices.len()
    }

    /// Explicitly refreshes both queues using the current seeded PRNG state.
    pub fn refresh(&mut self) -> bool {
        match self.machine.refresh(&mut self.rng) {
            Ok(()) => {
                self.clear_result(WasmComboCharacterLotteryKind::Unavailable);
                true
            }
            Err(error) => {
                self.write_error(error);
                false
            }
        }
    }

    #[wasm_bindgen(js_name = resultBufferPtr)]
    pub fn result_buffer_ptr(&self) -> usize {
        self.result_words.as_ptr() as usize
    }

    #[wasm_bindgen(js_name = resultBufferLen)]
    pub fn result_buffer_len(&self) -> usize {
        COMBO_CHARACTER_LOTTERY_WORDS
    }

    #[wasm_bindgen(js_name = commonInputStride)]
    pub fn common_input_stride(&self) -> usize {
        COMBO_CHARACTER_COMMON_INPUT_STRIDE
    }

    #[wasm_bindgen(js_name = fixedInputStride)]
    pub fn fixed_input_stride(&self) -> usize {
        COMBO_CHARACTER_FIXED_INPUT_STRIDE
    }

    #[wasm_bindgen(js_name = commonRemaining)]
    pub fn common_remaining(&self) -> usize {
        self.machine.common_remaining()
    }

    #[wasm_bindgen(js_name = fixedRemaining)]
    pub fn fixed_remaining(&self) -> usize {
        self.machine.fixed_remaining()
    }
}

impl WasmComboCharacterLotteryMachine {
    fn from_flat_words(
        common_words: &[i64],
        fixed_words: &[i64],
        seed: u32,
    ) -> Result<Self, String> {
        if !common_words
            .len()
            .is_multiple_of(COMBO_CHARACTER_COMMON_INPUT_STRIDE)
        {
            return Err(format!(
                "combo-character common input length {} is not a multiple of {}",
                common_words.len(),
                COMBO_CHARACTER_COMMON_INPUT_STRIDE
            ));
        }
        if !fixed_words
            .len()
            .is_multiple_of(COMBO_CHARACTER_FIXED_INPUT_STRIDE)
        {
            return Err(format!(
                "combo-character fixed input length {} is not a multiple of {}",
                fixed_words.len(),
                COMBO_CHARACTER_FIXED_INPUT_STRIDE
            ));
        }

        let mut common_source =
            Vec::with_capacity(common_words.len() / COMBO_CHARACTER_COMMON_INPUT_STRIDE);
        for words in common_words.chunks_exact(COMBO_CHARACTER_COMMON_INPUT_STRIDE) {
            let role = match words[0] {
                1 => ComboCharacterDialogueRole::Call,
                2 => ComboCharacterDialogueRole::Response,
                value => return Err(format!("invalid combo-character dialogue role: {value}")),
            };
            common_source.push(ComboCharacterCommonVoice {
                dialogue_id: words[1],
                role,
                voice: ComboCharacterVoice {
                    character_id: words[2],
                    voice_id: words[3],
                },
            });
        }

        let mut fixed_source =
            Vec::with_capacity(fixed_words.len() / COMBO_CHARACTER_FIXED_INPUT_STRIDE);
        for words in fixed_words.chunks_exact(COMBO_CHARACTER_FIXED_INPUT_STRIDE) {
            fixed_source.push(ComboCharacterFixedPair {
                dialogue_id: words[0],
                first: ComboCharacterVoice {
                    character_id: words[1],
                    voice_id: words[2],
                },
                second: ComboCharacterVoice {
                    character_id: words[3],
                    voice_id: words[4],
                },
            });
        }

        let mut rng = WasmComboCharacterRng::new(seed);
        let machine = ComboCharacterLotteryMachine::new(common_source, fixed_source, &mut rng)
            .map_err(|error| error.to_string())?;
        let mut result = Self {
            machine,
            rng,
            result_words: [NO_COMBO_CUT_IN_VALUE; COMBO_CHARACTER_LOTTERY_WORDS],
        };
        result.clear_result(WasmComboCharacterLotteryKind::Unavailable);
        Ok(result)
    }

    fn write_outcome(
        &mut self,
        outcome: Result<ComboCharacterLotteryOutcome, ComboCharacterLotteryError>,
    ) -> WasmComboCharacterLotteryKind {
        match outcome {
            Ok(ComboCharacterLotteryOutcome::Unavailable) => {
                self.clear_result(WasmComboCharacterLotteryKind::Unavailable);
                WasmComboCharacterLotteryKind::Unavailable
            }
            Ok(ComboCharacterLotteryOutcome::Fixed(pair)) => {
                self.clear_result(WasmComboCharacterLotteryKind::Fixed);
                self.result_words[COMBO_CHARACTER_LOTTERY_SOURCE] =
                    i64::from(ComboCutInSource::Fixed as u8);
                write_lottery_voice(
                    &mut self.result_words,
                    COMBO_CHARACTER_LOTTERY_FIRST_ROLE,
                    ComboCutInRole::First,
                    pair.dialogue_id,
                    pair.first,
                );
                write_lottery_voice(
                    &mut self.result_words,
                    COMBO_CHARACTER_LOTTERY_SECOND_ROLE,
                    ComboCutInRole::Second,
                    pair.dialogue_id,
                    pair.second,
                );
                WasmComboCharacterLotteryKind::Fixed
            }
            Ok(ComboCharacterLotteryOutcome::Common { call, response }) => {
                self.clear_result(WasmComboCharacterLotteryKind::Common);
                self.result_words[COMBO_CHARACTER_LOTTERY_SOURCE] =
                    i64::from(ComboCutInSource::Common as u8);
                write_lottery_voice(
                    &mut self.result_words,
                    COMBO_CHARACTER_LOTTERY_FIRST_ROLE,
                    ComboCutInRole::Call,
                    call.dialogue_id,
                    call.voice,
                );
                write_lottery_voice(
                    &mut self.result_words,
                    COMBO_CHARACTER_LOTTERY_SECOND_ROLE,
                    ComboCutInRole::Response,
                    response.dialogue_id,
                    response.voice,
                );
                WasmComboCharacterLotteryKind::Common
            }
            Ok(ComboCharacterLotteryOutcome::CommonPartial { call, failure }) => {
                self.clear_result(WasmComboCharacterLotteryKind::CommonPartial);
                self.result_words[COMBO_CHARACTER_LOTTERY_SOURCE] =
                    i64::from(ComboCutInSource::Common as u8);
                write_lottery_voice(
                    &mut self.result_words,
                    COMBO_CHARACTER_LOTTERY_FIRST_ROLE,
                    ComboCutInRole::Call,
                    call.dialogue_id,
                    call.voice,
                );
                self.write_partial_failure(failure);
                WasmComboCharacterLotteryKind::CommonPartial
            }
            Err(error) => {
                self.write_error(error);
                WasmComboCharacterLotteryKind::Error
            }
        }
    }

    fn clear_result(&mut self, kind: WasmComboCharacterLotteryKind) {
        self.result_words.fill(NO_COMBO_CUT_IN_VALUE);
        self.result_words[COMBO_CHARACTER_LOTTERY_ABI] = COMBO_CHARACTER_LOTTERY_ABI_VERSION;
        self.result_words[COMBO_CHARACTER_LOTTERY_KIND] = i64::from(kind as u8);
        self.write_remaining();
    }

    fn write_remaining(&mut self) {
        self.result_words[COMBO_CHARACTER_LOTTERY_COMMON_REMAINING] =
            self.machine.common_remaining() as i64;
        self.result_words[COMBO_CHARACTER_LOTTERY_FIXED_REMAINING] =
            self.machine.fixed_remaining() as i64;
    }

    fn write_partial_failure(&mut self, failure: ComboCharacterLotteryPartialFailure) {
        match failure {
            ComboCharacterLotteryPartialFailure::ResponseUnavailable {
                ignored_character_id,
            } => {
                self.result_words[COMBO_CHARACTER_LOTTERY_FAILURE] =
                    i64::from(WasmComboCharacterLotteryFailure::ResponseUnavailable as u8);
                self.result_words[COMBO_CHARACTER_LOTTERY_FAILURE_ROLE] =
                    i64::from(ComboCharacterDialogueRole::Response as u8);
                self.result_words[COMBO_CHARACTER_LOTTERY_IGNORED_CHARACTER_ID] =
                    ignored_character_id;
            }
            ComboCharacterLotteryPartialFailure::Error(error) => {
                self.write_error_fields(error);
            }
        }
    }

    fn write_error(&mut self, error: ComboCharacterLotteryError) {
        self.clear_result(WasmComboCharacterLotteryKind::Error);
        self.write_error_fields(error);
    }

    fn write_error_fields(&mut self, error: ComboCharacterLotteryError) {
        match error {
            ComboCharacterLotteryError::RandomIndexOutOfRange {
                upper_exclusive,
                actual,
            } => {
                self.result_words[COMBO_CHARACTER_LOTTERY_FAILURE] =
                    i64::from(WasmComboCharacterLotteryFailure::RandomIndexOutOfRange as u8);
                self.result_words[COMBO_CHARACTER_LOTTERY_ERROR_UPPER_EXCLUSIVE] =
                    upper_exclusive as i64;
                self.result_words[COMBO_CHARACTER_LOTTERY_ERROR_ACTUAL] = actual as i64;
            }
            ComboCharacterLotteryError::CommonVoiceUnavailableAfterRefresh {
                role,
                ignored_character_id,
            } => {
                self.result_words[COMBO_CHARACTER_LOTTERY_SOURCE] =
                    i64::from(ComboCutInSource::Common as u8);
                self.result_words[COMBO_CHARACTER_LOTTERY_FAILURE] = i64::from(
                    WasmComboCharacterLotteryFailure::CommonVoiceUnavailableAfterRefresh as u8,
                );
                self.result_words[COMBO_CHARACTER_LOTTERY_FAILURE_ROLE] = i64::from(role as u8);
                self.result_words[COMBO_CHARACTER_LOTTERY_IGNORED_CHARACTER_ID] =
                    ignored_character_id;
            }
        }
    }
}

fn write_lottery_voice(
    words: &mut [i64; COMBO_CHARACTER_LOTTERY_WORDS],
    role_offset: usize,
    role: ComboCutInRole,
    dialogue_id: i64,
    voice: ComboCharacterVoice,
) {
    words[role_offset] = i64::from(role as u8);
    words[role_offset + 1] = dialogue_id;
    words[role_offset + 2] = voice.character_id;
    words[role_offset + 3] = voice.voice_id;
}

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

    const COMMON_WORDS: [i64; COMBO_CHARACTER_COMMON_INPUT_STRIDE * 2] = [
        1, 11, 101, 1_001, // call
        2, 22, 202, 2_002, // response
    ];
    const FIXED_WORDS: [i64; COMBO_CHARACTER_FIXED_INPUT_STRIDE] = [33, 303, 3_003, 404, 4_004];

    fn lottery(common_words: &[i64], fixed_words: &[i64]) -> WasmComboCharacterLotteryMachine {
        WasmComboCharacterLotteryMachine::from_flat_words(common_words, fixed_words, 42).unwrap()
    }

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
    fn lottery_boundary_parses_flat_rows_and_reuses_one_result_region() {
        let mut machine = lottery(&COMMON_WORDS, &FIXED_WORDS);
        let pointer = machine.result_buffer_ptr();

        assert_eq!(machine.common_input_stride(), 4);
        assert_eq!(machine.fixed_input_stride(), 5);
        assert_eq!(machine.result_buffer_len(), 18);
        machine.load_random_indices(&[0]);
        assert_eq!(machine.random_indices_remaining(), 1);
        assert!(machine.refresh());
        assert_eq!(machine.random_indices_remaining(), 0);
        assert_eq!(machine.draw(), 1);
        assert_eq!(machine.result_buffer_ptr(), pointer);
        assert_eq!(machine.result_words[COMBO_CHARACTER_LOTTERY_ABI], 1);
        assert_eq!(machine.result_words[COMBO_CHARACTER_LOTTERY_KIND], 1);
        assert_eq!(machine.result_words[COMBO_CHARACTER_LOTTERY_SOURCE], 0);
        assert_eq!(machine.result_words[COMBO_CHARACTER_LOTTERY_FIRST_ROLE], 0);
        assert_eq!(
            machine.result_words[COMBO_CHARACTER_LOTTERY_FIRST_DIALOGUE_ID],
            33
        );
        assert_eq!(
            machine.result_words[COMBO_CHARACTER_LOTTERY_FIRST_CHARACTER_ID],
            303
        );
        assert_eq!(machine.result_words[COMBO_CHARACTER_LOTTERY_SECOND_ROLE], 1);
        assert_eq!(
            machine.result_words[COMBO_CHARACTER_LOTTERY_SECOND_DIALOGUE_ID],
            33
        );
        assert_eq!(
            machine.result_words[COMBO_CHARACTER_LOTTERY_SECOND_CHARACTER_ID],
            404
        );
        assert_eq!(
            machine.result_words[COMBO_CHARACTER_LOTTERY_COMMON_REMAINING],
            2
        );
        assert_eq!(
            machine.result_words[COMBO_CHARACTER_LOTTERY_FIXED_REMAINING],
            0
        );

        assert_eq!(machine.draw(), 2);
        assert_eq!(machine.result_buffer_ptr(), pointer);
        assert_eq!(machine.result_words[COMBO_CHARACTER_LOTTERY_SOURCE], 1);
        assert_eq!(machine.result_words[COMBO_CHARACTER_LOTTERY_FIRST_ROLE], 2);
        assert_eq!(
            machine.result_words[COMBO_CHARACTER_LOTTERY_FIRST_CHARACTER_ID],
            101
        );
        assert_eq!(machine.result_words[COMBO_CHARACTER_LOTTERY_SECOND_ROLE], 3);
        assert_eq!(
            machine.result_words[COMBO_CHARACTER_LOTTERY_SECOND_CHARACTER_ID],
            202
        );
    }

    #[test]
    fn lottery_boundary_can_require_exact_host_random_indices() {
        let mut machine = lottery(&COMMON_WORDS, &FIXED_WORDS);
        machine.load_random_indices(&[]);

        assert!(!machine.refresh());
        assert_eq!(
            machine.result_words[COMBO_CHARACTER_LOTTERY_FAILURE],
            WasmComboCharacterLotteryFailure::RandomIndexOutOfRange as i64
        );
        assert_eq!(
            machine.result_words[COMBO_CHARACTER_LOTTERY_ERROR_UPPER_EXCLUSIVE],
            2
        );
        assert_eq!(
            machine.result_words[COMBO_CHARACTER_LOTTERY_ERROR_ACTUAL],
            2
        );

        machine.use_seeded_random();
        assert!(machine.refresh());
    }

    #[test]
    fn lottery_boundary_encodes_partial_and_refreshed_errors_without_js_objects() {
        let call_only = [1, 11, 101, 1_001];
        let mut partial = lottery(&call_only, &[]);
        assert_eq!(partial.draw_with_shortage_refresh(false), 3);
        assert_eq!(
            partial.result_words[COMBO_CHARACTER_LOTTERY_FAILURE],
            WasmComboCharacterLotteryFailure::ResponseUnavailable as i64
        );
        assert_eq!(
            partial.result_words[COMBO_CHARACTER_LOTTERY_IGNORED_CHARACTER_ID],
            101
        );
        assert_eq!(
            partial.result_words[COMBO_CHARACTER_LOTTERY_FIRST_CHARACTER_ID],
            101
        );
        assert_eq!(
            partial.result_words[COMBO_CHARACTER_LOTTERY_SECOND_CHARACTER_ID],
            -1
        );

        let response_only = [2, 22, 202, 2_002];
        let mut error = lottery(&response_only, &[]);
        assert_eq!(error.draw(), 4);
        assert_eq!(
            error.result_words[COMBO_CHARACTER_LOTTERY_FAILURE],
            WasmComboCharacterLotteryFailure::CommonVoiceUnavailableAfterRefresh as i64
        );
        assert_eq!(
            error.result_words[COMBO_CHARACTER_LOTTERY_FAILURE_ROLE],
            ComboCharacterDialogueRole::Call as i64
        );
        assert_eq!(
            error.result_words[COMBO_CHARACTER_LOTTERY_IGNORED_CHARACTER_ID],
            -1
        );
    }

    #[test]
    fn lottery_seed_produces_identical_persistent_draw_sequences() {
        let common_words = [
            1, 11, 101, 1_001, 2, 22, 202, 2_002, 1, 33, 303, 3_003, 2, 44, 404, 4_004,
        ];
        let fixed_words = [33, 303, 3_003, 404, 4_004, 55, 505, 5_005, 606, 6_006];
        let mut first =
            WasmComboCharacterLotteryMachine::from_flat_words(&common_words, &fixed_words, 9001)
                .unwrap();
        let mut second =
            WasmComboCharacterLotteryMachine::from_flat_words(&common_words, &fixed_words, 9001)
                .unwrap();

        for _ in 0..12 {
            assert_eq!(first.draw(), second.draw());
            assert_eq!(first.result_words, second.result_words);
        }
    }

    #[test]
    fn lottery_boundary_rejects_misaligned_rows_and_unknown_roles() {
        assert!(
            WasmComboCharacterLotteryMachine::from_flat_words(&[0, 1, 2], &[], 1)
                .unwrap_err()
                .contains("common input length")
        );
        assert!(
            WasmComboCharacterLotteryMachine::from_flat_words(&[], &[1, 2], 1)
                .unwrap_err()
                .contains("fixed input length")
        );
        assert_eq!(
            WasmComboCharacterLotteryMachine::from_flat_words(&[9, 1, 2, 3], &[], 1).unwrap_err(),
            "invalid combo-character dialogue role: 9"
        );
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
