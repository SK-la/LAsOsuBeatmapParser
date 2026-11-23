//! Four-key JackDemand classification ported from Ez2Lazer's Mina analyzer.

use crate::csi_patterns::RcPatternAxis;

pub const EZ_MINA_FOUR_KEY_JACK_DEMAND_VERSION: u32 = 20240102;

const DENSE_CHORD_RATIO_MIN: f64 = 0.70;
const DENSE_CHORD_OVERLAP_MIN: f64 = 0.58;
const TWO_BACK_REHIT_EXCESS_MIN: f64 = 0.30;
const JACKABLE_MAX_CLUSTER_BPM: f64 = 230.0;
const JACK_CLUSTER_SHARE_MIN: f64 = 0.60;
const JACK_CLUSTER_CORROBORATED_SHARE_MIN: f64 = 0.25;
const JACK_CLUSTER_CORROBORATED_CHORDJACK_MIN: f64 = 0.5;
const JACK_CLUSTER_CORROBORATED_PRESSURE_MIN: f64 = 150.0;
const JACK_CLUSTER_PATTERN: &str = "Jacks";
const MARATHON_DURATION_MIN_MS: f64 = 240_000.0;
const MARATHON_JACK_SCORE_MIN: f64 = 0.75;
const MARATHON_JACK_PRESSURE_MIN: f64 = 175.0;
const MARATHON_CHORD_RATIO_MIN: f64 = 0.45;
const MARATHON_CHORD_OVERLAP_MIN: f64 = 0.55;

/// A LeoBlack cluster summary consumed by the JackDemand classifier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EzMinaFourKeyJackDemandCluster {
    pub pattern: &'static str,
    pub bpm: f64,
    pub importance: f64,
}

/// A classified CSI pattern score consumed by the JackDemand classifier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EzMinaFourKeyJackDemandPatternScore {
    pub axis: RcPatternAxis,
    pub score: f64,
}

/// Input measurements for four-key JackDemand classification.
///
/// The caller supplies CSI and LeoBlack outputs so each analyzer remains independently usable
/// and callers can compose the passes without converting through string-keyed JSON structures.
#[derive(Clone, Copy, Debug)]
pub struct EzMinaFourKeyJackDemandInput<'a> {
    pub key_count: usize,
    pub duration_ms: f64,
    pub chord_ratio: f64,
    pub chord_column_overlap_ratio: f64,
    pub two_back_column_rehit_excess: f64,
    pub jack_pressure: f64,
    pub patterns: &'a [EzMinaFourKeyJackDemandPatternScore],
    pub clusters: &'a [EzMinaFourKeyJackDemandCluster],
}

/// A reason that the analyzer classified a chart as requiring four-key jacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EzMinaFourKeyJackDemandReason {
    DenseAlternatingChords,
    JackClusterDominant,
    JackClusterCorroborated,
    JackMarathon,
}

impl EzMinaFourKeyJackDemandReason {
    /// Stable reason ID used by Ez2Lazer's chart analysis output.
    pub const fn id(self) -> &'static str {
        match self {
            Self::DenseAlternatingChords => "dense_alternating_chords",
            Self::JackClusterDominant => "jack_cluster_dominant",
            Self::JackClusterCorroborated => "jack_cluster_corroborated",
            Self::JackMarathon => "jack_marathon",
        }
    }
}

/// Result of the four-key JackDemand classifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EzMinaFourKeyJackDemandVerdict {
    pub version: u32,
    pub detected: bool,
    pub reasons: Vec<EzMinaFourKeyJackDemandReason>,
}

pub struct EzMinaFourKeyJackDemand;

impl EzMinaFourKeyJackDemand {
    pub const VERSION: u32 = EZ_MINA_FOUR_KEY_JACK_DEMAND_VERSION;

    /// Classifies the chart using the same threshold and reason ordering as Ez2Lazer.
    pub fn classify(input: EzMinaFourKeyJackDemandInput<'_>) -> EzMinaFourKeyJackDemandVerdict {
        if input.key_count != 4 {
            return verdict(Vec::new());
        }

        let mut reasons = Vec::new();

        let dense_alternating_chords = input.chord_ratio >= DENSE_CHORD_RATIO_MIN
            && input.chord_column_overlap_ratio >= DENSE_CHORD_OVERLAP_MIN
            && input.two_back_column_rehit_excess >= TWO_BACK_REHIT_EXCESS_MIN;
        if dense_alternating_chords {
            reasons.push(EzMinaFourKeyJackDemandReason::DenseAlternatingChords);
        }

        let mean_cluster_bpm = weighted_mean_cluster_bpm(input.clusters);
        let jackable_speed = mean_cluster_bpm.is_none_or(|bpm| bpm <= JACKABLE_MAX_CLUSTER_BPM);

        let jack_cluster_share = jack_cluster_importance_share(input.clusters);
        let jack_cluster_dominant = jackable_speed
            && mean_cluster_bpm.is_some()
            && jack_cluster_share >= JACK_CLUSTER_SHARE_MIN;
        if jack_cluster_dominant {
            reasons.push(EzMinaFourKeyJackDemandReason::JackClusterDominant);
        }

        let jack_cluster_corroborated = !jack_cluster_dominant
            && jackable_speed
            && mean_cluster_bpm.is_some()
            && jack_cluster_share >= JACK_CLUSTER_CORROBORATED_SHARE_MIN
            && pattern_score(input.patterns, RcPatternAxis::Chordjack)
                >= JACK_CLUSTER_CORROBORATED_CHORDJACK_MIN
            && input.jack_pressure >= JACK_CLUSTER_CORROBORATED_PRESSURE_MIN;
        if jack_cluster_corroborated {
            reasons.push(EzMinaFourKeyJackDemandReason::JackClusterCorroborated);
        }

        let jack_marathon = jackable_speed
            && input.duration_ms >= MARATHON_DURATION_MIN_MS
            && pattern_score(input.patterns, RcPatternAxis::Jack) >= MARATHON_JACK_SCORE_MIN
            && input.jack_pressure >= MARATHON_JACK_PRESSURE_MIN
            && input.chord_ratio >= MARATHON_CHORD_RATIO_MIN
            && input.chord_column_overlap_ratio >= MARATHON_CHORD_OVERLAP_MIN;
        if jack_marathon {
            reasons.push(EzMinaFourKeyJackDemandReason::JackMarathon);
        }

        verdict(reasons)
    }
}

fn verdict(reasons: Vec<EzMinaFourKeyJackDemandReason>) -> EzMinaFourKeyJackDemandVerdict {
    EzMinaFourKeyJackDemandVerdict {
        version: EzMinaFourKeyJackDemand::VERSION,
        detected: !reasons.is_empty(),
        reasons,
    }
}

fn pattern_score(patterns: &[EzMinaFourKeyJackDemandPatternScore], axis: RcPatternAxis) -> f64 {
    patterns
        .iter()
        .filter(|pattern| pattern.axis == axis && pattern.score.is_finite())
        .map(|pattern| pattern.score)
        .fold(0.0, f64::max)
}

fn jack_cluster_importance_share(clusters: &[EzMinaFourKeyJackDemandCluster]) -> f64 {
    let mut total = 0.0;
    let mut jack = 0.0;

    for cluster in clusters {
        if !cluster.importance.is_finite() || cluster.importance <= 0.0 {
            continue;
        }

        total += cluster.importance;
        if !cluster.bpm.is_finite() || cluster.bpm <= 0.0 || cluster.bpm > JACKABLE_MAX_CLUSTER_BPM
        {
            continue;
        }

        if cluster.pattern == JACK_CLUSTER_PATTERN {
            jack += cluster.importance;
        }
    }

    if total > 0.0 { jack / total } else { 0.0 }
}

fn weighted_mean_cluster_bpm(clusters: &[EzMinaFourKeyJackDemandCluster]) -> Option<f64> {
    let mut weight = 0.0;
    let mut weighted = 0.0;

    for cluster in clusters {
        if !cluster.importance.is_finite() || cluster.importance <= 0.0 {
            continue;
        }
        if !cluster.bpm.is_finite() || cluster.bpm <= 0.0 {
            continue;
        }

        weight += cluster.importance;
        weighted += cluster.importance * cluster.bpm;
    }

    (weight > 0.0).then_some(weighted / weight)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input<'a>(
        key_count: usize,
        patterns: &'a [EzMinaFourKeyJackDemandPatternScore],
        clusters: &'a [EzMinaFourKeyJackDemandCluster],
    ) -> EzMinaFourKeyJackDemandInput<'a> {
        EzMinaFourKeyJackDemandInput {
            key_count,
            duration_ms: 60_000.0,
            chord_ratio: 0.0,
            chord_column_overlap_ratio: 0.0,
            two_back_column_rehit_excess: 0.0,
            jack_pressure: 0.0,
            patterns,
            clusters,
        }
    }

    #[test]
    fn requires_four_key_and_keeps_the_version() {
        let verdict = EzMinaFourKeyJackDemand::classify(input(7, &[], &[]));

        assert_eq!(verdict.version, 20240102);
        assert!(!verdict.detected);
        assert!(verdict.reasons.is_empty());
    }

    #[test]
    fn dense_alternating_chords_use_inclusive_thresholds() {
        let mut map = input(4, &[], &[]);
        map.chord_ratio = 0.70;
        map.chord_column_overlap_ratio = 0.58;
        map.two_back_column_rehit_excess = 0.30;

        let verdict = EzMinaFourKeyJackDemand::classify(map);

        assert!(verdict.detected);
        assert_eq!(
            verdict.reasons,
            [EzMinaFourKeyJackDemandReason::DenseAlternatingChords]
        );
        assert_eq!(verdict.reasons[0].id(), "dense_alternating_chords");
    }

    #[test]
    fn jack_cluster_dominance_uses_weighted_share_and_speed_gate() {
        let clusters = [
            EzMinaFourKeyJackDemandCluster {
                pattern: "Jacks",
                bpm: 160.0,
                importance: 90.0,
            },
            EzMinaFourKeyJackDemandCluster {
                pattern: "Chordstream",
                bpm: 200.0,
                importance: 10.0,
            },
        ];

        let verdict = EzMinaFourKeyJackDemand::classify(input(4, &[], &clusters));

        assert_eq!(
            verdict.reasons,
            [EzMinaFourKeyJackDemandReason::JackClusterDominant]
        );
    }

    #[test]
    fn mean_cluster_speed_above_230_bpm_vetoes_jack_cluster_and_marathon() {
        let patterns = [EzMinaFourKeyJackDemandPatternScore {
            axis: RcPatternAxis::Jack,
            score: 1.0,
        }];
        let clusters = [
            EzMinaFourKeyJackDemandCluster {
                pattern: "Jacks",
                bpm: 230.0,
                importance: 60.0,
            },
            EzMinaFourKeyJackDemandCluster {
                pattern: "Chordstream",
                bpm: 250.0,
                importance: 40.0,
            },
        ];
        let mut map = input(4, &patterns, &clusters);
        map.duration_ms = 300_000.0;
        map.jack_pressure = 200.0;
        map.chord_ratio = 0.8;
        map.chord_column_overlap_ratio = 0.8;

        let verdict = EzMinaFourKeyJackDemand::classify(map);

        assert!(!verdict.detected);
        assert!(verdict.reasons.is_empty());
    }

    #[test]
    fn corroborated_cluster_requires_chordjack_pattern_and_pressure() {
        let patterns = [EzMinaFourKeyJackDemandPatternScore {
            axis: RcPatternAxis::Chordjack,
            score: 0.5,
        }];
        let clusters = [
            EzMinaFourKeyJackDemandCluster {
                pattern: "Jacks",
                bpm: 160.0,
                importance: 25.0,
            },
            EzMinaFourKeyJackDemandCluster {
                pattern: "Chordstream",
                bpm: 200.0,
                importance: 75.0,
            },
        ];
        let mut map = input(4, &patterns, &clusters);
        map.jack_pressure = 150.0;

        let verdict = EzMinaFourKeyJackDemand::classify(map);

        assert_eq!(
            verdict.reasons,
            [EzMinaFourKeyJackDemandReason::JackClusterCorroborated]
        );
    }

    #[test]
    fn marathon_uses_duration_jack_pattern_pressure_and_chord_thresholds() {
        let patterns = [EzMinaFourKeyJackDemandPatternScore {
            axis: RcPatternAxis::Jack,
            score: 0.75,
        }];
        let mut map = input(4, &patterns, &[]);
        map.duration_ms = 240_000.0;
        map.jack_pressure = 175.0;
        map.chord_ratio = 0.45;
        map.chord_column_overlap_ratio = 0.55;

        let verdict = EzMinaFourKeyJackDemand::classify(map);

        assert_eq!(
            verdict.reasons,
            [EzMinaFourKeyJackDemandReason::JackMarathon]
        );
    }

    #[test]
    fn invalid_cluster_measurements_do_not_create_demand_or_corrupt_share() {
        let clusters = [
            EzMinaFourKeyJackDemandCluster {
                pattern: "Jacks",
                bpm: f64::NAN,
                importance: f64::INFINITY,
            },
            EzMinaFourKeyJackDemandCluster {
                pattern: "Jacks",
                bpm: 160.0,
                importance: -1.0,
            },
            EzMinaFourKeyJackDemandCluster {
                pattern: "jacks",
                bpm: 160.0,
                importance: 100.0,
            },
        ];

        let verdict = EzMinaFourKeyJackDemand::classify(input(4, &[], &clusters));

        assert!(!verdict.detected);
        assert!(verdict.reasons.is_empty());
    }
}
