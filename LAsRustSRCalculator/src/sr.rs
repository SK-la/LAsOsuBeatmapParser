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

        // Get key usage
        let key_usage = Self::get_key_usage(k, t, &note_seq);

        let base_corners: Vec<f64> = (0..t).map(|i| i as f64).collect();
        let key_usage_400 = Self::get_key_usage_400(k, t, &note_seq, &base_corners);
        let anchor = Self::compute_anchor(k, &key_usage_400, &base_corners);

        let granularity = 1;
        let all_corners: Vec<f64> = (0..t).step_by(granularity).map(|i| i as f64).collect();
        let gap = granularity as f64;
        let effective_weights: Vec<f64> = all_corners.iter().map(|&c| {
            note_seq.iter().filter(|&(_, h, tail)| *h <= c as i32 && (if *tail < 0 { *h } else { *tail }) >= c as i32).count() as f64 * gap
        }).collect();
        let d_arr: Vec<f64> = effective_weights.iter().map(|&w| w / 1000.0).collect();

        // Compute Jbar and delta_ks
        let (delta_ks, jbar) = Self::compute_jbar(k, t, x, &note_seq_by_column);

        // Compute Xbar
        let xbar = Self::compute_xbar(k, t, x, &note_seq_by_column, &note_seq);

        // Compute Pbar
        let pbar = Self::compute_pbar(k, t, x, &note_seq, &ln_seq, &anchor);

        // Compute Abar
        let abar = Self::compute_abar(k, t, x, &note_seq_by_column, &key_usage, &delta_ks);

        // Compute Rbar
        let rbar = Self::compute_rbar(k, t, x, &note_seq_by_column, &tail_seq);

        // Compute C and Ks
        let (c_arr, _) = Self::compute_c_and_ks(k, t, &note_seq, &key_usage);

        // Final SR calculation
        Ok(Self::calculate_final_sr(
            &jbar,
            &xbar,
            &pbar,
            &abar,
            &rbar,
            &d_arr,
            &c_arr,
            &note_seq,
            &ln_seq,
        ))
    }

    fn get_key_usage(k: i32, t: i32, note_seq: &[(i32, i32, i32)]) -> Vec<Vec<bool>> {
        let mut key_usage = vec![vec![false; t as usize]; k as usize];

        for &(col, h, tail) in note_seq {
            let start_time = (h - 150).max(0) as usize;
            let end_time = if tail < 0 {
                (h + 150).min(t - 1) as usize
            } else {
                (tail + 150).min(t - 1) as usize
            };

            for j in start_time..=end_time {
                if j < key_usage[col as usize].len() {
                    key_usage[col as usize][j] = true;
                }
            }
        }

        key_usage
    }

    fn get_key_usage_400(k: i32, _t: i32, note_seq: &[(i32, i32, i32)], base_corners: &[f64]) -> Vec<Vec<f64>> {
        let mut key_usage_400 = vec![vec![0.0; base_corners.len()]; k as usize];
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
                key_usage_400[col as usize][i] += 3.75 + length / 150.0;
            }

            // Left fade
            for i in left400_idx..left_idx {
                let dist = start_time - base_corners[i];
                key_usage_400[col as usize][i] += 3.75 - 3.75 / 400.0 / 400.0 * dist * dist;
            }

            // Right fade
            for i in right_idx..right400_idx {
                let dist = base_corners[i] - end_time;
                key_usage_400[col as usize][i] += 3.75 - 3.75 / 400.0 / 400.0 * dist * dist;
            }
        }
        key_usage_400
    }

    fn compute_anchor(k: i32, key_usage_400: &[Vec<f64>], base_corners: &[f64]) -> Vec<f64> {
        let mut anchor = vec![0.0; base_corners.len()];
        for i in 0..base_corners.len() {
            let mut counts: Vec<f64> = (0..k as usize).map(|col| key_usage_400[col][i]).collect();
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

    fn compute_jbar(k: i32, t: i32, x: f64, note_seq_by_column: &[Vec<(i32, i32, i32)>]) -> (Vec<Vec<f64>>, Vec<f64>) {
        let mut j_ks = vec![vec![0.0; t as usize]; k as usize];
        let mut delta_ks = vec![vec![1e9; t as usize]; k as usize];

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

                let start = h1.max(0) as usize;
                let end = h2.min(t - 1) as usize;

                for j in start..end {
                    j_ks[col][j] = val;
                    delta_ks[col][j] = delta;
                }
            }
        }

        // Smooth each column's J_ks
        let mut jbar_ks = vec![vec![0.0; t as usize]; k as usize];
        for col in 0..k as usize {
            jbar_ks[col] = Self::smooth(&j_ks[col], t);
        }

        // Aggregate across columns
        let mut jbar = vec![0.0; t as usize];
        for i in 0..t as usize {
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

    fn compute_xbar(k: i32, t: i32, x: f64, note_seq_by_column: &[Vec<(i32, i32, i32)>], note_seq: &[(i32, i32, i32)]) -> Vec<f64> {
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
            vec![0.625, 0.55, 0.45, 0.35, 0.25, 0.05, 0.25, 0.35, 0.45, 0.55, 0.625],
            vec![-1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0],
            vec![0.8, 0.8, 0.8, 0.6, 0.4, 0.2, 0.05, 0.2, 0.4, 0.6, 0.8, 0.8, 0.8],
            vec![-1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0],
            vec![0.4, 0.4, 0.2, 0.2, 0.3, 0.3, 0.1, 0.1, 0.3, 0.3, 0.2, 0.2, 0.4, 0.4, 0.4],
            vec![-1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0],
            vec![0.4, 0.4, 0.2, 0.2, 0.4, 0.4, 0.2, 0.1, 0.1, 0.2, 0.4, 0.4, 0.2, 0.2, 0.4, 0.4, 0.4],
            vec![-1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0],
            vec![0.4, 0.4, 0.2, 0.4, 0.2, 0.4, 0.2, 0.3, 0.1, 0.1, 0.3, 0.2, 0.4, 0.2, 0.4, 0.2, 0.4, 0.4, 0.4],
        ];

        let matrix = &cross_matrices[k as usize];
        let mut x_ks = vec![vec![0.0; t as usize]; (k + 1) as usize];

        let mut active_columns_t = vec![vec![false; k as usize]; t as usize];
        for &(col, h, tail) in note_seq {
            let start_time = (h - 150).max(0) as usize;
            let end_time = if tail < 0 { (h + 150).min(t - 1) as usize } else { (tail + 150).min(t - 1) as usize };
            for s in start_time..=end_time {
                active_columns_t[s][col as usize] = true;
            }
        }

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

                let start = h1.max(0) as usize;
                let end = h2.min(t - 1) as usize;

                for j in start..end {
                    x_ks[col][j] = val;
                }
            }
        }

        let mut x_base = vec![0.0; t as usize];
        for i in 0..t as usize {
            for col in 0..(k + 1) as usize {
                x_base[i] += x_ks[col][i] * matrix[col];
            }
        }

        Self::smooth(&x_base, t)
    }

    fn compute_pbar(_k: i32, t: i32, x: f64, note_seq: &[(i32, i32, i32)], ln_seq: &[(i32, i32, i32)], anchor: &Vec<f64>) -> Vec<f64> {
        let mut ln_bodies = vec![0.0; t as usize];

        for &(_, h, tail) in ln_seq {
            let t1 = (h + 80).min(tail);
            for time in h..t1 {
                if time >= 0 && (time as usize) < ln_bodies.len() {
                    ln_bodies[time as usize] += 0.5;
                }
            }
            for time in t1..tail {
                if time >= 0 && (time as usize) < ln_bodies.len() {
                    ln_bodies[time as usize] += 1.0;
                }
            }
        }

        let mut prefix_sum_ln = vec![0.0; (t + 1) as usize];
        for i in 1..=t as usize {
            prefix_sum_ln[i] = prefix_sum_ln[i - 1] + ln_bodies[i - 1];
        }

        let mut p = vec![0.0; t as usize];

        for i in 0..note_seq.len() - 1 {
            let (_, h1, _) = note_seq[i];
            let (_, h2, _) = note_seq[i + 1];
            let delta_time = (h2 - h1) as f64;

            if delta_time < 1e-9 {
                let spike = 1000.0 * (0.02 * (4.0 / x - 24.0)).powf(0.25);
                let spike_start = h1.max(0) as usize;
                let spike_end = ((h1 + 1).min(t)) as usize;
                for j in spike_start..spike_end {
                    p[j] += spike;
                }
                continue;
            }

            let delta = 0.001 * delta_time;
            let ln_sum = prefix_sum_ln[h2.min(t) as usize] - prefix_sum_ln[h1.max(0) as usize];
            let v = 1.0 + 6.0 * 0.001 * ln_sum;

            let mut b_val = 1.0;
            let temp_val = 7.5 / delta;
            if temp_val > 160.0 && temp_val < 360.0 {
                let diff = temp_val - 160.0;
                let diff2 = temp_val - 360.0;
                b_val = 1.0 + 1.7e-7 * diff * diff2 * diff2;
            }

            let inc;
            if delta < 2.0 * x / 3.0 {
                let temp = 1.0 - 24.0 * (delta - x / 2.0) * (delta - x / 2.0) / x;
                inc = 1.0 / delta * (0.08 / x * temp).powf(0.25) * b_val.max(v);
            } else {
                let temp = 1.0 - 24.0 * (x / 6.0) * (x / 6.0) / x;
                inc = 1.0 / delta * (0.08 / x * temp).powf(0.25) * b_val.max(v);
            }

            let p_start = h1.max(0) as usize;
            let p_end = h2.min(t - 1) as usize;

            for j in p_start..p_end {
                p[j] += (inc * anchor[j]).min(inc.max(inc * 2.0 - 10.0));
            }
        }

        Self::smooth(&p, t)
    }

    fn compute_abar(k: i32, t: i32, _x: f64, _note_seq_by_column: &[Vec<(i32, i32, i32)>], key_usage: &[Vec<bool>], delta_ks: &[Vec<f64>]) -> Vec<f64> {
        let mut a = vec![1.0; t as usize];

        for s in 0..t as usize {
            let mut cols = vec![];
            for col in 0..k as usize {
                if key_usage[col][s] {
                    cols.push(col);
                }
            }

            for i in 0..cols.len().saturating_sub(1) {
                let col1 = cols[i];
                let col2 = cols[i + 1];

                let dk = (delta_ks[col1][s] - delta_ks[col2][s]).abs() +
                         (delta_ks[col1][s].max(delta_ks[col2][s]) - 0.3).max(0.0);

                let max_delta = delta_ks[col1][s].max(delta_ks[col2][s]);

                if dk < 0.02 {
                    a[s] *= (0.75 + 0.5 * max_delta).min(1.0);
                } else if dk < 0.07 {
                    a[s] *= (0.65 + 5.0 * dk + 0.5 * max_delta).min(1.0);
                }
            }
        }

        Self::smooth(&a, t)
    }

    fn compute_rbar(_k: i32, t: i32, x: f64, note_seq_by_column: &[Vec<(i32, i32, i32)>], tail_seq: &[(i32, i32, i32)]) -> Vec<f64> {
        let mut i_vals = vec![0.0; tail_seq.len()];

        for i in 0..tail_seq.len() {
            let (k, h_i, t_i) = tail_seq[i];
            let column_notes = if (k as usize) < note_seq_by_column.len() { &note_seq_by_column[k as usize] } else { &vec![] };

            // Find next note in column
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

        let mut r = vec![0.0; t as usize];

        for i in 0..tail_seq.len().saturating_sub(1) {
            let (_, _, t_i) = tail_seq[i];
            let (_, _, t_next) = tail_seq[i + 1];
            let delta_r = 0.001 * (t_next - t_i) as f64;
            let r_val = 0.08 * delta_r.powf(-0.5) / x * (1.0 + 0.8 * (i_vals[i] + i_vals[i + 1]));

            let start = t_i.max(0) as usize;
            let end = t_next.min(t - 1) as usize;

            for s in start..end {
                r[s] = r_val;
            }
        }

        Self::smooth(&r, t)
    }

    fn compute_c_and_ks(k: i32, t: i32, note_seq: &[(i32, i32, i32)], key_usage: &[Vec<bool>]) -> (Vec<f64>, Vec<f64>) {
        // C(s): count of notes within 500 ms
        let mut note_hit_times: Vec<f64> = note_seq.iter().map(|&(_, h, _)| h as f64 / 1000.0).collect();
        note_hit_times.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mut c_step = vec![0.0; t as usize];
        for i in 0..t as usize {
            let low = i as f64 - 500.0;
            let high = i as f64 + 500.0;
            let left = note_hit_times.partition_point(|&time| time < low);
            let right = note_hit_times.partition_point(|&time| time < high);
            c_step[i] = (right - left) as f64;
        }

        // Ks: local key usage count
        let mut ks_step = vec![0.0; t as usize];
        for i in 0..t as usize {
            let mut count = 0;
            for col in 0..k as usize {
                if key_usage[col][i] {
                    count += 1;
                }
            }
            ks_step[i] = count.max(1) as f64;
        }

        (c_step, ks_step)
    }

    fn smooth(lst: &[f64], t: i32) -> Vec<f64> {
        let mut prefix_sum = vec![0.0; (t + 1) as usize];
        for i in 1..=(t as usize) {
            prefix_sum[i] = prefix_sum[i - 1] + lst[i - 1];
        }

        let mut result = vec![0.0; t as usize];
        for s in 0..(t as usize) {
            let left = (s as i32 - 500).max(0) as usize;
            let right = ((s + 500) as i32).min(t) as usize;
            let sum = prefix_sum[right] - prefix_sum[left];
            result[s] = 0.001 * sum;
        }

        result
    }

    fn calculate_final_sr(
        jbar: &[f64],
        xbar: &[f64],
        pbar: &[f64],
        abar: &[f64],
        rbar: &[f64],
        d_arr: &[f64],
        c: &[f64],
        note_seq: &[(i32, i32, i32)],
        ln_seq: &[(i32, i32, i32)],
    ) -> f64 {
        println!("calculate_final_sr called");
        let len = jbar.len();
        let mut s = vec![0.0; len];
        let mut d = vec![0.0; len];

        for i in 0..len {
            let j_val = jbar[i].max(0.0);
            let x_val = xbar[i].max(0.0);
            let p_val = pbar[i].max(0.0);
            let a_val = abar[i].max(0.0);
            let r_val = rbar[i].max(0.0);
            let ks_val = d_arr[i];
            let c_val = c[i];

            let term1 = 0.4 * (a_val.powf(3.0 / ks_val) * j_val.min(8.0 + 0.85 * j_val)).powf(1.5);
            let term2 = (1.0 - 0.4) * (a_val.powf(2.0 / 3.0) * (0.8 * p_val + r_val * 35.0 / (c_val + 8.0))).powf(1.5);
            s[i] = (term1 + term2).powf(2.0 / 3.0);

            let t_t = a_val.powf(3.0 / ks_val) * x_val / (x_val + s[i] + 1.0);
            d[i] = 2.7 * s[i].powf(0.5) * t_t.powf(1.5) + s[i] * 0.27;
        }

        // Forward fill
        let mut last_valid = 0.0;
        for i in 0..d.len() {
            if d[i] != 0.0 {
                last_valid = d[i];
            } else {
                d[i] = last_valid;
            }
        }

        let mut c_mut = c.to_vec();
        let mut last_valid_c = 0.0;
        for i in 0..c_mut.len() {
            if c_mut[i] != 0.0 {
                last_valid_c = c_mut[i];
            } else {
                c_mut[i] = last_valid_c;
            }
        }

        // Calculate percentiles
        let mut d_with_weights: Vec<(f64, f64)> = d.iter().map(|&d_val| (d_val, 1.0)).collect();
        d_with_weights.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let sorted_d: Vec<f64> = d_with_weights.iter().map(|(d, _)| *d).collect();
        let sorted_weights: Vec<f64> = d_with_weights.iter().map(|(_, w)| *w).collect();

        let total_weight: f64 = sorted_weights.iter().sum();
        let mut cum_weights = vec![0.0; sorted_weights.len()];
        for i in 0..sorted_weights.len() {
            cum_weights[i] = if i == 0 { sorted_weights[i] } else { cum_weights[i - 1] + sorted_weights[i] };
        }
        let norm_cum_weights: Vec<f64> = cum_weights.iter().map(|&cw| cw / total_weight).collect();

        let target_percentiles = [0.945, 0.935, 0.925, 0.915, 0.845, 0.835, 0.825, 0.815];

        let mut percentile_93 = 0.0;
        for &p in &target_percentiles[..4] {
            let idx = norm_cum_weights.partition_point(|&x| x < p).min(sorted_d.len() - 1);
            percentile_93 += sorted_d[idx];
        }
        percentile_93 /= 4.0;

        let mut percentile_83 = 0.0;
        for &p in &target_percentiles[4..] {
            let idx = norm_cum_weights.partition_point(|&x| x < p).min(sorted_d.len() - 1);
            percentile_83 += sorted_d[idx];
        }
        percentile_83 /= 4.0;

        let weighted_mean = (sorted_d.iter().map(|d| d.powf(5.0)).sum::<f64>() / sorted_d.len() as f64).powf(1.0 / 5.0);

        let mut sr = 0.88 * percentile_93 * 0.25 + 0.94 * percentile_83 * 0.2 + weighted_mean * 0.55;
        println!("DEBUG: percentile_93={}, sr={}", percentile_93, sr);
        println!("Before scaling: percentile_93={}, percentile_83={}, weighted_mean={}, sr={}", percentile_93, percentile_83, weighted_mean, sr);
        sr = sr.powf(1.0) / 8.0f64.powf(1.0) * 8.0;

        let mut total_notes = note_seq.len() as f64;
        for &(_, h, t) in ln_seq {
            let ln_length = ((t - h) as f64).min(1000.0);
            total_notes += 0.5 * (ln_length / 200.0);
        }
        sr *= total_notes / (total_notes + 60.0);

        sr = Self::rescale_high(sr);
        sr *= 0.975;
        println!("Final sr: {}", sr);
        sr
    }

    fn rescale_high(sr: f64) -> f64 {
        if sr <= 9.0 {
            sr
        } else {
            9.0 + (sr - 9.0) * (1.0 / 1.2)
        }
    }
}
