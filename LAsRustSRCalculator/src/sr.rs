use crate::parser::ParsedData;

pub struct SRCalculator;

impl SRCalculator {
    pub fn calculate_sr_from_parsed_data(data: &ParsedData) -> Result<f64, String> {
        let od = data.od;
        let k = data.column_count;

        if k > 18 || k < 1 || (k > 10 && k % 2 == 1) {
            return Err("Unsupported key count".to_string());
        }

        // Build note_seq as in Python: list of (column, head_time, tail_time)
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
        let t = if t > 1000000 { 1000000 } else { t };

        let (all_corners, base_corners, a_corners) = Self::get_corners(t, &note_seq);

        // Get key usage
        let key_usage = Self::get_key_usage(&base_corners, &note_seq);
        let base_corners_len = base_corners.len();

        let key_usage_400 = Self::get_key_usage_400(k, &note_seq, &base_corners);
        let anchor = Self::compute_anchor(k, &key_usage_400, &base_corners);

        let granularity = 1;
        let all_corners: Vec<f64> = (0..t).step_by(granularity).map(|i| i as f64).collect();
        let gap = granularity as f64;
        let effective_weights: Vec<f64> = all_corners.iter().map(|&c| {
            note_seq.iter().filter(|&(_, h, tail)| *h <= c as i32 && (if *tail < 0 { *h } else { *tail }) >= c as i32).count() as f64 * gap
        }).collect();
        let d_arr: Vec<f64> = effective_weights.iter().map(|&w| w / 1000.0).collect();

        // Compute Jbar and delta_ks
        let (delta_ks, jbar) = Self::compute_jbar(k, &note_seq_by_column, &base_corners, x);

        // Compute Xbar
        let active_columns = Self::derive_active_columns(&key_usage);

        let xbar = Self::compute_xbar(k, x, &note_seq_by_column, &active_columns, &base_corners);

        // Compute Pbar
        let ln_rep = if !ln_seq.is_empty() { Some(Self::build_ln_representation(&ln_seq, t)) } else { None };

        let pbar = Self::compute_pbar(x, &note_seq, ln_rep.as_ref(), &anchor, &base_corners);

        // Compute Abar
        let abar = Self::compute_abar(k, &note_seq_by_column, &active_columns, &delta_ks, &a_corners, &base_corners);

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
        for &(k, h, tail) in ln_seq {
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
    fn derive_active_columns(key_usage: &[f64]) -> Vec<Vec<usize>> {
        let len = key_usage.len();
        let mut active_columns = vec![vec![]; len];
        for i in 0..len {
            if key_usage[i] > 0.0 {
                active_columns[i].push(0); // Since it's 1D, only one "column"
            }
        }
        active_columns
    }

    fn get_key_usage(base_corners: &[f64], note_seq: &[(i32, i32, i32)]) -> Vec<f64> {
        let mut key_usage = vec![0.0; base_corners.len()];
        for &(lane, h, tail) in note_seq {
            let h_f = h as f64;
            let tail_f = if tail >= 0 { tail as f64 } else { h_f };
            for (i, &corner) in base_corners.iter().enumerate() {
                if corner >= h_f && corner <= tail_f {
                    key_usage[i] += 1.0;
                }
            }
        }
        key_usage
    }

    fn get_key_usage_400(k: i32, note_seq: &[(i32, i32, i32)], base_corners: &[f64]) -> Vec<Vec<bool>> {
        let mut key_usage_400 = vec![vec![false; base_corners.len()]; k as usize];
        for &(col, h, tail) in note_seq {
            let start_time = h as f64;
            let end_time = if tail < 0 { h as f64 } else { tail as f64 };
            let left400_idx = base_corners.partition_point(|&x| x < (start_time - 400.0));
            let left_idx = base_corners.partition_point(|&x| x < start_time);
            let right_idx = base_corners.partition_point(|&x| x < end_time);
            let right400_idx = base_corners.partition_point(|&x| x < (end_time + 400.0));

            // Add 3.75 + length contribution
            let length = (end_time - start_time).min(1500.0);
            for i in left_idx..right_idx {
                key_usage_400[col as usize][i] = true;
            }

            // Left fade
            for i in left400_idx..left_idx {
                let dist = start_time - base_corners[i];
                if 3.75 - 3.75 / 400.0 / 400.0 * dist * dist > 0.0 {
                    key_usage_400[col as usize][i] = true;
                }
            }

            // Right fade
            for i in right_idx..right400_idx {
                let dist = base_corners[i] - end_time;
                if 3.75 - 3.75 / 400.0 / 400.0 * dist * dist > 0.0 {
                    key_usage_400[col as usize][i] = true;
                }
            }
        }
        key_usage_400
    }

    fn compute_anchor(k: i32, key_usage_400: &[Vec<bool>], base_corners: &[f64]) -> Vec<f64> {
        let mut anchor = vec![0.0; base_corners.len()];
        for i in 0..base_corners.len() {
        let mut counts: Vec<f64> = (0..k as usize).map(|col| if key_usage_400[col][i] { 1.0 } else { 0.0 }).collect();
            counts.sort_by(|a, b| b.partial_cmp(a).unwrap());
            let nonzero: Vec<f64> = counts.into_iter().filter(|&x| x != 0.0).collect();
            if nonzero.len() > 1 {
                let mut walk = 0.0;
                let mut max_walk = 0.0;
                for j in 0..nonzero.len() - 1 {
                    let factor = 1.0 - 4.0 * (0.5 - nonzero[j + 1] / nonzero[j]).powi(2);
                    walk += nonzero[j] * factor;
                    max_walk += nonzero[j];
                }
                anchor[i] = if max_walk > 0.0 { walk / max_walk } else { 0.0 };
            }
        }
        // Apply scaling
        for i in 0..anchor.len() {
            anchor[i] = 1.0 + (anchor[i] - 0.18).min(5.0 * (anchor[i] - 0.22).powf(3.0));
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
                    delta_ks[col][j] = delta;
                }
            }
        }

        // Smooth each column's J_ks
        let mut jbar_ks = vec![vec![0.0; base_corners.len()]; k as usize];
        for col in 0..k as usize {
            jbar_ks[col] = Self::smooth_on_corners(&j_ks[col], base_corners, 500.0);
        }

        // Aggregate across columns
        let mut jbar = vec![0.0; base_corners.len()];
        for i in 0..base_corners.len() {
            let mut vals = vec![];
            let mut weights = vec![];
            for col in 0..k as usize {
                let val = jbar_ks[col][i].max(0.0);
                if val > 0.0 {
                    vals.push(val.powf(5.0));
                    weights.push(1.0 / delta_ks[col][i].max(1e-9));
                }
            }
            if !weights.is_empty() {
                let weighted_sum: f64 = vals.iter().zip(weights.iter()).map(|(v, w)| v * w).sum();
                let total_weight: f64 = weights.iter().sum();
                jbar[i] = (weighted_sum / total_weight).powf(1.0 / 5.0);
            }
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
                let val = 0.16 / (max_xd * max_xd);

                let left_idx = base_corners.partition_point(|&x| x < h1 as f64);
                let right_idx = base_corners.partition_point(|&x| x < h2 as f64);

                let mut apply_coeff = true;
                if left_idx < active_columns.len() && right_idx < active_columns.len() {
                    let left_active = &active_columns[left_idx];
                    let right_active = &active_columns[right_idx];
                    if !left_active.contains(&(col.saturating_sub(1))) && !right_active.contains(&(col.saturating_sub(1))) ||
                       !left_active.contains(&col) && !right_active.contains(&col) {
                        apply_coeff = false;
                    }
                }

                if !apply_coeff {
                    // val *= (1 - matrix[col]);
                }

                for j in left_idx..right_idx {
                    x_ks[col][j] = val;
                    fast_cross[col][j] = (0.4 * (delta.max(0.06).max(0.75 * x)).powf(-2.0)).max(0.0) - 80.0;
                }
            }
        }

        let mut x_base = vec![0.0; base_corners.len()];
        for i in 0..base_corners.len() {
            for col in 0..(k + 1) as usize {
                x_base[i] += x_ks[col][i] * matrix[col];
            }
            for col in 0..k as usize {
                x_base[i] += (fast_cross[col][i].sqrt() * matrix[col] * fast_cross[col + 1][i].sqrt() * matrix[col + 1]).sqrt();
            }
        }

        Self::smooth_on_corners(&x_base, base_corners, 500.0)
    }

    fn ln_sum(a: f64, b: f64, ln_rep: &(Vec<f64>, Vec<f64>, Vec<f64>)) -> f64 {
        let (points, cumsum, values) = ln_rep;
        let i = points.partition_point(|&x| x < a) - 1;
        let j = points.partition_point(|&x| x < b) - 1;

        let mut total = 0.0;
        if i == j {
            total = (b - a) * values[i];
        } else {
            total += (points[i + 1] - a) * values[i];
            total += cumsum[j] - cumsum[i + 1];
            total += (b - points[j]) * values[j];
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

        Self::smooth_on_corners(&p, base_corners, 500.0)
    }

    fn compute_abar(k: i32, note_seq_by_column: &[Vec<(i32, i32, i32)>], active_columns: &[Vec<usize>], delta_ks: &[Vec<f64>], a_corners: &[f64], base_corners: &[f64]) -> Vec<f64> {
        let mut dks = vec![vec![0.0; base_corners.len()]; (k - 1) as usize];
        for i in 0..base_corners.len() {
            let cols = &active_columns[i];
            for j in 0..cols.len().saturating_sub(1) {
                let k0 = cols[j];
                let k1 = cols[j + 1];
                dks[k0][i] = (delta_ks[k0][i] - delta_ks[k1][i]).abs() + 0.4 * (delta_ks[k0][i].max(delta_ks[k1][i]) - 0.11).max(0.0);
            }
        }

        let mut a = vec![1.0; a_corners.len()];

        for i in 0..a_corners.len() {
            let s = a_corners[i];
            let idx = base_corners.partition_point(|&x| x < s).saturating_sub(1);
            let cols = &active_columns[idx.min(active_columns.len() - 1)];
            for j in 0..cols.len().saturating_sub(1) {
                let k0 = cols[j];
                let k1 = cols[j + 1];
                let dk = dks[k0][idx];
                let max_delta = delta_ks[k0][idx].max(delta_ks[k1][idx]);
                if dk < 0.02 {
                    a[i] *= (0.75 + 0.5 * max_delta).min(1.0);
                } else if dk < 0.07 {
                    a[i] *= (0.65 + 5.0 * dk + 0.5 * max_delta).min(1.0);
                }
            }
        }

        Self::smooth_on_corners(&a, a_corners, 250.0)
    }

    fn compute_rbar(x: f64, note_seq_by_column: &[Vec<(i32, i32, i32)>], tail_seq: &[(i32, i32, i32)], base_corners: &[f64]) -> Vec<f64> {
        let mut i_vals = vec![0.0; tail_seq.len()];

        for i in 0..tail_seq.len() {
            let (k, h_i, t_i) = tail_seq[i];
            let column_notes = if (k as usize) < note_seq_by_column.len() { &note_seq_by_column[k as usize] } else { &vec![] };

            let mut next_note_time = i32::MAX;
            for &(_, h_j, _) in column_notes {
                if h_j > t_i && h_j < next_note_time {
                    next_note_time = h_j;
                }
            }

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
                    r[s] = r_val;
                }
            }
        }

        Self::smooth_on_corners(&r, base_corners, 500.0)
    }

    fn compute_c_and_ks(k: i32, note_seq: &[(i32, i32, i32)], key_usage: &[f64], base_corners: &[f64]) -> (Vec<f64>, Vec<f64>) {
        let mut note_hit_times: Vec<f64> = note_seq.iter().map(|&(_, h, _)| h as f64).collect();
        note_hit_times.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mut c_step = vec![0.0; base_corners.len()];
        for i in 0..base_corners.len() {
            let low = base_corners[i] - 500.0;
            let high = base_corners[i] + 500.0;
            let left = note_hit_times.partition_point(|&time| time < low);
            let right = note_hit_times.partition_point(|&time| time < high);
            c_step[i] = (right - left) as f64;
        }

        let mut ks_step = vec![0.0; base_corners.len()];
        for i in 0..base_corners.len() {
            ks_step[i] = key_usage[i].max(1.0);
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
        let i = x.partition_point(|&val| val < q) - 1;
        f_cum[i] + f[i] * (q - x[i])
    }

    fn smooth_on_corners(f: &[f64], x: &[f64], window: f64) -> Vec<f64> {
        let f_cum = Self::cumulative_sum(x, f);
        let mut g = vec![0.0; f.len()];
        for i in 0..x.len() {
            let s = x[i];
            let a = (s - window).max(x[0]);
            let b = (s + window).min(x[x.len() - 1]);
            let val = Self::query_cumsum(b, x, &f_cum, f) - Self::query_cumsum(a, x, &f_cum, f);
            g[i] = 0.001 * val;
        }
        g
    }

    // fn calculate_final_sr(
    //     jbar: &[f64],
    //     xbar: &[f64],
    //     pbar: &[f64],
    //     abar: &[f64],
    //     rbar: &[f64],
    //     c_arr: &[f64],
    //     note_seq: &[(i32, i32, i32)],
    //     ln_seq: &[(i32, i32, i32)],
    //     all_corners: &[f64],
    // ) -> f64 {
    //     0.0
    // }

    fn calculate_final_sr(
        jbar: &[f64],
        xbar: &[f64],
        pbar: &[f64],
        abar: &[f64],
        rbar: &[f64],
        c_arr: &[f64],
        note_seq: &[(i32, i32, i32)],
        ln_seq: &[(i32, i32, i32)],
        all_corners: &[f64],
    ) -> f64 {
        0.0
    }

    fn rescale_high(sr: f64) -> f64 {
        if sr <= 9.0 {
            sr
        } else {
            9.0 + (sr - 9.0) * (1.0 / 1.2)
        }
    }
}
