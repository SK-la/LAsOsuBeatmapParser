//! Strongly typed gameplay modifiers shared by difficulty and performance calculators.

/// Difficulty overrides applied by Ez2Lazer's `ModDifficultyAdjust`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DifficultyAdjustment {
    pub hp_drain_rate: Option<f64>,
    pub circle_size: Option<f64>,
    pub overall_difficulty: Option<f64>,
    pub approach_rate: Option<f64>,
    pub slider_multiplier: Option<f64>,
    pub slider_tick_rate: Option<f64>,
}

/// A configured osu! gameplay modifier.
///
/// Variants map to the corresponding Ez2Lazer mod acronyms. Mod settings are typed values; callers
/// do not need to serialize mod objects to pass them to a calculator.
#[derive(Clone, Debug, PartialEq)]
pub enum Mod {
    NoFail,
    Easy,
    TouchDevice,
    Hidden,
    HardRock,
    SuddenDeath,
    Perfect,
    DoubleTime,
    Nightcore,
    HalfTime,
    Daycore,
    Flashlight,
    Relax,
    Autopilot,
    SpunOut,
    Autoplay,
    ScoreV2,
    Mirror,
    Classic,
    RateAdjust {
        rate: f64,
    },
    DifficultyAdjust(DifficultyAdjustment),
    KeyCount {
        keys: u8,
    },
    SplitStages,
    /// A future or ruleset-specific mod that has no Rust implementation yet.
    Other(String),
}

impl Mod {
    /// The short identifier used by Ez2Lazer and osu! web.
    pub fn acronym(&self) -> &str {
        match self {
            Self::NoFail => "NF",
            Self::Easy => "EZ",
            Self::TouchDevice => "TD",
            Self::Hidden => "HD",
            Self::HardRock => "HR",
            Self::SuddenDeath => "SD",
            Self::Perfect => "PF",
            Self::DoubleTime => "DT",
            Self::Nightcore => "NC",
            Self::HalfTime => "HT",
            Self::Daycore => "DC",
            Self::Flashlight => "FL",
            Self::Relax => "RL",
            Self::Autopilot => "AP",
            Self::SpunOut => "SO",
            Self::Autoplay => "AT",
            Self::ScoreV2 => "V2",
            Self::Mirror => "MR",
            Self::Classic => "CL",
            Self::RateAdjust { .. } => "DA",
            Self::DifficultyAdjust(_) => "DA",
            Self::KeyCount { keys } => match keys {
                1 => "1K",
                2 => "2K",
                3 => "3K",
                4 => "4K",
                5 => "5K",
                6 => "6K",
                7 => "7K",
                8 => "8K",
                9 => "9K",
                _ => "K",
            },
            Self::SplitStages => "ST",
            Self::Other(acronym) => acronym,
        }
    }

    /// Returns the fixed rate multiplier contributed by this modifier, if any.
    pub fn rate_multiplier(&self) -> Option<f64> {
        match self {
            Self::DoubleTime | Self::Nightcore => Some(1.5),
            Self::HalfTime | Self::Daycore => Some(0.75),
            Self::RateAdjust { rate } => Some(*rate),
            _ => None,
        }
    }
}

/// Calculates the playback rate using the same multiplicative rule as `ModUtils.CalculateRateWithMods`.
pub fn calculate_rate_with_mods(mods: &[Mod]) -> Result<f64, ModError> {
    let mut rate = 1.0;

    for modifier in mods {
        if let Some(multiplier) = modifier.rate_multiplier() {
            if !multiplier.is_finite() || multiplier <= 0.0 {
                return Err(ModError::InvalidRate(multiplier));
            }
            rate *= multiplier;
        }
    }

    if !rate.is_finite() || rate <= 0.0 {
        return Err(ModError::InvalidRate(rate));
    }

    Ok(rate)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModError {
    InvalidRate(f64),
}

impl std::fmt::Display for ModError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRate(rate) => {
                write!(f, "mod rate must be finite and positive, got {rate}")
            }
        }
    }
}

impl std::error::Error for ModError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_modifiers_multiply_in_sequence() {
        assert_eq!(calculate_rate_with_mods(&[]).unwrap(), 1.0);
        assert_eq!(calculate_rate_with_mods(&[Mod::DoubleTime]).unwrap(), 1.5);
        let compounded =
            calculate_rate_with_mods(&[Mod::DoubleTime, Mod::RateAdjust { rate: 1.2 }]).unwrap();
        assert!((compounded - 1.8).abs() < 1e-12);
        assert_eq!(calculate_rate_with_mods(&[Mod::HalfTime]).unwrap(), 0.75);
    }

    #[test]
    fn invalid_rate_modifiers_are_rejected() {
        assert!(calculate_rate_with_mods(&[Mod::RateAdjust { rate: 0.0 }]).is_err());
        assert!(calculate_rate_with_mods(&[Mod::RateAdjust { rate: f64::NAN }]).is_err());
    }

    #[test]
    fn mod_acronyms_match_lazer_conventions() {
        assert_eq!(Mod::HardRock.acronym(), "HR");
        assert_eq!(Mod::KeyCount { keys: 7 }.acronym(), "7K");
    }
}
