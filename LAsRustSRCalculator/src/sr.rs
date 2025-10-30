use crate::parser::ParsedData;

#[derive(Clone, Copy, PartialEq)]
enum SmoothMode {
    Sum,
    Average,
}

pub struct SRCalculator;

impl SRCalculator {
    pub fn calculate_sr_from_parsed_data(data: &ParsedData) -> Result<f64, String> {
        let od = data.od;
        let k = data.column_count;

        if k > 18 || k < 1 || (k > 10 && k % 2 == 1) {
            return Err("Unsupported key count".to_string());
        }

        // Build note_seq as (column, head_time, tail_time)
        let mut note_seq: Vec<(i32, i32, i32)> = data.columns.iter().enumerate().map(|(i, &col)| {
            let h = data.note_starts[i];
            let t = if data.note_types[i] == 128 { data.note_ends[i] } else { -1 };
            (col, h, t)
        }).collect();

        if note_seq.is_empty() {
            return Ok(0.0);
        }

        // Sort by (start_time, column) as in Python
        note_seq.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

        // Hit leniency x - exactly as in Python
        let x = 0.3 * ((64.5 - (od * 3.0).ceil()) / 500.0).sqrt();
        let x = x.min(0.6 * (x - 0.09) + 0.09);

        // Group notes by column
        let mut note_seq_by_column: Vec<Vec<(i32, i32, i32)>> = vec![vec![]; k as usize];
        for &note in &note_seq {
            note_seq_by_column[note.0 as usize].push(note);
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

        // Compute Xbar
        let xbar = Self::compute_xbar(k, x, &note_seq_by_column, &active_columns, &base_corners);

        // Compute Pbar
        let ln_rep = if !ln_seq.is_empty() { Some(Self::build_ln_representation(&ln_seq, t)) } else { None };

        let pbar = Self::compute_pbar(x, &note_seq, ln_rep.as_ref(), &anchor, &base_corners);

        // Compute Abar
        let abar = Self::compute_abar(k, &delta_ks, &active_columns, &a_corners, &base_corners);

        // Compute Rbar
        let rbar = Self::compute_rbar(x, &note_seq_by_column, &tail_seq, &base_corners);

        // Compute C and Ks
        let (c_arr, ks_arr) = Self::compute_c_and_ks(k, &note_seq, &key_usage, &base_corners);

        // Final SR calculation
        let jbar_interp = Self::interp_values(&all_corners, &base_corners, &jbar);
        let xbar_interp = Self::interp_values(&all_corners, &base_corners, &xbar);
        let pbar_interp = Self::interp_values(&all_corners, &base_corners, &pbar);
        let abar_interp = Self::interp_values(&all_corners, &a_corners, &abar);
        let rbar_interp = Self::interp_values(&all_corners, &base_corners, &rbar);
        let c_arr_interp = Self::step_interp(&all_corners, &base_corners, &c_arr);
        let ks_arr_interp = Self::step_interp(&all_corners, &base_corners, &ks_arr);

        let sr = Self::calculate_final_sr(
            &jbar_interp,
            &xbar_interp,
            &pbar_interp,
            &abar_interp,
            &rbar_interp,
            &c_arr_interp,
            &ks_arr_interp,
            &note_seq,
            &ln_seq,
            &all_corners,
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
            let end_time = if tail < 0 { h as f64 } else { tail as f64 };
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
            for i in 0..notes.len() - 1 {
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

    fn compute_xbar(k: i32, x: f64, note_seq_by_column: &[Vec<(i32, i32, i32)>], active_columns: &[Vec<usize>], base_corners: &[f64]) -> Vec<f64> {
        let cross_matrices: Vec<Vec<f64>> = vec![
            vec![-1.0],
            vec![0.075, 0.075],
            vec![0.125, 0.05, 0.125],
            vec![0.125, 0.125, 0.125, 0.125],
            vec![0.175, 0.25, 0.05, 0.25, 0.175],
            vec![0.175, 0.25, 0.175, 0.175, 0.25, 0.175],
            vec![0.225, 0.35, 0.25, 0.05, 0.25, 0.35, 0.225],
            vec![0.225, 0.35, 0.25, 0.225, 0.225, 0.25, 0.35, 0.225],
            vec![0.275, 0.45, 0.35, 0.25, 0.05, 0.25, 0.35, 0.45, 0.275],
            vec![0.275, 0.45, 0.35, 0.25, 0.275, 0.275, 0.25, 0.35, 0.45, 0.275],
            vec![0.325, 0.55, 0.45, 0.35, 0.25, 0.05, 0.25, 0.35, 0.45, 0.55, 0.325]
        ];

        let matrix = &cross_matrices[k as usize];
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
        let i = (points.partition_point(|&x| x < a) as i32) - 1;
        let j = (points.partition_point(|&x| x < b) as i32) - 1;
        let mut total = 0.0;
        if i == j {
            let idx = i.max(0) as usize;
            total = (b - a) * values[idx];
        } else {
            let i_idx = i.max(0) as usize;
            let start_a = if i < 0 { points[0] } else { points[i_idx + 1] };
            total += (start_a - a) * values[i_idx];
            let j_idx = j.max(0) as usize;
            total += cumsum[j_idx] - cumsum[i_idx + 1];
            total += (b - points[j_idx]) * values[j_idx];
        }
        total
    }

    fn compute_pbar(x: f64, note_seq: &[(i32, i32, i32)], ln_rep: Option<&(Vec<f64>, Vec<f64>, Vec<f64>)>, anchor: &[f64], base_corners: &[f64]) -> Vec<f64> {
        let mut p = vec![0.0; base_corners.len()];

        for i in 0..note_seq.len() - 1 {
            let (_, h1, _) = note_seq[i];
            let (_, h2, _) = note_seq[i + 1];
            let delta_time = (h2 - h1) as f64;

            if delta_time < 1e-9 {
                let spike = 1000.0 * (0.02 * (4.0 / x - 24.0)).powf(0.25);
                let left_idx = base_corners.partition_point(|&x| x < h1 as f64);
                let right_idx = base_corners.partition_point(|&x| x <= h1 as f64);
                for j in left_idx..right_idx {
                    if j < p.len() {
                        p[j] += spike;
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
            let v = 1.0 + 6.0 * 0.001 * ln_sum_val;

            let mut b_val = 1.0;
            let temp_val = 7.5 / delta;
            if temp_val > 160.0 && temp_val < 360.0 {
                b_val = 1.0 + 1.7e-7 * (temp_val - 160.0) * (temp_val - 360.0);
            }

            let inc = if delta < 2.0 * x / 3.0 {
                let temp = 1.0 - 24.0 * (delta - x / 2.0).powi(2) / x;
                1.0 / delta * (0.08 / x * temp).powf(0.25) * b_val.max(v)
            } else {
                let temp = 1.0 - 24.0 * (x / 6.0).powi(2) / x;
                1.0 / delta * (0.08 / x * temp).powf(0.25) * b_val.max(v)
            };

            let left_idx = base_corners.partition_point(|&x| x < h1 as f64);
            let right_idx = base_corners.partition_point(|&x| x < h2 as f64);

            for j in left_idx..right_idx {
                if j < anchor.len() && j < p.len() {
                    p[j] += (inc * anchor[j]).min(inc.max(inc * 2.0 - 10.0));
                }
            }
        }

        Self::smooth_on_corners(&p, base_corners, 500.0, 0.001, SmoothMode::Sum)
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
        new_x.iter().map(|&x| {
            let idx = old_x.partition_point(|&val| val < x).saturating_sub(1);
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
    ) -> f64 {
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

        // Compute d_all
        let mut d_all = vec![0.0; all_corners.len()];
        for i in 0..all_corners.len() {
            let abar_exp = 3.0 / ks_arr[i].max(1e-6);
            let abar_pow = if abar[i] <= 0.0 { 0.0 } else { abar[i].powf(abar_exp) };
            let min_candidate_contribution = 0.85 * jbar[i];
            let min_candidate = 8.0 + min_candidate_contribution;
            let min_j = jbar[i].min(min_candidate);
            let jack_component = abar_pow * min_j;
            let term1 = 0.4 * if jack_component <= 0.0 { 0.0 } else { jack_component.powf(1.5) };

            let scaled_p = 0.8 * pbar[i];
            let jack_penalty = rbar[i] * 35.0;
            let ratio = jack_penalty / (c_arr[i] + 8.0);
            let p_component = scaled_p + ratio;
            let power_base = (if abar[i] <= 0.0 { 0.0 } else { abar[i].powf(2.0 / 3.0) }) * p_component;
            let term2 = 0.6 * if power_base <= 0.0 { 0.0 } else { power_base.powf(1.5) };

            let sum_terms = term1 + term2;
            let s = if sum_terms <= 0.0 { 0.0 } else { sum_terms.powf(2.0 / 3.0) };
            let numerator = abar_pow * xbar[i];
            let denominator = xbar[i] + s + 1.0;
            let t_value = if denominator <= 0.0 { 0.0 } else { numerator / denominator };
            let sqrt_component = s.max(0.0).sqrt();
            let primary_impact = 2.7 * sqrt_component * (if t_value <= 0.0 { 0.0 } else { t_value.powf(1.5) });
            let secondary_impact = s * 0.27;

            d_all[i] = primary_impact + secondary_impact;
        }

        // Finalise difficulty
        Self::finalise_difficulty(&d_all, &effective_weights, note_seq, ln_seq)
    }

    fn finalise_difficulty(
        difficulties: &[f64],
        weights: &[f64],
        note_seq: &[(i32, i32, i32)],
        ln_seq: &[(i32, i32, i32)],
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
        sr = Self::rescale_high(sr);
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

    fn rescale_high(sr: f64) -> f64 {
        if sr <= 9.0 {
            sr
        } else {
            9.0 + (sr - 9.0) * (1.0 / 1.2)
        }
    }
}
