//! Cross matrix (inter-column weight) tables for the SR core.
//!
//! Every table is indexed by key count `k` and holds `k + 1` coefficients. The data mirrors
//! `CrossMatrixProvider` in Ez2Lazer, odd key modes included: the C# side rates 11/13/15/17 from the
//! neighbour-inferred values it stores explicitly, so a library that rejects them would make charts
//! un-rateable that the host can currently rate.
//!
//! The host may also pass its own table (Ez exposes a custom-matrix hook); `validate` is the check both
//! paths share.

/// Largest key count the tables cover.
pub const MAX_SUPPORTED_KEY_COUNT: usize = 18;

/// Index 0 is the `CS=0` placeholder, index `k` is `k` keys.
const DEFAULT_MATRICES: [&[f64]; MAX_SUPPORTED_KEY_COUNT + 1] = [
    // CS=0 placeholder: never requested as a real key count.
    &[-1.0],
    // K=1
    &[0.075, 0.075],
    // K=2
    &[0.125, 0.05, 0.125],
    // K=3
    &[0.125, 0.125, 0.125, 0.125],
    // K=4
    &[0.175, 0.25, 0.05, 0.25, 0.175],
    // K=5
    &[0.175, 0.25, 0.175, 0.175, 0.25, 0.175],
    // K=6
    &[0.225, 0.35, 0.25, 0.05, 0.25, 0.35, 0.225],
    // K=7
    &[0.225, 0.35, 0.25, 0.225, 0.225, 0.25, 0.35, 0.225],
    // K=8
    &[0.275, 0.45, 0.35, 0.25, 0.05, 0.25, 0.35, 0.45, 0.275],
    // K=9
    &[0.275, 0.45, 0.35, 0.25, 0.275, 0.275, 0.25, 0.35, 0.45, 0.275],
    // K=10
    &[0.325, 0.55, 0.45, 0.35, 0.25, 0.05, 0.25, 0.35, 0.45, 0.55, 0.325],
    // K=11 (neighbour-inferred from K=10 and K=12)
    &[0.5625, 0.675, 0.625, 0.475, 0.325, 0.125, 0.15, 0.275, 0.425, 0.575, 0.5625, 0.5625],
    // K=12
    &[0.8, 0.8, 0.8, 0.6, 0.4, 0.2, 0.05, 0.2, 0.4, 0.6, 0.8, 0.8, 0.8],
    // K=13 (neighbour-inferred from K=12 and K=14)
    &[0.6, 0.6, 0.5, 0.4, 0.35, 0.25, 0.075, 0.15, 0.35, 0.45, 0.5, 0.5, 0.6, 0.6],
    // K=14
    &[0.4, 0.4, 0.2, 0.2, 0.3, 0.3, 0.1, 0.1, 0.3, 0.3, 0.2, 0.2, 0.4, 0.4, 0.4],
    // K=15 (neighbour-inferred from K=14 and K=16)
    &[0.4, 0.4, 0.2, 0.2, 0.35, 0.35, 0.15, 0.1, 0.2, 0.25, 0.3, 0.3, 0.3, 0.3, 0.4, 0.4],
    // K=16
    &[0.4, 0.4, 0.2, 0.2, 0.4, 0.4, 0.2, 0.1, 0.1, 0.2, 0.4, 0.4, 0.2, 0.2, 0.4, 0.4, 0.4],
    // K=17 (neighbour-inferred from K=16 and K=18)
    &[0.4, 0.4, 0.2, 0.3, 0.3, 0.4, 0.2, 0.2, 0.1, 0.15, 0.35, 0.3, 0.3, 0.2, 0.4, 0.3, 0.4, 0.4],
    // K=18
    &[0.4, 0.4, 0.2, 0.4, 0.2, 0.4, 0.2, 0.3, 0.1, 0.1, 0.3, 0.2, 0.4, 0.2, 0.4, 0.2, 0.4, 0.4, 0.4],
];

pub struct CrossMatrixProvider;

impl CrossMatrixProvider {
    /// The default table for `k` keys, or `None` when `k` is outside `1..=18`.
    pub fn get_matrix(k: i32) -> Option<&'static [f64]> {
        if k < 1 || k as usize > MAX_SUPPORTED_KEY_COUNT {
            return None;
        }

        Some(DEFAULT_MATRICES[k as usize])
    }

    /// Whether the default tables cover `k`. Odd key modes are included.
    pub fn is_supported(k: i32) -> bool {
        Self::get_matrix(k).is_some()
    }

    /// Every key count the default tables cover, ascending.
    pub fn supported_keys() -> Vec<i32> {
        (1..=MAX_SUPPORTED_KEY_COUNT as i32).collect()
    }

    /// Checks a caller-supplied table: `k + 1` coefficients, each finite and non-negative.
    ///
    /// This is the only validity rule; the SR core assumes it held when it indexes `cross[k]`.
    pub fn validate(k: i32, matrix: &[f64]) -> Result<(), String> {
        if k < 1 || k as usize > MAX_SUPPORTED_KEY_COUNT {
            return Err(format!("Unsupported key count: {k}"));
        }

        let expected = (k + 1) as usize;

        if matrix.len() != expected {
            return Err(format!("Cross matrix for {k}k must have {expected} values, got {}", matrix.len()));
        }

        for (index, value) in matrix.iter().enumerate() {
            if !value.is_finite() || *value < 0.0 {
                return Err(format!("Cross matrix for {k}k is invalid at {index}: {value}"));
            }
        }

        Ok(())
    }
}
