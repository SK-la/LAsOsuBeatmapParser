//! Fixed-layout score input shared by the ruleset performance calculators.

use crate::mods::Mod;

/// Judgement results used by lazer rulesets and their score calculators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum HitResult {
    Perfect = 0,
    Great = 1,
    Good = 2,
    Ok = 3,
    Meh = 4,
    Miss = 5,
    Geki = 6,
    Katu = 7,
    LargeTickHit = 8,
    LargeTickMiss = 9,
    SmallTickHit = 10,
    SmallTickMiss = 11,
    SliderTailHit = 12,
    SliderTailMiss = 13,
    IgnoreHit = 14,
    IgnoreMiss = 15,
    StrongGreat = 16,
    StrongOk = 17,
}

impl HitResult {
    pub const COUNT: usize = 18;

    pub const ALL: [Self; Self::COUNT] = [
        Self::Perfect,
        Self::Great,
        Self::Good,
        Self::Ok,
        Self::Meh,
        Self::Miss,
        Self::Geki,
        Self::Katu,
        Self::LargeTickHit,
        Self::LargeTickMiss,
        Self::SmallTickHit,
        Self::SmallTickMiss,
        Self::SliderTailHit,
        Self::SliderTailMiss,
        Self::IgnoreHit,
        Self::IgnoreMiss,
        Self::StrongGreat,
        Self::StrongOk,
    ];
}

/// Compact score result counts in a fixed order, analogous to `ScoreInfo.Statistics`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreStatistics {
    counts: [i32; HitResult::COUNT],
}

impl Default for ScoreStatistics {
    fn default() -> Self {
        Self {
            counts: [0; HitResult::COUNT],
        }
    }
}

impl ScoreStatistics {
    pub fn get(&self, result: HitResult) -> i32 {
        self.counts[result as usize]
    }

    pub fn set(&mut self, result: HitResult, count: i32) {
        self.counts[result as usize] = count;
    }

    pub fn increment(&mut self, result: HitResult, count: i32) {
        self.counts[result as usize] += count;
    }

    pub fn as_slice(&self) -> &[i32; HitResult::COUNT] {
        &self.counts
    }
}

/// Score information consumed by the per-ruleset performance calculators.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScoreInfo {
    pub mods: Vec<Mod>,
    pub statistics: ScoreStatistics,
    pub max_combo: i32,
    pub total_score: i64,
    pub legacy_total_score: i64,
    pub accuracy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judgement_counts_use_a_stable_dense_layout() {
        let mut statistics = ScoreStatistics::default();
        statistics.set(HitResult::Great, 12);
        statistics.increment(HitResult::Miss, 2);
        assert_eq!(statistics.get(HitResult::Great), 12);
        assert_eq!(statistics.get(HitResult::Miss), 2);
        assert_eq!(statistics.as_slice().len(), HitResult::COUNT);
    }
}
