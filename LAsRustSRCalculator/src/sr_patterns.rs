//! Typed chart-skill and radar projection for the Sunny SR4 component pass.
//!
//! [`crate::sr::SRCalculator`] computes the six component ratings alongside the total SR. This
//! module exposes those values using the same stable axes as Ez2Lazer without converting them to
//! a string-keyed map or requiring UI types.

use crate::skills::{EzSunnySrSkillAxis, SkillAxis, SkillVector};
use crate::sr::{COMPONENT_COUNT, SrOutput};

/// One validated Sunny SR component set, ready for chart storage or radar display.
#[derive(Clone, Debug, PartialEq)]
pub struct EzSunnySrSkillResult {
    pub star_rating: f64,
    pub skills: SkillVector<EzSunnySrSkillAxis>,
}

impl EzSunnySrSkillResult {
    /// Projects the component layout returned by the SR core into Ez2Lazer axis order.
    pub fn from_sr_output(output: &SrOutput) -> Option<Self> {
        let skills = SkillVector::from_values(output.components.to_vec()).ok()?;

        if !output.sr.is_finite() || output.sr < 0.0 || !skills.is_complete() {
            return None;
        }

        Some(Self {
            star_rating: output.sr,
            skills,
        })
    }

    /// Rebuilds a chart skill result from dense values in Ez2Lazer axis order.
    ///
    /// This is the Rust equivalent of `EzSunnySrSkillRadar.TryCreateFromStored`: partial or
    /// non-finite skill sets are unavailable instead of being padded with zeros.
    pub fn try_from_stored(star_rating: f64, values: &[f64]) -> Option<Self> {
        if !star_rating.is_finite() || star_rating < 0.0 || values.len() != COMPONENT_COUNT {
            return None;
        }

        let skills = SkillVector::from_values(values.to_vec()).ok()?;
        skills.is_complete().then_some(Self {
            star_rating,
            skills,
        })
    }

    /// Returns the chart values in canonical radar order with Ez2Lazer's persisted IDs and labels.
    pub fn radar_axes(&self) -> impl ExactSizeIterator<Item = EzSunnySrRadarAxisValue> + '_ {
        self.skills
            .iter()
            .map(|(axis, value)| EzSunnySrRadarAxisValue {
                axis,
                id: axis.id(),
                label: axis.display_name(),
                value,
            })
    }
}

/// One typed value in the Sunny SR skill radar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EzSunnySrRadarAxisValue {
    pub axis: EzSunnySrSkillAxis,
    pub id: &'static str,
    pub label: &'static str,
    pub value: f64,
}

/// Chart-level helper matching the Ez2Lazer SR4 axis and radar entry points.
pub struct EzSunnySrSkillRadar;

impl EzSunnySrSkillRadar {
    /// Extracts all six chart axes from an SR calculation result.
    pub fn from_output(output: &SrOutput) -> Option<EzSunnySrSkillResult> {
        EzSunnySrSkillResult::from_sr_output(output)
    }

    /// Returns true only when one finite value is present for each stable axis.
    pub fn has_complete_chart_skills(values: &[f64]) -> bool {
        values.len() == EzSunnySrSkillAxis::ALL.len()
            && values.iter().all(|value| value.is_finite())
    }

    /// Builds a radar-ready result from dense stored chart values.
    pub fn try_create_from_stored(
        star_rating: f64,
        values: &[f64],
    ) -> Option<EzSunnySrSkillResult> {
        EzSunnySrSkillResult::try_from_stored(star_rating, values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::SkillAxis;

    #[test]
    fn output_components_map_to_the_persisted_sunny_axis_order() {
        let output = SrOutput {
            sr: 7.25,
            components: [1.1, 2.2, 3.3, 4.4, 5.5, 6.6],
        };

        let result = EzSunnySrSkillRadar::from_output(&output).unwrap();

        assert_eq!(result.star_rating, 7.25);
        assert_eq!(result.skills.get(EzSunnySrSkillAxis::Bracket), 1.1);
        assert_eq!(result.skills.get(EzSunnySrSkillAxis::Jack), 2.2);
        assert_eq!(result.skills.get(EzSunnySrSkillAxis::Burst), 3.3);
        assert_eq!(result.skills.get(EzSunnySrSkillAxis::LnHold), 4.4);
        assert_eq!(result.skills.get(EzSunnySrSkillAxis::LnRelease), 5.5);
        assert_eq!(result.skills.get(EzSunnySrSkillAxis::Anchor), 6.6);
        assert!(result.skills.is_complete());
    }

    #[test]
    fn radar_axes_preserve_ez_ids_labels_and_order() {
        let result =
            EzSunnySrSkillRadar::try_create_from_stored(6.8, &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6])
                .unwrap();

        let axes = result.radar_axes().collect::<Vec<_>>();

        assert_eq!(
            axes.iter().map(|axis| axis.id).collect::<Vec<_>>(),
            [
                "bracket",
                "jack",
                "burst",
                "ln_hold",
                "ln_release",
                "anchor"
            ]
        );
        assert_eq!(
            axes.iter().map(|axis| axis.label).collect::<Vec<_>>(),
            [
                "Bracket",
                "Jack",
                "Burst",
                "LN Hold",
                "LN Release",
                "Anchor"
            ]
        );
        assert_eq!(axes[3].axis, EzSunnySrSkillAxis::LnHold);
        assert_eq!(axes[3].value, 0.4);
    }

    #[test]
    fn partial_non_finite_and_invalid_rating_skill_data_are_unavailable() {
        assert!(!EzSunnySrSkillRadar::has_complete_chart_skills(&[]));
        assert!(!EzSunnySrSkillRadar::has_complete_chart_skills(&[1.0; 5]));
        assert!(!EzSunnySrSkillRadar::has_complete_chart_skills(&[
            1.0,
            2.0,
            f64::NAN,
            4.0,
            5.0,
            6.0,
        ]));
        assert!(EzSunnySrSkillRadar::try_create_from_stored(6.0, &[1.0; 5]).is_none());
        assert!(
            EzSunnySrSkillRadar::try_create_from_stored(
                6.0,
                &[1.0, 2.0, 3.0, f64::INFINITY, 5.0, 6.0]
            )
            .is_none()
        );
        assert!(EzSunnySrSkillRadar::try_create_from_stored(-1.0, &[1.0; 6]).is_none());
    }

    #[test]
    fn axis_ids_match_ez_persisted_contract() {
        assert_eq!(
            EzSunnySrSkillAxis::ALL
                .iter()
                .map(|axis| axis.id())
                .collect::<Vec<_>>(),
            [
                "bracket",
                "jack",
                "burst",
                "ln_hold",
                "ln_release",
                "anchor"
            ]
        );
    }
}
