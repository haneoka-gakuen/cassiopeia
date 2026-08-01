use serde::{Deserialize, Serialize};
use std::fmt;

use crate::judgement::Judgement;

pub const LIFE_BASE: u32 = 1_000;
pub const LIFE_DANGER: u32 = 300;
pub const MAX_NORMALIZED_SCORE: u32 = 1_000_000;

const FACTOR_SCALE: u64 = 1_000;
const WEIGHT_SCALE: u64 = 1_000;
const BONUS_SCALE: u64 = 10_000;

/// Stable operation values used by score weighting and input routing.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteOperateType {
    None,
    Normal,
    SlideBegin,
    SlideConnection,
    SlideEnd,
    Flick,
    SlideBeginFlick,
    SlideEndFlick,
    Trace,
    SlideBeginTrace,
    SlideEndTrace,
    SlideConnectionTrace,
    HiddenSlideBegin,
    HiddenSlideEnd,
    GuideBegin,
    GuideBeginNormal,
    GuideBeginFlick,
    GuideEnd,
    GuideBeginTrace,
    GuideEndTrace,
    Combo,
    ComboSkip,
    Hidden,
    InvalidHidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ComboAction {
    Ignore,
    Increment,
    Break,
}

/// Fixed-point contribution numerator.
///
/// One unbonused Perfect normal note is `10_000_000_000` units. Keeping the
/// factor, note weight, and combo multiplier scales multiplied together means
/// normalization performs only one rounded integer division.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScoreUnits(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoreError {
    Overflow,
}

impl fmt::Display for ScoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("score calculation overflowed")
    }
}

impl std::error::Error for ScoreError {}

impl ScoreUnits {
    pub const ZERO: Self = Self(0);

    pub fn checked_add(self, other: Self) -> Result<Self, ScoreError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(ScoreError::Overflow)
    }
}

/// Returns the judgement multiplier in thousandths.
pub const fn judgement_factor_milli(judgement: Judgement) -> u16 {
    match judgement {
        Judgement::Just => 2_300,
        Judgement::Perfect => 1_000,
        Judgement::Great => 800,
        Judgement::Good => 500,
        Judgement::None | Judgement::Wait | Judgement::Miss | Judgement::Bad | Judgement::Pass => 0,
    }
}

/// Returns the note weight in thousandths.
pub const fn note_weight_milli(note_type: NoteOperateType) -> u16 {
    match note_type {
        NoteOperateType::SlideConnection
        | NoteOperateType::Trace
        | NoteOperateType::SlideBeginTrace
        | NoteOperateType::SlideEndTrace
        | NoteOperateType::SlideConnectionTrace
        | NoteOperateType::GuideBeginTrace
        | NoteOperateType::GuideEndTrace
        | NoteOperateType::Combo => 100,
        _ => 1_000,
    }
}

/// Combo bonus in basis points: 100 means one percent.
pub const fn combo_bonus_basis_points(combo: u32) -> u16 {
    let first_unbounded = combo / 10;
    let first = if first_unbounded > 10 {
        10
    } else {
        first_unbounded
    };
    let second_unbounded = combo.saturating_sub(100).saturating_div(10);
    let second = if second_unbounded > 40 {
        40
    } else {
        second_unbounded
    };
    let bonus = first * 100 + second * 50;
    if bonus > 3_000 { 3_000 } else { bonus as u16 }
}

pub fn contribution(judgement: Judgement, note_type: NoteOperateType, combo: u32) -> ScoreUnits {
    let factor = u64::from(judgement_factor_milli(judgement));
    let weight = u64::from(note_weight_milli(note_type));
    let multiplier = BONUS_SCALE + u64::from(combo_bonus_basis_points(combo));
    ScoreUnits(factor * weight * multiplier)
}

pub fn perfect_ceiling(
    note_types: impl IntoIterator<Item = NoteOperateType>,
) -> Result<ScoreUnits, ScoreError> {
    note_types
        .into_iter()
        .enumerate()
        .try_fold(ScoreUnits::ZERO, |sum, (index, note_type)| {
            let combo = u32::try_from(index)
                .map_err(|_| ScoreError::Overflow)?
                .checked_add(1)
                .ok_or(ScoreError::Overflow)?;
            let weight = u64::from(note_weight_milli(note_type));
            let multiplier = BONUS_SCALE + u64::from(combo_bonus_basis_points(combo));
            sum.checked_add(ScoreUnits(FACTOR_SCALE * weight * multiplier))
        })
}

/// Normalizes accumulated contribution with non-negative `Math.round` rules.
pub fn normalize_score(sum: ScoreUnits, ceiling: ScoreUnits) -> Result<u32, ScoreError> {
    if ceiling.0 == 0 {
        return Ok(0);
    }
    let numerator = u128::from(MAX_NORMALIZED_SCORE) * u128::from(sum.0);
    let denominator = u128::from(ceiling.0);
    let rounded = (numerator + denominator / 2) / denominator;
    u32::try_from(rounded).map_err(|_| ScoreError::Overflow)
}

pub const fn life_damage(judgement: Judgement) -> u32 {
    match judgement {
        Judgement::Bad => 50,
        Judgement::Miss => 100,
        _ => 0,
    }
}

pub const fn preserves_combo(judgement: Judgement) -> bool {
    !matches!(combo_action(judgement), ComboAction::Break)
}

pub const fn increments_combo(judgement: Judgement) -> bool {
    matches!(combo_action(judgement), ComboAction::Increment)
}

pub const fn breaks_combo(judgement: Judgement) -> bool {
    matches!(combo_action(judgement), ComboAction::Break)
}

pub const fn combo_action(judgement: Judgement) -> ComboAction {
    match judgement {
        Judgement::None | Judgement::Wait | Judgement::Pass => ComboAction::Ignore,
        Judgement::Perfect | Judgement::Great | Judgement::Good | Judgement::Just => {
            ComboAction::Increment
        }
        Judgement::Bad | Judgement::Miss => ComboAction::Break,
    }
}

pub const fn contribution_scale() -> u64 {
    FACTOR_SCALE * WEIGHT_SCALE * BONUS_SCALE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combo_bonus_uses_the_two_bounded_steps() {
        assert_eq!(combo_bonus_basis_points(9), 0);
        assert_eq!(combo_bonus_basis_points(10), 100);
        assert_eq!(combo_bonus_basis_points(100), 1_000);
        assert_eq!(combo_bonus_basis_points(110), 1_050);
        assert_eq!(combo_bonus_basis_points(500), 3_000);
        assert_eq!(combo_bonus_basis_points(u32::MAX), 3_000);
    }

    #[test]
    fn contribution_is_exact_fixed_point() {
        assert_eq!(contribution_scale(), 10_000_000_000);
        assert_eq!(
            contribution(Judgement::Perfect, NoteOperateType::Normal, 0),
            ScoreUnits(contribution_scale())
        );
        assert_eq!(
            contribution(Judgement::Just, NoteOperateType::Normal, 0),
            ScoreUnits(23_000_000_000)
        );
        assert_eq!(
            contribution(Judgement::Perfect, NoteOperateType::Trace, 10),
            ScoreUnits(1_010_000_000)
        );
    }

    #[test]
    fn perfect_ceiling_uses_post_judgement_combo_numbers() {
        let types = [NoteOperateType::Normal; 10];
        let ceiling = perfect_ceiling(types).unwrap();
        assert_eq!(ceiling, ScoreUnits(100_100_000_000));
        assert_eq!(
            normalize_score(ceiling, ceiling).unwrap(),
            MAX_NORMALIZED_SCORE
        );
    }

    #[test]
    fn normalization_rounds_non_negative_ties_up() {
        assert_eq!(
            normalize_score(ScoreUnits(1), ScoreUnits(3)).unwrap(),
            333_333
        );
        assert_eq!(
            normalize_score(ScoreUnits(2), ScoreUnits(3)).unwrap(),
            666_667
        );
        assert_eq!(
            normalize_score(ScoreUnits(1), ScoreUnits(2)).unwrap(),
            500_000
        );
        assert_eq!(
            normalize_score(ScoreUnits::ZERO, ScoreUnits::ZERO).unwrap(),
            0
        );
    }

    #[test]
    fn life_and_combo_rules_are_explicit() {
        assert_eq!(life_damage(Judgement::Bad), 50);
        assert_eq!(life_damage(Judgement::Miss), 100);
        assert_eq!(combo_action(Judgement::Wait), ComboAction::Ignore);
        assert_eq!(combo_action(Judgement::Pass), ComboAction::Ignore);
        assert_eq!(combo_action(Judgement::Good), ComboAction::Increment);
        assert_eq!(combo_action(Judgement::Bad), ComboAction::Break);
        assert!(preserves_combo(Judgement::Pass));
        assert!(preserves_combo(Judgement::Great));
        assert!(preserves_combo(Judgement::Good));
        assert!(increments_combo(Judgement::Good));
        assert!(!increments_combo(Judgement::Pass));
        assert!(breaks_combo(Judgement::Miss));
        assert!(!breaks_combo(Judgement::Wait));
    }
}
