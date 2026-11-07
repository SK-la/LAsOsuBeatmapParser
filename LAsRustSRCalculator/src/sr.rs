use crate::cross_matrix::CrossMatrixProvider;
use crate::parser::ParsedData;

#[derive(Clone, Copy, PartialEq)]
enum SmoothMode {
    Sum,
    Average,
}

/// Number of component values the breakdown produces, in this order:
/// Bracket, Jack, Burst, Long-note usage, Long-note release, Anchor.
pub const COMPONENT_COUNT: usize = 6;

/// Tunables the host owns. Ez passes its own values so the rating matches the C# reference; the legacy
/// file-path entry keeps the defaults this port started with.
#[derive(Clone, Copy)]
pub struct SrConfig {
    pub rescale_threshold: f64,
    pub ln_multiplier: f64,
}

impl SrConfig {
    /// The values Ez2Lazer produces and persists (C# `RescaleHighThreshold` / `LnIntegralMultiplier`).
    pub const EZ_DEFAULT: SrConfig = SrConfig { rescale_threshold: 8.0, ln_multiplier: 5.5 };

    /// The original defaults of this port, kept for the file-path entry so its behaviour does not change.
    pub const LEGACY_DEFAULT: SrConfig = SrConfig { rescale_threshold: 9.0, ln_multiplier: 6.0 };
}

#[derive(Clone, Copy)]
pub struct SrOutput {
    pub sr: f64,
    pub components: [f64; COMPONENT_COUNT],
}

pub struct SRCalculator;

/// `(column, head_ms, tail_ms)`, with `tail = -1` for a tap.
pub type SrNote = (i32, i32, i32);

impl SRCalculator {
    /// Star rating only, from a parsed `.osu`. Kept for the standalone/file-path use case.
    pub fn calculate_sr_from_parsed_data(data: &ParsedData) -> Result<f64, String> {
        let note_seq = build_note_seq(data);
        let cross = CrossMatrixProvider::get_matrix(data.column_count)
            .ok_or_else(|| format!("No cross matrix for {}k", data.column_count))?;
        Ok(Self::calculate_with_components(note_seq, data.column_count, data.od, cross, SrConfig::LEGACY_DEFAULT)?.sr)
    }

    /// Star rating plus the six-component breakdown.
    ///
    /// The caller owns the integer rounding (including any clock-rate division) and the cross matrix, so the
    /// numbers arriving here are the ones being rated and odd key modes are not gated out by this module.
    pub fn calculate_with_components(
        mut note_seq: Vec<SrNote>,
        k: i32,
        od: f64,
        cross: &[f64],
        config: SrConfig,
    ) -> Result<SrOutput, String> {
        if !(1..=18).contains(&k) {
            return Err(format!("Unsupported key count: {k}"));
        }

        let expected_cross = (k + 1) as usize;

        if cross.len() < expected_cross {
            return Err(format!("Cross matrix for {k}k has {} values, expected at least {expected_cross}", cross.len()));
        }

        let zero = SrOutput { sr: 0.0, components: [0.0; COMPONENT_COUNT] };

        if note_seq.is_empty() {
            return Ok(zero);
        }

        // Sort by (start_time, column) as in Python
        note_seq.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

        // Hit leniency x - exactly as in Python
        let x = 0.3 * ((64.5 - (od * 3.0).ceil()) / 500.0).sqrt();
        let x = x.min(0.6 * (x - 0.09) + 0.09);

        // Group notes by column
        let mut note_seq_by_column: Vec<Vec<(i32, i32, i32)>> = vec![vec![]; k as usize];
        for &note in &note_seq {
            let col = note.0.max(0).min((k - 1) as i32) as usize;
            note_seq_by_column[col].push(note);
        }

        // LN sequences
        let ln_seq: Vec<(i32, i32, i32)> = note_seq.iter().filter(|&&(_, _, t)| t >= 0).cloned().collect();
        let mut tail_seq = ln_seq.clone();
        tail_seq.sort_by(|a, b| a.2.cmp(&b.2)); // Sort by tail time

        // Calculate T
        let t = note_seq.iter().map(|&(_, h, t)| h.max(t)).max().unwrap() + 1;

        let (all_corners, base_corners, a_corners) = Self::get_corners(t, &note_seq);

        // Get key usage
        let key_usage = Self::get_key_usage(k, t, &note_seq, &base_corners);
        let active_columns = Self::derive_active_columns(&key_usage);


        let key_usage_400 = Self::get_key_usage_400(k, t, &note_seq, &base_corners);
        let anchor = Self::compute_anchor(k, &key_usage_400, &base_corners);

        // Compute Jbar and delta_ks
        let (delta_ks, jbar) = Self::compute_jbar(k, &note_seq_by_column, &base_corners, x);

        // Compute Xbar. The cross matrix is the caller's (or the shared table's), not a copy local to this
        // function: it must be the same table the host validated.
        let xbar = Self::compute_xbar(k, x, &note_seq_by_column, &active_columns, &base_corners, cross);

        // Compute Pbar. One pass produces all three variants the result needs: the plain anchored curve (the
        // rating), the curve without hold contribution, and the curve with holds but a neutral anchor (both
        // feed the component breakdown).
        let ln_rep = if !ln_seq.is_empty() { Some(Self::build_ln_representation(&ln_seq, t)) } else { None };

        let (burst_base, ln_usage_base, pbar_base) = Self::compute_pbar_variants(
            x, &note_seq, ln_rep.as_ref(), &anchor, &base_corners, config.ln_multiplier);

        let burst_bar = Self::interp_values(&all_corners, &base_corners, &burst_base);
        let ln_usage_bar = Self::interp_values(&all_corners, &base_corners, &ln_usage_base);
        let pbar = Self::interp_values(&all_corners, &base_corners, &pbar_base);

        // Compute Abar
        let abar = Self::compute_abar(k, &delta_ks, &active_columns, &a_corners, &base_corners);

        // Compute Rbar
        let rbar = Self::compute_rbar(x, &note_seq_by_column, &tail_seq, &base_corners);

        // Compute C and Ks
        let (c_arr, ks_arr) = Self::compute_c_and_ks(k, &note_seq, &key_usage, &base_corners);

        // Final SR calculation
        let jbar_interp = Self::interp_values(&all_corners, &base_corners, &jbar);
        let xbar_interp = Self::interp_values(&all_corners, &base_corners, &xbar);
        let abar_interp = Self::interp_values(&all_corners, &a_corners, &abar);
        let rbar_interp = Self::interp_values(&all_corners, &base_corners, &rbar);
        let c_arr_interp = Self::step_interp(&all_corners, &base_corners, &c_arr);
        let ks_arr_interp = Self::step_interp(&all_corners, &base_corners, &ks_arr);

        let sr = Self::calculate_final_sr(
            &jbar_interp,
            &xbar_interp,
            &pbar,
            &abar_interp,
            &rbar_interp,
            &c_arr_interp,
            &ks_arr_interp,
            &note_seq,
            &ln_seq,
            &all_corners,
            &burst_bar,
            &ln_usage_bar,
            config,
        );

        Ok(sr)
    }

    fn get_corners(t: i32, note_seq: &[(i32, i32, i32)]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let mut corners_base = std::collections::BTreeSet::new();
        for &(_, h, tail) in note_seq {
            corners_base.insert(h);
            if tail >= 0 {
                corners_base.insert(tail);
            }
        }
        corners_base.insert(0);
        corners_base.insert(t);
        let corners_base_list: Vec<i32> = corners_base.iter().cloned().collect();
        for &s in &corners_base_list {
            corners_base.insert(s + 501);
            corners_base.insert(s - 499);
            corners_base.insert(s + 1);
        }
        let mut corners_base_vec: Vec<f64> = corners_base.into_iter()
            .filter(|&s| s >= 0 && s <= t)
            .map(|s| s as f64)
            .collect();
        corners_base_vec.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mut corners_a = std::collections::BTreeSet::new();
        for &(_, h, tail) in note_seq {
            corners_a.insert(h);
            if tail >= 0 {
                corners_a.insert(tail);
            }
        }
        corners_a.insert(0);
        corners_a.insert(t);
        let corners_a_list: Vec<i32> = corners_a.iter().cloned().collect();
        for &s in &corners_a_list {
            corners_a.insert(s + 1000);
            corners_a.insert(s - 1000);
        }
        let mut a_corners: Vec<f64> = corners_a.into_iter()
            .filter(|&s| s >= 0 && s <= t)
            .map(|s| s as f64)
            .collect();
        a_corners.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mut all_corners = vec![];
        for &s in &corners_base_vec {
            all_corners.push(s);
        }
        for &s in &a_corners {
            all_corners.push(s);
        }
        all_corners.sort_by(|a, b| a.partial_cmp(b).unwrap());
        all_corners.dedup();

        (all_corners, corners_base_vec, a_corners)
    }

    fn build_ln_representation(ln_seq: &[(i32, i32, i32)], t: i32) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let mut changes = vec![];
        for &(_k, h, tail) in ln_seq {
            let t0 = (h + 60).min(tail) as f64;
            let t1 = (h + 120).min(tail) as f64;
            changes.push((t0, 1.3));
            changes.push((t1, -1.3 + 1.0));
            changes.push((tail as f64, -1.0));
        }
        changes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let mut points: Vec<f64> = vec![0.0, t as f64];
        for &(t, _) in &changes {
            points.push(t);
        }
        points.sort_by(|a, b| a.partial_cmp(b).unwrap());
        points.dedup();

        let mut values = vec![];
        let mut cumsum = vec![0.0];
        let mut curr: f64 = 0.0;
        let mut change_idx = 0;

        for i in 0..points.len() - 1 {
            let t_curr = points[i];
            while change_idx < changes.len() && changes[change_idx].0 <= t_curr {
                curr += changes[change_idx].1;
                change_idx += 1;
            }
            let v = curr.min(2.5 + 0.5 * curr);
            values.push(v);
            let seg_length = points[i + 1] - points[i];
            cumsum.push(cumsum.last().unwrap() + seg_length * v);
        }

        (points, cumsum, values)
    }
    fn derive_active_columns(key_usage: &[Vec<bool>]) -> Vec<Vec<usize>> {
        let length = key_usage[0].len();
        let mut active = vec![vec![]; length];
        for i in 0..length {
            for col in 0..key_usage.len() {
                if key_usage[col][i] {
                    active[i].push(col);
                }
            }
        }
        active
    }

    fn get_key_usage(k: i32, total_time: i32, note_seq: &[(i32, i32, i32)], base_corners: &[f64]) -> Vec<Vec<bool>> {
        let mut key_usage = vec![vec![false; base_corners.len()]; k as usize];
        for &(col, h, tail) in note_seq {
            let start = (h - 150).max(0) as f64;
            let end = if tail >= 0 { ((tail + 150) as f64).min(total_time as f64 - 1.0) } else { ((h + 150) as f64).min(total_time as f64 - 1.0) };
            let left = Self::bisect_left(base_corners, start);
            let right = Self::bisect_left(base_corners, end);
            for idx in left..right {
                key_usage[col as usize][idx] = true;
            }
        }
        key_usage
    }

    fn add_falloff_contribution(usage: &mut [Vec<f64>], col: usize, base_corners: &[f64], range: std::ops::Range<usize>, ref_time: f64) {
        for idx in range {
            let offset = base_corners[idx] - ref_time;
            let falloff_contribution = 3.75 / (400.0 * 400.0) * offset * offset;
            let value = 3.75 - falloff_contribution;
            let clamped = value.max(0.0);
            usage[col][idx] += clamped;
        }
    }

    fn get_key_usage_400(k: i32, _total_time: i32, note_seq: &[(i32, i32, i32)], base_corners: &[f64]) -> Vec<Vec<f64>> {
        let mut usage = vec![vec![0.0; base_corners.len()]; k as usize];
        for &(col, h, tail) in note_seq {
            let start_time = h.max(0) as f64;
            let end_time = if tail < 0 {
                h as f64
            } else {
                (tail as f64).min(base_corners[base_corners.len() - 1].max(0.0))
            };
            let left400_idx = Self::bisect_left(base_corners, start_time - 400.0);
            let left_idx = Self::bisect_left(base_corners, start_time);
            let right_idx = Self::bisect_left(base_corners, end_time);
            let right400_idx = Self::bisect_left(base_corners, end_time + 400.0);

            let duration = end_time - start_time;
            let clamped_duration = duration.min(1500.0);
            let extension = clamped_duration / 150.0;
            let contribution = 3.75 + extension;

            for idx in left_idx..right_idx {
                usage[col as usize][idx] += contribution;
            }

            Self::add_falloff_contribution(&mut usage, col as usize, base_corners, left400_idx..left_idx, start_time);
            Self::add_falloff_contribution(&mut usage, col as usize, base_corners, right_idx..right400_idx, end_time);
        }
        usage
    }

    fn compute_anchor(k: i32, key_usage_400: &[Vec<f64>], base_corners: &[f64]) -> Vec<f64> {
        let mut anchor = vec![0.0; base_corners.len()];
        for i in 0..base_corners.len() {
            let mut counts: Vec<f64> = (0..k as usize).map(|col| key_usage_400[col][i]).collect();
            counts.sort_by(|a, b| b.partial_cmp(a).unwrap());
            let non_zero: Vec<f64> = counts.into_iter().filter(|&c| c > 0.0).collect();
            if non_zero.len() <= 1 {
                anchor[i] = 0.0;
                continue;
            }
            let mut walk = 0.0;
            let mut max_walk = 0.0;
            for idx in 0..non_zero.len() - 1 {
                let current = non_zero[idx];
                let next = non_zero[idx + 1];
                let ratio = next / current;
                let offset = 0.5 - ratio;
                let offset_penalty = 4.0 * offset * offset;
                let damping = 1.0 - offset_penalty;
                walk += current * damping;
                max_walk += current;
            }
            let value = walk / max_walk.max(1e-9);
            anchor[i] = 1.0 + (value - 0.18).min(5.0 * (value - 0.22).powf(3.0));
        }
        anchor
    }

    fn compute_jbar(k: i32, note_seq_by_column: &[Vec<(i32, i32, i32)>], base_corners: &[f64], x: f64) -> (Vec<Vec<f64>>, Vec<f64>) {
        let mut j_ks = vec![vec![0.0; base_corners.len()]; k as usize];
        let mut delta_ks = vec![vec![1e9; base_corners.len()]; k as usize];

        for col in 0..k as usize {
            let notes = &note_seq_by_column[col];
            for i in 0..notes.len().saturating_sub(1) {
                let (_, h1, _) = notes[i];
                let (_, h2, _) = notes[i + 1];
                let delta = 0.001 * (h2 - h1) as f64;

                if delta < 1e-9 { continue; }

                let x_pow_025 = x.powf(0.25);
                let lambda1_x = 0.11 * x_pow_025;
                let abs_delta = (delta - 0.08).abs();
                let temp = 0.15 + abs_delta;
                let temp4 = temp * temp * temp * temp;
                let jack = 1.0 - 7e-5 / temp4;
                let val = 1.0 / (delta * (delta + lambda1_x)) * jack;

                let left_idx = base_corners.partition_point(|&x| x < h1 as f64);
                let right_idx = base_corners.partition_point(|&x| x < h2 as f64);

                for j in left_idx..right_idx {
                    j_ks[col][j] = val;
                    delta_ks[col][j] = f64::min(delta_ks[col][j], delta);
                }
            }
        }

        // Smooth each column's J_ks
        let mut jbar_ks = vec![vec![0.0; base_corners.len()]; k as usize];
        for col in 0..k as usize {
            jbar_ks[col] = Self::smooth_on_corners(&j_ks[col], base_corners, 500.0, 0.001, SmoothMode::Sum);
        }

        // Aggregate across columns
        let mut jbar = vec![0.0; base_corners.len()];
        for i in 0..base_corners.len() {
            let mut vals = vec![];
            let mut weights = vec![];
            for col in 0..k as usize {
                let val = jbar_ks[col][i].max(0.0);
                vals.push(val.powf(5.0));
                weights.push(1.0 / f64::max(delta_ks[col][i], 1e-9));
            }
            let weighted_sum: f64 = vals.iter().zip(weights.iter()).map(|(v, w)| v * w).sum();
            let total_weight: f64 = weights.iter().sum();
            let combined = if total_weight <= 0.0 { 0.0 } else { weighted_sum / total_weight };
            jbar[i] = combined.max(0.0).powf(0.2);
        }

        (delta_ks, jbar)
    }

    fn compute_xbar(k: i32, x: f64, note_seq_by_column: &[Vec<(i32, i32, i32)>], active_columns: &[Vec<usize>], base_corners: &[f64], cross: &[f64]) -> Vec<f64> {
        // `cross` holds k + 1 coefficients for this key mode; the caller validated its length.
        let matrix = cross;

        let mut x_ks = vec![vec![0.0; base_corners.len()]; (k + 1) as usize];
        let mut fast_cross = vec![vec![0.0; base_corners.len()]; (k + 1) as usize];

        for col in 0..(k + 1) as usize {
            let mut notes_in_pair = vec![];
            if col == 0 {
                if !note_seq_by_column.is_empty() {
                    notes_in_pair = note_seq_by_column[0].clone();
                }
            } else if col == k as usize {
                if !note_seq_by_column.is_empty() {
                    notes_in_pair = note_seq_by_column[k as usize - 1].clone();
                }
            } else {
                let left_col = col - 1;
                let right_col = col;
                let mut left_notes = if left_col < note_seq_by_column.len() { note_seq_by_column[left_col].clone() } else { vec![] };
                let mut right_notes = if right_col < note_seq_by_column.len() { note_seq_by_column[right_col].clone() } else { vec![] };
                left_notes.append(&mut right_notes);
                left_notes.sort_by(|a, b| a.1.cmp(&b.1));
                notes_in_pair = left_notes;
            }

            for i in 1..notes_in_pair.len() {
                let (_, h1, _) = notes_in_pair[i - 1];
                let (_, h2, _) = notes_in_pair[i];
                let delta = 0.001 * (h2 - h1) as f64;
                let max_xd = x.max(delta);
                let mut val = 0.16 / (max_xd * max_xd);

                let left_idx = base_corners.partition_point(|&x| x < h1 as f64);
                let right_idx = base_corners.partition_point(|&x| x < h2 as f64);

                let idx_start = left_idx.min(active_columns.len().saturating_sub(1));
                let idx_end = right_idx.min(active_columns.len().saturating_sub(1));

                let condition1 = if col == 0 { true } else { !active_columns[idx_start].contains(&(col - 1)) && !active_columns[idx_end].contains(&(col - 1)) };
                let condition2 = !active_columns[idx_start].contains(&col) && !active_columns[idx_end].contains(&col);
                if condition1 || condition2 {
                    val *= 1.0 - matrix[col];
                }

                for j in left_idx..right_idx {
                    x_ks[col][j] = val;
                    fast_cross[col][j] = (0.4 * (delta.max(0.06).max(0.75 * x)).powf(-2.0) - 80.0).max(0.0);
                }
            }
        }

        let mut x_base = vec![0.0; base_corners.len()];
        for i in 0..base_corners.len() {
            for col in 0..(k + 1) as usize {
                x_base[i] += x_ks[col][i] * matrix[col];
            }
            for col in 0..k as usize {
                let left_contrib = fast_cross[col][i] * matrix[col];
                let right_contrib = fast_cross[col + 1][i] * matrix[col + 1];
                x_base[i] += (left_contrib * right_contrib).max(0.0).sqrt();
            }
        }

        Self::smooth_on_corners(&x_base, base_corners, 500.0, 0.001, SmoothMode::Sum)
    }

    fn ln_sum(a: f64, b: f64, ln_rep: &(Vec<f64>, Vec<f64>, Vec<f64>)) -> f64 {
        let (points, cumsum, values) = ln_rep;

        // Upper bound - 1, as in the reference: the segment containing a point is the one that starts at or before it.
        let i = (points.partition_point(|&x| x <= a) as i64) - 1;
        let j = (points.partition_point(|&x| x <= b) as i64) - 1;

        let start = i.max(0) as usize;
        let end = (j.max(0) as usize).max(start);

        if end >= values.len() || start >= values.len() {
            return 0.0;
        }

        if start == end {
            return (b - a) * values[start];
        }

        let mut total = 0.0;
        total += (points[start + 1] - a) * values[start];
        total += cumsum[end] - cumsum[start + 1];
        total += (b - points[end]) * values[end];

        total
    }

    /// One pass over the note sequence producing the three bar curves the result needs.
    ///
    /// * `burst`: no hold contribution, neutral anchor.
    /// * `ln_usage`: hold contribution, neutral anchor.
    /// * `anchored`: hold contribution, real anchor - this is the curve the star rating itself uses.
    ///
    /// A neutral anchor of `1.0` makes the "min(anchored, limit)" clamp a no-op, so those two curves are the raw
    /// increment; that is why they can share the loop instead of repeating it three times.
    fn compute_pbar_variants(
        x: f64,
        note_seq: &[(i32, i32, i32)],
        ln_rep: Option<&(Vec<f64>, Vec<f64>, Vec<f64>)>,
        anchor: &[f64],
        base_corners: &[f64],
        ln_multiplier: f64,
    ) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let n = base_corners.len();
        let mut p_burst = vec![0.0; n];
        let mut p_ln = vec![0.0; n];
        let mut p_anchored = vec![0.0; n];

        for i in 0..note_seq.len().saturating_sub(1) {
            let (_, h1, _) = note_seq[i];
            let (_, h2, _) = note_seq[i + 1];
            let delta_time = (h2 - h1) as f64;

            if delta_time < 1e-9 {
                let inv_x = 1.0 / x.max(1e-6);
                let spike_base = 0.02 * (4.0 * inv_x - 24.0);

                // A non-positive base has no fourth root; the reference skips the spike entirely.
                if spike_base <= 0.0 {
                    continue;
                }

                let spike = 1000.0 * spike_base.powf(0.25);
                let left_idx = base_corners.partition_point(|&c| c < h1 as f64);
                let right_idx = base_corners.partition_point(|&c| c <= h1 as f64);

                for j in left_idx..right_idx {
                    if j < n {
                        p_burst[j] += spike;
                        p_ln[j] += spike;
                        p_anchored[j] += spike;
                    }
                }

                continue;
            }

            let delta = 0.001 * delta_time;
            let ln_sum_val = if let Some(rep) = ln_rep {
                Self::ln_sum(h1 as f64, h2 as f64, rep)
            } else {
                0.0
            };
            let ln_value = 1.0 + ln_multiplier * 0.001 * ln_sum_val;

            let mut b_val = 1.0;
            let temp_val = 7.5 / delta;
            if temp_val > 160.0 && temp_val < 360.0 {
                // distance is squared, as in the reference; using the signed value flips the sign of the correction.
                let shifted = temp_val - 160.0;
                let distance = temp_val - 360.0;
                b_val = 1.0 + 1.7e-7 * shifted * distance * distance;
            }

            let (inc_burst, inc_ln) = if delta < 2.0 * x / 3.0 {
                let temp = 1.0 - 24.0 * (delta - x / 2.0).powi(2) / x;
                let magnitude = 1.0 / delta * (0.08 / x * temp).powf(0.25);
                (magnitude * b_val.max(1.0), magnitude * b_val.max(ln_value))
            } else {
                let temp = 1.0 - 24.0 * (x / 6.0).powi(2) / x;
                let magnitude = 1.0 / delta * (0.08 / x * temp).powf(0.25);
                (magnitude * b_val.max(1.0), magnitude * b_val.max(ln_value))
            };

            let limit_ln = inc_ln.max(inc_ln * 2.0 - 10.0);

            let left_idx = base_corners.partition_point(|&c| c < h1 as f64);
            let right_idx = base_corners.partition_point(|&c| c < h2 as f64);

            for j in left_idx..right_idx {
                if j >= n {
                    break;
                }

                p_burst[j] += inc_burst;
                p_ln[j] += inc_ln;

                let anchored_value = if j < anchor.len() { inc_ln * anchor[j] } else { inc_ln };
                p_anchored[j] += anchored_value.min(limit_ln);
            }
        }

        (
            Self::smooth_on_corners(&p_burst, base_corners, 500.0, 0.001, SmoothMode::Sum),
            Self::smooth_on_corners(&p_ln, base_corners, 500.0, 0.001, SmoothMode::Sum),
            Self::smooth_on_corners(&p_anchored, base_corners, 500.0, 0.001, SmoothMode::Sum),
        )
    }

    fn compute_abar(_k: i32, delta_ks: &[Vec<f64>], active_columns: &[Vec<usize>], a_corners: &[f64], base_corners: &[f64]) -> Vec<f64> {
        let mut a_step = vec![1.0; a_corners.len()];
        for i in 0..a_corners.len() {
            let s = a_corners[i];
            let idx = Self::bisect_left(base_corners, s);
            let idx = idx.min(active_columns.len().saturating_sub(1));
            let cols = &active_columns[idx];
            for j in 0..cols.len().saturating_sub(1) {
                let c0 = cols[j];
                let c1 = cols[j + 1];
                let delta_gap = (delta_ks[c0][idx] - delta_ks[c1][idx]).abs();
                let max_delta = delta_ks[c0][idx].max(delta_ks[c1][idx]);
                let offset = (max_delta - 0.11).max(0.0);
                let offset_contribution = 0.4 * offset;
                let diff = delta_gap + offset_contribution;
                if diff < 0.02 {
                    let factor_base = max_delta;
                    let factor_contribution = 0.5 * factor_base;
                    let factor = 0.75 + factor_contribution;
                    a_step[i] *= factor.min(1.0);
                } else if diff < 0.07 {
                    let factor_base = max_delta;
                    let growth = 5.0 * diff;
                    let factor_contribution = 0.5 * factor_base;
                    let factor = 0.65 + growth + factor_contribution;
                    a_step[i] *= factor.min(1.0);
                }
            }
        }
        Self::smooth_on_corners(&a_step, a_corners, 250.0, 0.0, SmoothMode::Average)
    }

    fn compute_rbar(x: f64, note_seq_by_column: &[Vec<(i32, i32, i32)>], tail_seq: &[(i32, i32, i32)], base_corners: &[f64]) -> Vec<f64> {
        let mut i_vals = vec![0.0; tail_seq.len()];

        for i in 0..tail_seq.len() {
            let (k, h_i, t_i) = tail_seq[i];
            let column_notes = if (k as usize) < note_seq_by_column.len() { &note_seq_by_column[k as usize] } else { &vec![] };

            let index = column_notes.iter().position(|&n| n.1 == h_i).unwrap_or(0);
            let next_note_time = if index + 1 < column_notes.len() { column_notes[index + 1].1 } else { 1000000000 };

            let i_h = 0.001 * ((t_i - h_i - 80) as f64).abs() / x;
            let i_t = 0.001 * ((next_note_time - t_i - 80) as f64).abs() / x;
            i_vals[i] = 2.0 / (2.0 + (-5.0 * (i_h - 0.75)).exp() + (-5.0 * (i_t - 0.75)).exp());
        }

        let mut r = vec![0.0; base_corners.len()];

        for i in 0..tail_seq.len().saturating_sub(1) {
            let (_, _, t_i) = tail_seq[i];
            let (_, _, t_next) = tail_seq[i + 1];
            let delta_r = 0.001 * (t_next - t_i) as f64;
            let r_val = 0.08 * delta_r.powf(-0.5) / x * (1.0 + 0.8 * (i_vals[i] + i_vals[i + 1]));

            let left_idx = base_corners.partition_point(|&x| x < t_i as f64);
            let right_idx = base_corners.partition_point(|&x| x < t_next as f64);

            for s in left_idx..right_idx {
                if s < r.len() {
                    r[s] = f64::max(r[s], r_val);
                }
            }
        }

        Self::smooth_on_corners(&r, base_corners, 500.0, 0.001, SmoothMode::Sum)
    }

    fn compute_c_and_ks(k: i32, note_seq: &[(i32, i32, i32)], key_usage: &[Vec<bool>], base_corners: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let mut note_times: Vec<f64> = note_seq.iter().map(|&(_, h, _)| h as f64).collect();
        note_times.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mut c_step = vec![0.0; base_corners.len()];
        for i in 0..base_corners.len() {
            let left = base_corners[i] - 500.0;
            let right = base_corners[i] + 500.0;
            let left_index = Self::bisect_left(&note_times, left);
            let right_index = Self::bisect_left(&note_times, right);
            c_step[i] = (right_index - left_index) as f64;
        }

        let mut ks_step = vec![0.0; base_corners.len()];
        for i in 0..base_corners.len() {
            let mut active_count = 0;
            for col in 0..k as usize {
                if key_usage[col][i] {
                    active_count += 1;
                }
            }
            ks_step[i] = active_count.max(1) as f64;
        }

        (c_step, ks_step)
    }

    fn interp_values(new_x: &[f64], old_x: &[f64], old_vals: &[f64]) -> Vec<f64> {
        new_x.iter().map(|&x| {
            if x <= old_x[0] {
                old_vals[0]
            } else if x >= old_x[old_x.len() - 1] {
                old_vals[old_vals.len() - 1]
            } else {
                let idx = old_x.partition_point(|&val| val < x) - 1;
                let frac = (x - old_x[idx]) / (old_x[idx + 1] - old_x[idx]);
                old_vals[idx] + frac * (old_vals[idx + 1] - old_vals[idx])
            }
        }).collect()
    }

    fn step_interp(new_x: &[f64], old_x: &[f64], old_vals: &[f64]) -> Vec<f64> {
        // Upper bound, not lower bound: at an exact corner the reference reads that corner's step, and the corners
        // are shared between the two arrays by construction.
        new_x.iter().map(|&x| {
            let idx = old_x.partition_point(|&val| val <= x).saturating_sub(1);
            old_vals[idx.min(old_vals.len() - 1)]
        }).collect()
    }

    fn cumulative_sum(x: &[f64], f: &[f64]) -> Vec<f64> {
        let mut f_cum = vec![0.0; x.len()];
        for i in 1..x.len() {
            f_cum[i] = f_cum[i - 1] + f[i - 1] * (x[i] - x[i - 1]);
        }
        f_cum
    }

    fn query_cumsum(q: f64, x: &[f64], f_cum: &[f64], f: &[f64]) -> f64 {
        if q <= x[0] {
            return 0.0;
        }
        if q >= x[x.len() - 1] {
            return f_cum[f_cum.len() - 1];
        }
        let i = x.partition_point(|&val| val < q).saturating_sub(1);
        f_cum[i] + f[i] * (q - x[i])
    }

    fn smooth_on_corners(f: &[f64], x: &[f64], window: f64, scale: f64, mode: SmoothMode) -> Vec<f64> {
        let f_cum = Self::cumulative_sum(x, f);
        let mut g = vec![0.0; f.len()];
        for i in 0..x.len() {
            let s = x[i];
            let a = (s - window).max(x[0]);
            let b = (s + window).min(x[x.len() - 1]);
            let integral = Self::query_cumsum(b, x, &f_cum, f) - Self::query_cumsum(a, x, &f_cum, f);
            if mode == SmoothMode::Average {
                g[i] = integral / (b - a).max(1e-9);
            } else {
                g[i] = integral * scale;
            }
        }
        g
    }

    fn calculate_final_sr(
        jbar: &[f64],
        xbar: &[f64],
        pbar: &[f64],
        abar: &[f64],
        rbar: &[f64],
        c_arr: &[f64],
        ks_arr: &[f64],
        note_seq: &[(i32, i32, i32)],
        ln_seq: &[(i32, i32, i32)],
        all_corners: &[f64],
        burst_bar: &[f64],
        ln_usage_bar: &[f64],
        config: SrConfig,
    ) -> SrOutput {
        // Compute gaps for all_corners
        let mut gaps = vec![0.0; all_corners.len()];
        if all_corners.len() > 1 {
            gaps[0] = (all_corners[1] - all_corners[0]) / 2.0;
            gaps[all_corners.len() - 1] = (all_corners[all_corners.len() - 1] - all_corners[all_corners.len() - 2]) / 2.0;
            for i in 1..all_corners.len() - 1 {
                gaps[i] = (all_corners[i + 1] - all_corners[i - 1]) / 2.0;
            }
        }

        // Compute effective weights
        let mut effective_weights = vec![0.0; all_corners.len()];
        for i in 0..all_corners.len() {
            effective_weights[i] = c_arr[i] * gaps[i];
        }

        let n = all_corners.len();

        // Per-corner difficulty and the component contributions it decomposes into. This is one loop over the same
        // values the C# breakdown computes, so the components cannot drift from the rating they belong to.
        let mut d_all = vec![0.0; n];
        let mut bracket_contribution = vec![0.0; n];
        let mut jack_contribution = vec![0.0; n];
        let mut burst_contribution = vec![0.0; n];
        let mut ln_usage_contribution = vec![0.0; n];
        let mut ln_release_contribution = vec![0.0; n];
        let mut anchor_contribution = vec![0.0; n];
        let mut bracket_alloc = vec![0.0; n];
        let mut jack_alloc = vec![0.0; n];
        let mut burst_alloc = vec![0.0; n];
        let mut ln_usage_alloc = vec![0.0; n];
        let mut ln_release_alloc = vec![0.0; n];
        let mut anchor_alloc = vec![0.0; n];

        for i in 0..n {
            let actual_term1 = Self::jack_term(jbar[i], abar[i], ks_arr[i]);
            let burst_only_term2 = Self::burst_term(burst_bar[i], 0.0, abar[i], c_arr[i]);
            let ln_usage_term2 = Self::burst_term(ln_usage_bar[i], 0.0, abar[i], c_arr[i]);
            let anchored_burst_term2 = Self::burst_term(pbar[i], 0.0, abar[i], c_arr[i]);
            let actual_term2 = Self::burst_term(pbar[i], rbar[i], abar[i], c_arr[i]);

            let without_burst_branch = Self::local_difficulty(xbar[i], abar[i], ks_arr[i], actual_term1, 0.0);
            let with_burst_only = Self::local_difficulty(xbar[i], abar[i], ks_arr[i], actual_term1, burst_only_term2);
            let with_ln_usage = Self::local_difficulty(xbar[i], abar[i], ks_arr[i], actual_term1, ln_usage_term2);
            let with_anchor = Self::local_difficulty(xbar[i], abar[i], ks_arr[i], actual_term1, anchored_burst_term2);
            let actual_difficulty = Self::local_difficulty(xbar[i], abar[i], ks_arr[i], actual_term1, actual_term2);

            d_all[i] = actual_difficulty;

            bracket_contribution[i] = (actual_difficulty - Self::local_difficulty(0.0, abar[i], ks_arr[i], actual_term1, actual_term2)).max(0.0);
            jack_contribution[i] = (actual_difficulty - Self::local_difficulty(xbar[i], abar[i], ks_arr[i], 0.0, actual_term2)).max(0.0);
            burst_contribution[i] = (with_burst_only - without_burst_branch).max(0.0);
            ln_usage_contribution[i] = (with_ln_usage - with_burst_only).max(0.0);
            anchor_contribution[i] = (with_anchor - with_ln_usage).max(0.0);
            ln_release_contribution[i] = (actual_difficulty - with_anchor).max(0.0);

            let total_marginal = bracket_contribution[i]
                + jack_contribution[i]
                + burst_contribution[i]
                + ln_usage_contribution[i]
                + ln_release_contribution[i]
                + anchor_contribution[i];

            if actual_difficulty <= 0.0 || total_marginal <= 0.0 {
                continue;
            }

            let scale = actual_difficulty / total_marginal;
            bracket_alloc[i] = bracket_contribution[i] * scale;
            jack_alloc[i] = jack_contribution[i] * scale;
            burst_alloc[i] = burst_contribution[i] * scale;
            ln_usage_alloc[i] = ln_usage_contribution[i] * scale;
            ln_release_alloc[i] = ln_release_contribution[i] * scale;
            anchor_alloc[i] = anchor_contribution[i] * scale;
        }

        let sr = Self::finalise_difficulty(&d_all, &effective_weights, note_seq, ln_seq, config.rescale_threshold);

        let mut components = [0.0; COMPONENT_COUNT];
        components[0] = Self::contribution_star(&bracket_alloc, &d_all, &effective_weights, note_seq, ln_seq, sr, config.rescale_threshold);
        components[1] = Self::contribution_star(&jack_alloc, &d_all, &effective_weights, note_seq, ln_seq, sr, config.rescale_threshold);
        components[2] = Self::contribution_star(&burst_alloc, &d_all, &effective_weights, note_seq, ln_seq, sr, config.rescale_threshold);
        components[3] = Self::contribution_star(&ln_usage_alloc, &d_all, &effective_weights, note_seq, ln_seq, sr, config.rescale_threshold);
        components[4] = Self::contribution_star(&ln_release_alloc, &d_all, &effective_weights, note_seq, ln_seq, sr, config.rescale_threshold);
        components[5] = Self::contribution_star(&anchor_alloc, &d_all, &effective_weights, note_seq, ln_seq, sr, config.rescale_threshold);

        Self::normalise_components(sr, &mut components);

        SrOutput { sr, components }
    }

    /// Coordination power shared by every term: unsupported columns zero the whole term rather than dividing.
    fn coordination_power(abar: f64, active_columns: f64) -> f64 {
        if abar <= 0.0 {
            0.0
        } else {
            abar.powf(3.0 / active_columns.max(1e-6))
        }
    }

    fn jack_term(jack_value: f64, abar: f64, active_columns: f64) -> f64 {
        let coordination = Self::coordination_power(abar, active_columns);
        let limited = jack_value.min(8.0 + 0.85 * jack_value);

        0.4 * safe_pow(coordination * limited, 1.5)
    }

    fn burst_term(burst_value: f64, release_value: f64, abar: f64, density: f64) -> f64 {
        let component = 0.8 * burst_value + release_value * 35.0 / (density + 8.0);
        let coordination = if abar <= 0.0 { 0.0 } else { abar.powf(2.0 / 3.0) };

        0.6 * safe_pow(coordination * component, 1.5)
    }

    fn local_difficulty(bracket: f64, abar: f64, active_columns: f64, jack_term: f64, burst_term: f64) -> f64 {
        let coordination = Self::coordination_power(abar, active_columns);
        let sum_terms = jack_term + burst_term;
        let s = if sum_terms <= 0.0 { 0.0 } else { sum_terms.powf(2.0 / 3.0) };
        let denominator = bracket + s + 1.0;
        let t_value = if denominator <= 0.0 { 0.0 } else { coordination * bracket / denominator };
        let sqrt_component = s.max(0.0).sqrt();
        let primary = 2.7 * sqrt_component * safe_pow(t_value, 1.5);

        primary + s * 0.27
    }

    /// One component's share of the rating, expressed as stars.
    fn contribution_star(
        contribution: &[f64],
        total_difficulty: &[f64],
        weights: &[f64],
        note_seq: &[(i32, i32, i32)],
        ln_seq: &[(i32, i32, i32)],
        total_sr: f64,
        threshold: f64,
    ) -> f64 {
        if contribution.is_empty() || total_difficulty.is_empty() || weights.is_empty() || total_sr <= 0.0 {
            return 0.0;
        }

        let count = contribution.len().min(total_difficulty.len()).min(weights.len());
        let mut clamped = vec![0.0; count];
        let mut local_weights = vec![0.0; count];

        for i in 0..count {
            clamped[i] = contribution[i].max(0.0).min(total_difficulty[i].max(0.0));
            local_weights[i] = weights[i];
        }

        if clamped.iter().all(|&v| v <= 0.0) {
            return 0.0;
        }

        let component_sr = Self::finalise_difficulty(&clamped, &local_weights, note_seq, ln_seq, threshold);

        round2(component_sr.clamp(0.0, total_sr))
    }

    fn normalise_components(total_sr: f64, components: &mut [f64; COMPONENT_COUNT]) {
        if total_sr <= 0.0 {
            *components = [0.0; COMPONENT_COUNT];
            return;
        }

        let mut max_component = 0.0_f64;

        for &value in components.iter() {
            max_component = max_component.max(value);
        }

        if max_component <= 0.0 {
            return;
        }

        let scale = total_sr / max_component;

        for value in components.iter_mut() {
            *value = round2((*value * scale).clamp(0.0, total_sr));
        }
    }

    fn finalise_difficulty(
        difficulties: &[f64],
        weights: &[f64],
        note_seq: &[(i32, i32, i32)],
        ln_seq: &[(i32, i32, i32)],
        rescale_threshold: f64,
    ) -> f64 {
        // Combine and sort by difficulty, stable sort to match C#
        let mut combined: Vec<(usize, f64, f64)> = difficulties.iter().enumerate().map(|(idx, &d)| (idx, d, weights[idx].max(0.0))).collect();
        combined.sort_by(|a, b| {
            use std::cmp::Ordering;
            if a.1.is_nan() && b.1.is_nan() {
                Ordering::Equal
            } else if a.1.is_nan() {
                Ordering::Greater
            } else if b.1.is_nan() {
                Ordering::Less
            } else {
                match a.1.partial_cmp(&b.1).unwrap() {
                    Ordering::Equal => a.0.cmp(&b.0),  // stable by original index
                    ord => ord,
                }
            }
        });
        
        if combined.is_empty() {
            return 0.0;
        }
        
        let sorted_d: Vec<f64> = combined.iter().map(|(_, d, _)| *d).collect();
        let sorted_weights: Vec<f64> = combined.iter().map(|(_, _, w)| *w).collect();

        // Cumulative weights
        let mut cumulative = vec![0.0; sorted_weights.len()];
        cumulative[0] = sorted_weights[0];
        for i in 1..sorted_weights.len() {
            cumulative[i] = cumulative[i - 1] + sorted_weights[i];
        }

        let total_weight = cumulative.last().unwrap().max(1e-9);
        let norm: Vec<f64> = cumulative.iter().map(|&v| v / total_weight).collect();

        let targets = [0.945, 0.935, 0.925, 0.915, 0.845, 0.835, 0.825, 0.815];

        // Percentile 93
        let mut percentile93 = 0.0;
        for i in 0..4 {
            let index = Self::bisect_left(&norm, targets[i]).min(sorted_d.len() - 1);
            percentile93 += sorted_d[index];
        }
        percentile93 /= 4.0;

        // Percentile 83
        let mut percentile83 = 0.0;
        for i in 4..8 {
            let index = Self::bisect_left(&norm, targets[i]).min(sorted_d.len() - 1);
            percentile83 += sorted_d[index];
        }
        percentile83 /= 4.0;

        // Weighted mean
        let mut weighted_mean_numerator = 0.0;
        for i in 0..sorted_d.len() {
            weighted_mean_numerator += sorted_d[i].powf(5.0) * sorted_weights[i];
        }
        let weighted_mean = (weighted_mean_numerator / total_weight).max(0.0).powf(0.2);

        // SR calculation
        let top_component = 0.25 * 0.88 * percentile93;
        let middle_component = 0.2 * 0.94 * percentile83;
        let mean_component = 0.55 * weighted_mean;
        let mut sr = top_component + middle_component + mean_component;
        sr = sr.powf(1.0) / 8.0_f64.powf(1.0) * 8.0;

        // Total notes adjustment
        let mut total_notes = note_seq.len() as f64;
        for &(_, h, t) in ln_seq {
            if t >= 0 {
                let len = (t - h).min(1000) as f64;
                total_notes += 0.5 * (len / 200.0);
            }
        }

        sr *= total_notes / (total_notes + 60.0);
        sr = Self::rescale_high(sr, rescale_threshold);
        sr *= 0.975;

        sr
    }

    fn bisect_left(arr: &[f64], target: f64) -> usize {
        let mut low = 0;
        let mut high = arr.len();

        while low < high {
            let mid = (low + high) / 2;
            if arr[mid] < target {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        low
    }

    fn rescale_high(sr: f64, threshold: f64) -> f64 {
        if sr <= threshold {
            sr
        } else {
            threshold + (sr - threshold) * (1.0 / 1.2)
        }
    }
}

/// Round to two decimals, half away from zero - the rounding the radar axes use.
pub fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn safe_pow(value: f64, exponent: f64) -> f64 {
    if value <= 0.0 { 0.0 } else { value.powf(exponent) }
}

/// `(column, head_ms, tail_ms)` from a parsed `.osu`, with `tail = -1` for a tap.
fn build_note_seq(data: &ParsedData) -> Vec<SrNote> {
    data.columns
        .iter()
        .enumerate()
        .map(|(i, &col)| {
            let h = data.note_starts[i];
            let t = if data.note_types[i] == 128 { data.note_ends[i] } else { -1 };
            (col, h, t)
        })
        .collect()
}
