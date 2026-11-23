//! Stable skill-axis models used by the Ez2Lazer analysis APIs.
//!
//! Axis order and IDs are part of the public contract: callers can store vectors densely without
//! converting through a string-keyed map, while still resolving the same IDs used by Ez2Lazer.

use std::marker::PhantomData;

/// A named, ordered skill axis.
pub trait SkillAxis: Copy + Eq + 'static {
    /// All axes in the canonical order used by the corresponding Ez2Lazer skill system.
    const ALL: &'static [Self];

    /// Stable persistence and interop ID.
    fn id(self) -> &'static str;

    /// Human-readable English label, matching the analysis readout.
    fn display_name(self) -> &'static str;

    /// Whether this axis belongs in the system's radar chart.
    fn in_radar(self) -> bool;

    /// Returns the axis position in [`Self::ALL`].
    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|axis| *axis == self)
            .expect("SkillAxis::ALL must contain every variant")
    }

    /// Resolves a stable ID using Ez2Lazer's ordinal, case-sensitive lookup behavior.
    fn try_parse_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|axis| axis.id() == id)
    }

    /// Resolves a stable ID without regard to ASCII case.
    fn try_parse_id_ignore_ascii_case(id: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|axis| axis.id().eq_ignore_ascii_case(id))
    }

    /// Resolves a namespaced ID by its final segment, ignoring ASCII case.
    fn try_parse_qualified_id(id: &str) -> Option<Self> {
        let bare_id = id.rsplit_once('.').map_or(id, |(_, tail)| tail);
        Self::try_parse_id_ignore_ascii_case(bare_id)
    }

    /// Returns the radar axes in their canonical order.
    fn radar_axes() -> impl Iterator<Item = Self> {
        Self::ALL.iter().copied().filter(|axis| axis.in_radar())
    }
}

/// A dense skill vector in the canonical order of its axis type.
///
/// This mirrors Ez2Lazer's axis-enum plus `IReadOnlyDictionary<string, double>` usage while
/// keeping Rust-side storage contiguous and avoiding per-value string hashing.
#[derive(Clone, Debug, PartialEq)]
pub struct SkillVector<A: SkillAxis> {
    values: Box<[f64]>,
    axis: PhantomData<A>,
}

impl<A: SkillAxis> SkillVector<A> {
    /// Creates a zero-filled vector containing every axis in canonical order.
    pub fn zeros() -> Self {
        Self::from_values(vec![0.0; A::ALL.len()]).expect("zero vector has the correct axis count")
    }

    /// Creates a vector from values in [`SkillAxis::ALL`] order.
    pub fn from_values(values: Vec<f64>) -> Result<Self, SkillVectorError> {
        if values.len() != A::ALL.len() {
            return Err(SkillVectorError {
                expected: A::ALL.len(),
                actual: values.len(),
            });
        }

        Ok(Self {
            values: values.into_boxed_slice(),
            axis: PhantomData,
        })
    }

    /// Creates a vector by evaluating each axis in canonical order.
    pub fn from_fn(mut value_for: impl FnMut(A) -> f64) -> Self {
        Self::from_values(A::ALL.iter().copied().map(&mut value_for).collect())
            .expect("axis iteration always produces the canonical axis count")
    }

    /// Returns one value by its typed axis.
    pub fn get(&self, axis: A) -> f64 {
        self.values[axis.index()]
    }

    /// Sets one value by its typed axis.
    pub fn set(&mut self, axis: A, value: f64) {
        self.values[axis.index()] = value;
    }

    /// Returns one value by stable ID.
    pub fn get_by_id(&self, id: &str) -> Option<f64> {
        A::try_parse_id(id).map(|axis| self.get(axis))
    }

    /// Returns a value by a qualified or case-insensitive stable ID.
    pub fn get_by_qualified_id(&self, id: &str) -> Option<f64> {
        A::try_parse_qualified_id(id).map(|axis| self.get(axis))
    }

    /// Iterates over `(axis, value)` pairs in canonical order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (A, f64)> + '_ {
        A::ALL.iter().copied().zip(self.values.iter().copied())
    }

    /// Returns the dense values in canonical order for binary interop.
    pub fn as_slice(&self) -> &[f64] {
        &self.values
    }

    /// Returns true when each axis is present and finite.
    pub fn is_complete(&self) -> bool {
        self.values.len() == A::ALL.len() && self.values.iter().all(|value| value.is_finite())
    }
}

/// The supplied skill vector did not have one value for every axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SkillVectorError {
    pub expected: usize,
    pub actual: usize,
}

impl std::fmt::Display for SkillVectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "skill vector requires {} values, got {}",
            self.expected, self.actual
        )
    }
}

impl std::error::Error for SkillVectorError {}

macro_rules! define_skill_axis {
    ($name:ident { $($variant:ident => ($id:literal, $label:literal, $radar:literal)),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name { $($variant),+ }

        impl SkillAxis for $name {
            const ALL: &'static [Self] = &[$(Self::$variant),+];

            fn id(self) -> &'static str {
                match self { $(Self::$variant => $id),+ }
            }

            fn display_name(self) -> &'static str {
                match self { $(Self::$variant => $label),+ }
            }

            fn in_radar(self) -> bool {
                match self { $(Self::$variant => $radar),+ }
            }
        }
    };
}

define_skill_axis!(EzSunnySrSkillAxis {
    Bracket => ("bracket", "Bracket", true),
    Jack => ("jack", "Jack", true),
    Burst => ("burst", "Burst", true),
    LnHold => ("ln_hold", "LN Hold", true),
    LnRelease => ("ln_release", "LN Release", true),
    Anchor => ("anchor", "Anchor", true),
});

define_skill_axis!(EzMinaSkillAxis {
    Overall => ("overall", "Overall", false),
    Stream => ("stream", "Stream", true),
    Jumpstream => ("jumpstream", "Jumpstream", true),
    Handstream => ("handstream", "Handstream", true),
    Stamina => ("stamina", "Stamina", true),
    JackSpeed => ("jack", "Jack", true),
    Chordjack => ("chordjack", "Chordjack", true),
    Technical => ("tech", "Tech", true),
});

define_skill_axis!(EzPPPSkillAxis {
    Aim => ("aim", "Aim", false),
    JumpAim => ("jump_aim", "Jump", true),
    FlowAim => ("flow_aim", "Flow", true),
    RawAim => ("raw_aim", "Raw", false),
    Precision => ("precision", "Precision", true),
    Speed => ("speed", "Speed", true),
    Stamina => ("stamina", "Stamina", true),
    Accuracy => ("accuracy", "Accuracy", true),
    StarRating => ("star_rating", "Stars", false),
});

// Full osu PPP chart vector. Its order matches `EzPPPSkillCatalog.MetricsFor(osu)`.
define_skill_axis!(EzPPPOSUSkillMetric {
    Aim => ("aim", "Aim", false),
    RawAim => ("raw_aim", "Raw", false),
    JumpAim => ("jump_aim", "Jump", true),
    FlowAim => ("flow_aim", "Flow", true),
    Precision => ("precision", "Precision", true),
    Speed => ("speed", "Speed", true),
    Stamina => ("stamina", "Stamina", true),
    Accuracy => ("accuracy", "Accuracy", true),
    StarRating => ("star_rating", "Stars", false),
    AccuracyDifficulty => ("accuracy_difficulty", "Accuracy Difficulty", false),
});

define_skill_axis!(EzPPPTaikoSkillAxis {
    Rhythm => ("rhythm", "Rhythm", true),
    Colour => ("colour", "Colour", true),
    Stamina => ("stamina", "Stamina", true),
    StarRating => ("star_rating", "Stars", false),
});

define_skill_axis!(EzPPPCatchSkillAxis {
    Movement => ("movement", "Movement", true),
    StarRating => ("star_rating", "Stars", false),
});

define_skill_axis!(EzPPPManiaSkillAxis {
    Strain => ("strain", "Strain", true),
    StarRating => ("star_rating", "Stars", false),
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_ids_and_order_match_persisted_contract() {
        assert_eq!(
            EzSunnySrSkillAxis::ALL
                .iter()
                .map(|axis| axis.id())
                .collect::<Vec<_>>(),
            vec![
                "bracket",
                "jack",
                "burst",
                "ln_hold",
                "ln_release",
                "anchor"
            ]
        );
        assert_eq!(
            EzMinaSkillAxis::ALL
                .iter()
                .map(|axis| axis.id())
                .collect::<Vec<_>>(),
            vec![
                "overall",
                "stream",
                "jumpstream",
                "handstream",
                "stamina",
                "jack",
                "chordjack",
                "tech"
            ]
        );
        assert_eq!(
            EzPPPSkillAxis::ALL
                .iter()
                .map(|axis| axis.id())
                .collect::<Vec<_>>(),
            vec![
                "aim",
                "jump_aim",
                "flow_aim",
                "raw_aim",
                "precision",
                "speed",
                "stamina",
                "accuracy",
                "star_rating"
            ]
        );
        assert_eq!(
            EzPPPOSUSkillMetric::ALL
                .iter()
                .map(|axis| axis.id())
                .collect::<Vec<_>>(),
            vec![
                "aim",
                "raw_aim",
                "jump_aim",
                "flow_aim",
                "precision",
                "speed",
                "stamina",
                "accuracy",
                "star_rating",
                "accuracy_difficulty"
            ]
        );
    }

    #[test]
    fn vectors_are_dense_typed_and_resolve_namespaced_ids() {
        let mut vector = SkillVector::<EzMinaSkillAxis>::from_fn(|axis| axis.index() as f64 + 1.0);
        assert_eq!(vector.get(EzMinaSkillAxis::JackSpeed), 6.0);
        assert_eq!(vector.get_by_id("jack"), Some(6.0));
        assert_eq!(vector.get_by_qualified_id("chart.JACK"), Some(6.0));
        vector.set(EzMinaSkillAxis::Overall, 2.5);
        assert_eq!(vector.as_slice()[0], 2.5);
        assert!(vector.is_complete());
        assert_eq!(vector.iter().count(), EzMinaSkillAxis::ALL.len());
    }

    #[test]
    fn vectors_reject_incomplete_axis_sets() {
        let error = SkillVector::<EzSunnySrSkillAxis>::from_values(vec![0.0]).unwrap_err();
        assert_eq!(error.expected, 6);
        assert_eq!(error.actual, 1);
    }
}
