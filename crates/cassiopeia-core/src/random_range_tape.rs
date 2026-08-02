use std::fmt;

/// Number of `i32` words in one recorded range call.
///
/// Entries are `[min_inclusive, max_exclusive, result]`.
pub const RANDOM_RANGE_TAPE_STRIDE: usize = 3;

/// A fixed-storage sequence of exact, host-recorded integer range results.
///
/// This is a replay primitive, not a seeded random-number generator. Calls are
/// consumed in order and must reproduce both recorded bounds exactly. Storage
/// is allocated once during construction; reads, resets, and cursor restores
/// do not allocate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RandomRangeTape {
    words: Box<[i32]>,
    cursor: usize,
}

impl RandomRangeTape {
    pub fn from_words(words: &[i32]) -> Result<Self, RandomRangeTapeError> {
        if !words.len().is_multiple_of(RANDOM_RANGE_TAPE_STRIDE) {
            return Err(RandomRangeTapeError::InvalidWordCount { words: words.len() });
        }

        for (index, entry) in words.chunks_exact(RANDOM_RANGE_TAPE_STRIDE).enumerate() {
            let min_inclusive = entry[0];
            let max_exclusive = entry[1];
            let result = entry[2];
            if min_inclusive >= max_exclusive {
                return Err(RandomRangeTapeError::InvalidRecordedBounds {
                    index,
                    min_inclusive,
                    max_exclusive,
                });
            }
            if result < min_inclusive || result >= max_exclusive {
                return Err(RandomRangeTapeError::RecordedResultOutOfRange {
                    index,
                    min_inclusive,
                    max_exclusive,
                    result,
                });
            }
        }

        Ok(Self {
            words: words.into(),
            cursor: 0,
        })
    }

    /// Consumes the next result after checking the request against its entry.
    ///
    /// Any failure leaves the cursor unchanged.
    pub fn range_int(
        &mut self,
        min_inclusive: i32,
        max_exclusive: i32,
    ) -> Result<i32, RandomRangeTapeError> {
        if min_inclusive >= max_exclusive {
            return Err(RandomRangeTapeError::InvalidRequestBounds {
                min_inclusive,
                max_exclusive,
            });
        }

        let index = self.cursor;
        let Some(entry) = self
            .words
            .get(index * RANDOM_RANGE_TAPE_STRIDE..(index + 1) * RANDOM_RANGE_TAPE_STRIDE)
        else {
            return Err(RandomRangeTapeError::Exhausted { cursor: index });
        };

        let recorded_min = entry[0];
        let recorded_max = entry[1];
        let result = entry[2];
        if recorded_min != min_inclusive || recorded_max != max_exclusive {
            return Err(RandomRangeTapeError::RequestMismatch {
                index,
                recorded_min_inclusive: recorded_min,
                recorded_max_exclusive: recorded_max,
                requested_min_inclusive: min_inclusive,
                requested_max_exclusive: max_exclusive,
            });
        }

        // Construction validates every entry. Keep this check at the
        // consumption boundary as part of the replay contract too.
        if result < min_inclusive || result >= max_exclusive {
            return Err(RandomRangeTapeError::RecordedResultOutOfRange {
                index,
                min_inclusive,
                max_exclusive,
                result,
            });
        }

        self.cursor += 1;
        Ok(result)
    }

    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn len(&self) -> usize {
        self.words.len() / RANDOM_RANGE_TAPE_STRIDE
    }

    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    pub fn remaining(&self) -> usize {
        self.len() - self.cursor
    }

    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    /// Restores a position in the same immutable tape.
    pub fn restore(&mut self, cursor: usize) -> Result<(), RandomRangeTapeError> {
        let len = self.len();
        if cursor > len {
            return Err(RandomRangeTapeError::CursorOutOfRange { cursor, len });
        }
        self.cursor = cursor;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RandomRangeTapeError {
    InvalidWordCount {
        words: usize,
    },
    InvalidRecordedBounds {
        index: usize,
        min_inclusive: i32,
        max_exclusive: i32,
    },
    RecordedResultOutOfRange {
        index: usize,
        min_inclusive: i32,
        max_exclusive: i32,
        result: i32,
    },
    InvalidRequestBounds {
        min_inclusive: i32,
        max_exclusive: i32,
    },
    Exhausted {
        cursor: usize,
    },
    RequestMismatch {
        index: usize,
        recorded_min_inclusive: i32,
        recorded_max_exclusive: i32,
        requested_min_inclusive: i32,
        requested_max_exclusive: i32,
    },
    CursorOutOfRange {
        cursor: usize,
        len: usize,
    },
}

impl fmt::Display for RandomRangeTapeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWordCount { words } => write!(
                formatter,
                "random-range tape has {words} words; expected a multiple of {RANDOM_RANGE_TAPE_STRIDE}"
            ),
            Self::InvalidRecordedBounds {
                index,
                min_inclusive,
                max_exclusive,
            } => write!(
                formatter,
                "random-range tape entry {index} has invalid bounds [{min_inclusive}, {max_exclusive})"
            ),
            Self::RecordedResultOutOfRange {
                index,
                min_inclusive,
                max_exclusive,
                result,
            } => write!(
                formatter,
                "random-range tape entry {index} recorded {result}; expected [{min_inclusive}, {max_exclusive})"
            ),
            Self::InvalidRequestBounds {
                min_inclusive,
                max_exclusive,
            } => write!(
                formatter,
                "random-range request has invalid bounds [{min_inclusive}, {max_exclusive})"
            ),
            Self::Exhausted { cursor } => {
                write!(
                    formatter,
                    "random-range tape is exhausted at cursor {cursor}"
                )
            }
            Self::RequestMismatch {
                index,
                recorded_min_inclusive,
                recorded_max_exclusive,
                requested_min_inclusive,
                requested_max_exclusive,
            } => write!(
                formatter,
                "random-range tape entry {index} expects [{recorded_min_inclusive}, {recorded_max_exclusive}); requested [{requested_min_inclusive}, {requested_max_exclusive})"
            ),
            Self::CursorOutOfRange { cursor, len } => write!(
                formatter,
                "random-range tape cursor {cursor} is out of range for {len} entries"
            ),
        }
    }
}

impl std::error::Error for RandomRangeTapeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consumes_signed_ranges_in_order_without_moving_storage() {
        let mut tape = RandomRangeTape::from_words(&[-5, 5, -2, 0, 4, 3]).unwrap();
        let storage = tape.words.as_ptr();

        assert_eq!(tape.len(), 2);
        assert_eq!(tape.remaining(), 2);
        assert_eq!(tape.range_int(-5, 5), Ok(-2));
        assert_eq!(tape.cursor(), 1);
        assert_eq!(tape.range_int(0, 4), Ok(3));
        assert_eq!(tape.cursor(), 2);
        assert_eq!(tape.remaining(), 0);
        assert_eq!(tape.words.as_ptr(), storage);
    }

    #[test]
    fn accepts_full_width_i32_bounds() {
        let mut tape = RandomRangeTape::from_words(&[i32::MIN, i32::MAX, i32::MAX - 1]).unwrap();
        assert_eq!(tape.range_int(i32::MIN, i32::MAX), Ok(i32::MAX - 1));
    }

    #[test]
    fn rejects_misaligned_words_and_invalid_recorded_bounds() {
        assert_eq!(
            RandomRangeTape::from_words(&[0, 1]).unwrap_err(),
            RandomRangeTapeError::InvalidWordCount { words: 2 }
        );
        assert_eq!(
            RandomRangeTape::from_words(&[2, 2, 2]).unwrap_err(),
            RandomRangeTapeError::InvalidRecordedBounds {
                index: 0,
                min_inclusive: 2,
                max_exclusive: 2,
            }
        );
        assert_eq!(
            RandomRangeTape::from_words(&[5, -5, 0]).unwrap_err(),
            RandomRangeTapeError::InvalidRecordedBounds {
                index: 0,
                min_inclusive: 5,
                max_exclusive: -5,
            }
        );
    }

    #[test]
    fn rejects_each_out_of_range_recorded_result() {
        assert_eq!(
            RandomRangeTape::from_words(&[0, 3, -1]).unwrap_err(),
            RandomRangeTapeError::RecordedResultOutOfRange {
                index: 0,
                min_inclusive: 0,
                max_exclusive: 3,
                result: -1,
            }
        );
        assert_eq!(
            RandomRangeTape::from_words(&[0, 3, 3]).unwrap_err(),
            RandomRangeTapeError::RecordedResultOutOfRange {
                index: 0,
                min_inclusive: 0,
                max_exclusive: 3,
                result: 3,
            }
        );
    }

    #[test]
    fn request_failures_never_advance_the_cursor() {
        let mut tape = RandomRangeTape::from_words(&[0, 4, 2]).unwrap();

        assert_eq!(
            tape.range_int(4, 4),
            Err(RandomRangeTapeError::InvalidRequestBounds {
                min_inclusive: 4,
                max_exclusive: 4,
            })
        );
        assert_eq!(tape.cursor(), 0);
        assert_eq!(
            tape.range_int(0, 5),
            Err(RandomRangeTapeError::RequestMismatch {
                index: 0,
                recorded_min_inclusive: 0,
                recorded_max_exclusive: 4,
                requested_min_inclusive: 0,
                requested_max_exclusive: 5,
            })
        );
        assert_eq!(tape.cursor(), 0);
        assert_eq!(tape.range_int(0, 4), Ok(2));
        assert_eq!(
            tape.range_int(0, 1),
            Err(RandomRangeTapeError::Exhausted { cursor: 1 })
        );
        assert_eq!(tape.cursor(), 1);
    }

    #[test]
    fn reset_and_restore_replay_existing_storage() {
        let mut tape = RandomRangeTape::from_words(&[0, 2, 1, 10, 20, 15]).unwrap();
        assert_eq!(tape.range_int(0, 2), Ok(1));
        assert_eq!(tape.range_int(10, 20), Ok(15));

        tape.restore(1).unwrap();
        assert_eq!(tape.range_int(10, 20), Ok(15));
        tape.reset();
        assert_eq!(tape.range_int(0, 2), Ok(1));
    }

    #[test]
    fn invalid_restore_preserves_the_current_cursor() {
        let mut tape = RandomRangeTape::from_words(&[0, 2, 1]).unwrap();
        tape.restore(1).unwrap();
        assert_eq!(
            tape.restore(2),
            Err(RandomRangeTapeError::CursorOutOfRange { cursor: 2, len: 1 })
        );
        assert_eq!(tape.cursor(), 1);
    }

    #[test]
    fn empty_tape_is_valid_and_immediately_exhausted() {
        let mut tape = RandomRangeTape::from_words(&[]).unwrap();
        assert!(tape.is_empty());
        assert_eq!(tape.remaining(), 0);
        assert_eq!(
            tape.range_int(0, 1),
            Err(RandomRangeTapeError::Exhausted { cursor: 0 })
        );
        tape.restore(0).unwrap();
    }
}
