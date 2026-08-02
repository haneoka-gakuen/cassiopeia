use haneoka_cassiopeia_core::{RANDOM_RANGE_TAPE_STRIDE, RandomRangeTape, RandomRangeTapeError};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::prelude::*;

pub(crate) type SharedRandomRangeTape = Rc<RefCell<RandomRangeTape>>;

/// Exact replay of host-recorded integer range calls.
///
/// Input is an `Int32Array` of `[minInclusive, maxExclusive, result]` entries.
/// This class intentionally exposes no seed API and makes no claim about a
/// particular random-number generator.
#[derive(Debug)]
#[wasm_bindgen(js_name = CassiopeiaRandomRangeTape)]
pub struct WasmRandomRangeTape {
    tape: SharedRandomRangeTape,
}

#[wasm_bindgen(js_class = CassiopeiaRandomRangeTape)]
impl WasmRandomRangeTape {
    #[wasm_bindgen(constructor)]
    pub fn new(words: &[i32]) -> Result<WasmRandomRangeTape, JsError> {
        Self::from_words(words).map_err(super::js_error)
    }

    #[wasm_bindgen(js_name = rangeInt)]
    pub fn range_int(&self, min_inclusive: i32, max_exclusive: i32) -> Result<i32, JsError> {
        self.range_int_inner(min_inclusive, max_exclusive)
            .map_err(super::js_error)
    }

    pub fn cursor(&self) -> usize {
        self.tape.borrow().cursor()
    }

    pub fn remaining(&self) -> usize {
        self.tape.borrow().remaining()
    }

    pub fn len(&self) -> usize {
        self.tape.borrow().len()
    }

    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.tape.borrow().is_empty()
    }

    pub fn restore(&self, cursor: usize) -> Result<(), JsError> {
        self.restore_cursor_inner(cursor).map_err(super::js_error)
    }

    /// Alias retained for hosts that prefer an explicit cursor name.
    #[wasm_bindgen(js_name = restoreCursor)]
    pub fn restore_cursor(&self, cursor: usize) -> Result<(), JsError> {
        self.restore(cursor)
    }

    pub fn reset(&self) {
        self.tape.borrow_mut().reset();
    }

    #[wasm_bindgen(js_name = inputStride)]
    pub fn input_stride(&self) -> usize {
        RANDOM_RANGE_TAPE_STRIDE
    }
}

impl WasmRandomRangeTape {
    pub(crate) fn from_words(words: &[i32]) -> Result<Self, RandomRangeTapeError> {
        Ok(Self {
            tape: Rc::new(RefCell::new(RandomRangeTape::from_words(words)?)),
        })
    }

    pub(crate) fn shared(&self) -> SharedRandomRangeTape {
        Rc::clone(&self.tape)
    }

    pub(crate) fn range_int_inner(
        &self,
        min_inclusive: i32,
        max_exclusive: i32,
    ) -> Result<i32, RandomRangeTapeError> {
        self.tape
            .borrow_mut()
            .range_int(min_inclusive, max_exclusive)
    }

    fn restore_cursor_inner(&self, cursor: usize) -> Result<(), RandomRangeTapeError> {
        self.tape.borrow_mut().restore(cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_typed_boundary_shares_one_cursor_and_restores_it() {
        let tape = WasmRandomRangeTape::from_words(&[0, 4, 2, -3, 3, -1]).unwrap();
        let shared = tape.shared();

        assert_eq!(tape.input_stride(), 3);
        assert_eq!(tape.len(), 2);
        assert_eq!(tape.range_int_inner(0, 4), Ok(2));
        assert_eq!(shared.borrow().cursor(), 1);
        assert_eq!(tape.remaining(), 1);

        tape.restore_cursor_inner(0).unwrap();
        assert_eq!(shared.borrow().cursor(), 0);
        tape.reset();
        assert_eq!(tape.cursor(), 0);
    }

    #[test]
    fn wasm_typed_boundary_preserves_cursor_after_mismatch() {
        let tape = WasmRandomRangeTape::from_words(&[0, 5, 4]).unwrap();
        assert!(matches!(
            tape.range_int_inner(0, 4),
            Err(RandomRangeTapeError::RequestMismatch { index: 0, .. })
        ));
        assert_eq!(tape.cursor(), 0);
        assert_eq!(tape.range_int_inner(0, 5), Ok(4));
    }

    #[test]
    fn wasm_typed_boundary_rejects_bad_tape_and_restore() {
        assert!(matches!(
            WasmRandomRangeTape::from_words(&[0, 1]),
            Err(RandomRangeTapeError::InvalidWordCount { words: 2 })
        ));

        let tape = WasmRandomRangeTape::from_words(&[]).unwrap();
        assert_eq!(
            tape.restore_cursor_inner(1),
            Err(RandomRangeTapeError::CursorOutOfRange { cursor: 1, len: 0 })
        );
    }
}
