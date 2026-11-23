//! Typed difficulty and performance results matching the Ez2Lazer calculator model.

use crate::beatmap::GameMode;
use crate::mods::Mod;

/// Common fields returned by every ruleset difficulty calculator.
pub trait DifficultyAttributes {
    fn ruleset(&self) -> GameMode;
    fn mods(&self) -> &[Mod];
    fn star_rating(&self) -> f64;
    fn max_combo(&self) -> i32;
}

/// osu!standard difficulty calculation output.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OsuDifficultyAttributes {
    pub mods: Vec<Mod>,
    pub star_rating: f64,
    pub max_combo: i32,
    pub aim_difficulty: f64,
    pub aim_difficult_slider_count: f64,
    pub speed_difficulty: f64,
    pub speed_note_count: f64,
    pub flashlight_difficulty: Option<f64>,
    pub reading_difficulty: f64,
    pub slider_factor: f64,
    pub aim_top_weighted_slider_factor: f64,
    pub speed_top_weighted_slider_factor: f64,
    pub aim_difficult_strain_count: f64,
    pub speed_difficult_strain_count: f64,
    pub reading_difficult_note_count: f64,
    pub nested_score_per_object: f64,
    pub legacy_score_base_multiplier: f64,
    pub maximum_legacy_combo_score: f64,
    pub hit_circle_count: i32,
    pub slider_count: i32,
    pub spinner_count: i32,
}

/// osu!taiko difficulty calculation output.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaikoDifficultyAttributes {
    pub mods: Vec<Mod>,
    pub star_rating: f64,
    pub max_combo: i32,
    pub mechanical_difficulty: f64,
    pub rhythm_difficulty: f64,
    pub reading_difficulty: f64,
    pub colour_difficulty: f64,
    pub stamina_difficulty: f64,
    pub mono_stamina_factor: f64,
    pub consistency_factor: f64,
    pub stamina_top_strains: f64,
}

/// osu!catch difficulty calculation output.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CatchDifficultyAttributes {
    pub mods: Vec<Mod>,
    pub star_rating: f64,
    pub max_combo: i32,
}

/// osu!mania difficulty calculation output.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ManiaDifficultyAttributes {
    pub mods: Vec<Mod>,
    pub star_rating: f64,
    pub max_combo: i32,
}

impl DifficultyAttributes for OsuDifficultyAttributes {
    fn ruleset(&self) -> GameMode {
        GameMode::Standard
    }
    fn mods(&self) -> &[Mod] {
        &self.mods
    }
    fn star_rating(&self) -> f64 {
        self.star_rating
    }
    fn max_combo(&self) -> i32 {
        self.max_combo
    }
}

impl DifficultyAttributes for TaikoDifficultyAttributes {
    fn ruleset(&self) -> GameMode {
        GameMode::Taiko
    }
    fn mods(&self) -> &[Mod] {
        &self.mods
    }
    fn star_rating(&self) -> f64 {
        self.star_rating
    }
    fn max_combo(&self) -> i32 {
        self.max_combo
    }
}

impl DifficultyAttributes for CatchDifficultyAttributes {
    fn ruleset(&self) -> GameMode {
        GameMode::Catch
    }
    fn mods(&self) -> &[Mod] {
        &self.mods
    }
    fn star_rating(&self) -> f64 {
        self.star_rating
    }
    fn max_combo(&self) -> i32 {
        self.max_combo
    }
}

impl DifficultyAttributes for ManiaDifficultyAttributes {
    fn ruleset(&self) -> GameMode {
        GameMode::Mania
    }
    fn mods(&self) -> &[Mod] {
        &self.mods
    }
    fn star_rating(&self) -> f64 {
        self.star_rating
    }
    fn max_combo(&self) -> i32 {
        self.max_combo
    }
}

/// Difficulty attributes returned by a ruleset selected at runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum AnyDifficultyAttributes {
    Osu(OsuDifficultyAttributes),
    Taiko(TaikoDifficultyAttributes),
    Catch(CatchDifficultyAttributes),
    Mania(ManiaDifficultyAttributes),
}

impl DifficultyAttributes for AnyDifficultyAttributes {
    fn ruleset(&self) -> GameMode {
        match self {
            Self::Osu(value) => value.ruleset(),
            Self::Taiko(value) => value.ruleset(),
            Self::Catch(value) => value.ruleset(),
            Self::Mania(value) => value.ruleset(),
        }
    }

    fn mods(&self) -> &[Mod] {
        match self {
            Self::Osu(value) => value.mods(),
            Self::Taiko(value) => value.mods(),
            Self::Catch(value) => value.mods(),
            Self::Mania(value) => value.mods(),
        }
    }

    fn star_rating(&self) -> f64 {
        match self {
            Self::Osu(value) => value.star_rating(),
            Self::Taiko(value) => value.star_rating(),
            Self::Catch(value) => value.star_rating(),
            Self::Mania(value) => value.star_rating(),
        }
    }

    fn max_combo(&self) -> i32 {
        match self {
            Self::Osu(value) => value.max_combo(),
            Self::Taiko(value) => value.max_combo(),
            Self::Catch(value) => value.max_combo(),
            Self::Mania(value) => value.max_combo(),
        }
    }
}

/// Common score performance result.
pub trait PerformanceAttributes {
    fn total(&self) -> f64;
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OsuPerformanceAttributes {
    pub total: f64,
    pub aim: f64,
    pub speed: f64,
    pub accuracy: f64,
    pub flashlight: f64,
    pub reading: f64,
    pub effective_miss_count: f64,
    pub speed_deviation: Option<f64>,
    pub combo_based_estimated_miss_count: f64,
    pub score_based_estimated_miss_count: Option<f64>,
    pub aim_estimated_slider_breaks: f64,
    pub speed_estimated_slider_breaks: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaikoPerformanceAttributes {
    pub total: f64,
    pub difficulty: f64,
    pub accuracy: f64,
    pub estimated_unstable_rate: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CatchPerformanceAttributes {
    pub total: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ManiaPerformanceAttributes {
    pub total: f64,
    pub difficulty: f64,
}

impl PerformanceAttributes for OsuPerformanceAttributes {
    fn total(&self) -> f64 {
        self.total
    }
}

impl PerformanceAttributes for TaikoPerformanceAttributes {
    fn total(&self) -> f64 {
        self.total
    }
}

impl PerformanceAttributes for CatchPerformanceAttributes {
    fn total(&self) -> f64 {
        self.total
    }
}

impl PerformanceAttributes for ManiaPerformanceAttributes {
    fn total(&self) -> f64 {
        self.total
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnyPerformanceAttributes {
    Osu(OsuPerformanceAttributes),
    Taiko(TaikoPerformanceAttributes),
    Catch(CatchPerformanceAttributes),
    Mania(ManiaPerformanceAttributes),
}

impl PerformanceAttributes for AnyPerformanceAttributes {
    fn total(&self) -> f64 {
        match self {
            Self::Osu(value) => value.total(),
            Self::Taiko(value) => value.total(),
            Self::Catch(value) => value.total(),
            Self::Mania(value) => value.total(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_specific_attributes_expose_common_calculator_fields() {
        let attributes = ManiaDifficultyAttributes {
            star_rating: 4.25,
            max_combo: 123,
            ..Default::default()
        };
        assert_eq!(attributes.ruleset(), GameMode::Mania);
        assert_eq!(attributes.star_rating(), 4.25);
        assert_eq!(attributes.max_combo(), 123);
    }

    #[test]
    fn performance_result_exposes_total_and_mode_breakdown() {
        let attributes = OsuPerformanceAttributes {
            total: 250.0,
            aim: 100.0,
            ..Default::default()
        };
        assert_eq!(attributes.total(), 250.0);
        assert_eq!(attributes.aim, 100.0);
    }
}
