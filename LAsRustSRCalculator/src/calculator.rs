//! Calculator traits following Ez2Lazer's difficulty/performance split.

use crate::attributes::{AnyDifficultyAttributes, DifficultyAttributes, PerformanceAttributes};
use crate::beatmap::{GameMode, OsuBeatmap};
use crate::mods::Mod;
use crate::score::ScoreInfo;

/// Calculates beatmap difficulty for one ruleset.
///
/// The calculator accepts the parsed raw map and applies the selected mods and ruleset conversion
/// internally, analogous to Ez2Lazer's `DifficultyCalculator.Calculate(mods)`.
pub trait DifficultyCalculator {
    type Attributes: DifficultyAttributes;

    /// Calculator revision used to invalidate values produced by older algorithm versions.
    fn version(&self) -> u32 {
        0
    }

    fn calculate(
        &self,
        beatmap: &OsuBeatmap,
        mods: &[Mod],
    ) -> Result<Self::Attributes, CalculationError>;
}

/// Calculates score performance from a score and the matching beatmap difficulty attributes.
pub trait PerformanceCalculator {
    type Difficulty: DifficultyAttributes;
    type Attributes: PerformanceAttributes;

    fn calculate(
        &self,
        score: &ScoreInfo,
        difficulty: &Self::Difficulty,
    ) -> Result<Self::Attributes, CalculationError>;
}

/// Calculate official difficulty for the beatmap's native ruleset.
pub fn calculate_difficulty(
    beatmap: &OsuBeatmap,
    mods: &[Mod],
) -> Result<AnyDifficultyAttributes, CalculationError> {
    match beatmap.mode {
        GameMode::Standard => crate::osu_converter::OsuDifficultyCalculator
            .calculate(beatmap, mods)
            .map(AnyDifficultyAttributes::Osu),
        GameMode::Taiko => crate::taiko_converter::TaikoBeatmapDifficultyCalculator
            .calculate(beatmap, mods)
            .map(AnyDifficultyAttributes::Taiko),
        GameMode::Catch => crate::catch_converter::CatchDifficultyCalculator
            .calculate(beatmap, mods)
            .map(AnyDifficultyAttributes::Catch),
        GameMode::Mania => crate::mania::ManiaDifficultyCalculator
            .calculate(beatmap, mods)
            .map(AnyDifficultyAttributes::Mania),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalculationError {
    WrongRuleset {
        expected: GameMode,
        actual: GameMode,
    },
    InvalidBeatmap(String),
    UnsupportedObject {
        ruleset: GameMode,
        object: &'static str,
    },
    InvalidMod(String),
}

impl std::fmt::Display for CalculationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongRuleset { expected, actual } => {
                write!(f, "calculator requires {expected:?}, beatmap is {actual:?}")
            }
            Self::InvalidBeatmap(message) => f.write_str(message),
            Self::UnsupportedObject { ruleset, object } => {
                write!(f, "{ruleset:?} conversion does not support {object}")
            }
            Self::InvalidMod(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for CalculationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculation_errors_identify_ruleset_and_object_contracts() {
        let wrong_mode = CalculationError::WrongRuleset {
            expected: GameMode::Mania,
            actual: GameMode::Catch,
        };
        assert!(wrong_mode.to_string().contains("Mania"));
        let unsupported = CalculationError::UnsupportedObject {
            ruleset: GameMode::Taiko,
            object: "slider",
        };
        assert!(unsupported.to_string().contains("slider"));
    }
}
