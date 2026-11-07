//! C ABI for the SR core.
//!
//! The host passes the notes and the cross matrix directly: no JSON, no file path, no per-call allocation of the
//! buffers. Every value the caller needs to reproduce the C# reference (the integer rounding of head/tail, the
//! cross matrix, the two tunables) is an argument, so the two implementations can be compared on identical input.
//!
//! Layout rules:
//! * `EzSrNote` is 12 bytes, all `i32`, with `tail = -1` for a tap.
//! * `EzSrResult` is `f64` + a fixed component array, so adding a component later does not change the struct size
//!   the host marshals; the host reads `component_count` and maps order to meaning.
//! * Every entry returns `0` on success and a negative code on failure; output is only written on success.

use crate::cross_matrix::CrossMatrixProvider;
use crate::sr::{SrConfig, SrNote, SrOutput, SRCalculator, COMPONENT_COUNT};

/// A note as the host encodes it: column, head time, tail time (`-1` for a tap). All milliseconds.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EzSrNote {
    pub col: i32,
    pub head: i32,
    pub tail: i32,
}

/// Result payload. `components` is in the order `Bracket, Jack, Burst, LongNoteUsage, LongNoteRelease, Anchor`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EzSrResult {
    pub sr: f64,
    pub components: [f64; COMPONENT_COUNT],
    pub component_count: u32,
}

/// `Ok`.
pub const EZ_SR_OK: i32 = 0;
/// A required pointer was null.
pub const EZ_SR_ERR_NULL: i32 = -1;
/// The key count is outside `1..=18`.
pub const EZ_SR_ERR_KEY_COUNT: i32 = -2;
/// The supplied cross matrix does not match the key count (length or coefficient range).
pub const EZ_SR_ERR_CROSS_MATRIX: i32 = -3;
/// The core rejected the input for another reason.
pub const EZ_SR_ERR_COMPUTE: i32 = -4;

/// Config the host owns, kept separate so adding one does not change the call signature.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EzSrConfig {
    /// Star rating above which the high-end rescale applies. Ez uses 8.0.
    pub rescale_threshold: f64,
    /// Multiplier on the long-note integral in the burst curve. Ez uses 5.5.
    pub ln_multiplier: f64,
}

impl EzSrConfig {
    fn to_internal(self) -> SrConfig {
        SrConfig {
            rescale_threshold: if self.rescale_threshold.is_finite() && self.rescale_threshold > 0.0 {
                self.rescale_threshold
            } else {
                SrConfig::EZ_DEFAULT.rescale_threshold
            },
            ln_multiplier: if self.ln_multiplier.is_finite() && self.ln_multiplier > 0.0 {
                self.ln_multiplier
            } else {
                SrConfig::EZ_DEFAULT.ln_multiplier
            },
        }
    }
}

/// Rating plus components for `count` notes.
///
/// The times are already in the rated time base: the host applies its clock-rate division and rounds to whole
/// milliseconds before calling, so there is exactly one rounding rule and this module never re-derives it.
///
/// # Safety
/// `notes` must point to `count` readable `EzSrNote` values, `cross` to `key_count + 1` readable `f64` values, and
/// `out` to one writable `EzSrResult`. All three must be non-null.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ez_sr_calculate(
    notes: *const EzSrNote,
    count: usize,
    key_count: i32,
    od: f64,
    cross: *const f64,
    config: EzSrConfig,
    out: *mut EzSrResult,
) -> i32 {
    if notes.is_null() || cross.is_null() || out.is_null() {
        return EZ_SR_ERR_NULL;
    }

    if !(1..=CrossMatrixProvider::supported_keys().len() as i32).contains(&key_count) {
        return EZ_SR_ERR_KEY_COUNT;
    }

    let cross_len = (key_count + 1) as usize;
    // SAFETY: the caller guarantees `key_count + 1` readable values; `key_count` was just range-checked.
    let cross_slice = unsafe { std::slice::from_raw_parts(cross, cross_len) };

    if CrossMatrixProvider::validate(key_count, cross_slice).is_err() {
        return EZ_SR_ERR_CROSS_MATRIX;
    }

    let mut note_seq: Vec<SrNote> = Vec::with_capacity(count);

    for i in 0..count {
        // SAFETY: the caller guarantees `count` readable notes.
        let note = unsafe { *notes.add(i) };
        // A tail that is not strictly after its head is a tap, matching the host's rule.
        let tail = if note.tail < 0 || note.tail <= note.head { -1 } else { note.tail };

        note_seq.push((note.col.clamp(0, key_count - 1), note.head, tail));
    }

    let effective_od = if od.is_finite() && od >= 0.0 { od } else { 0.0 };

    match SRCalculator::calculate_with_components(note_seq, key_count, effective_od, cross_slice, config.to_internal()) {
        Ok(result) => {
            // SAFETY: the caller guarantees `out` is writable.
            unsafe {
                *out = EzSrResult {
                    sr: result.sr,
                    components: result.components,
                    component_count: COMPONENT_COUNT as u32,
                };
            }

            EZ_SR_OK
        }
        Err(_) => EZ_SR_ERR_COMPUTE,
    }
}

/// The default cross matrix for `key_count` keys, or null when the key count is outside `1..=18`.
///
/// Lets the host read the table it should pass, so the two sides cannot disagree about it. The returned slice is
/// static and valid for the process lifetime.
#[unsafe(no_mangle)]
pub extern "C" fn ez_sr_cross_matrix(key_count: i32) -> *const f64 {
    match CrossMatrixProvider::get_matrix(key_count) {
        Some(matrix) => matrix.as_ptr(),
        None => std::ptr::null(),
    }
}

/// Length of the table `ez_sr_cross_matrix` returns for `key_count`, or 0 when unsupported.
#[unsafe(no_mangle)]
pub extern "C" fn ez_sr_cross_matrix_len(key_count: i32) -> usize {
    CrossMatrixProvider::get_matrix(key_count).map(|m| m.len()).unwrap_or(0)
}

/// Number of components a successful call writes.
pub fn component_count() -> usize {
    COMPONENT_COUNT
}

/// Convenience wrapper for Rust-side tests and tools; the C ABI above is what the host calls.
///
/// Times are already rate-adjusted whole milliseconds, as in the ABI.
pub fn calculate_notes(notes: &[SrNote], key_count: i32, od: f64, config: SrConfig) -> Option<SrOutput> {
    let cross = CrossMatrixProvider::get_matrix(key_count)?;

    let normalized: Vec<SrNote> = notes
        .iter()
        .map(|&(col, head, tail)| {
            let t = if tail < 0 || tail <= head { -1 } else { tail };
            (col.clamp(0, key_count - 1), head, t)
        })
        .collect();

    SRCalculator::calculate_with_components(normalized, key_count, od, cross, config).ok()
}
