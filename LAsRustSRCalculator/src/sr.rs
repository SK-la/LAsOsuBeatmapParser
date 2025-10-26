use rayon::prelude::*;

use crate::note::{Note, NoteComparerByT};
use crate::cross_matrix::CrossMatrixProvider;
use crate::parser::ParsedData;

pub struct SRCalculator;

impl SRCalculator {
    const LAMBDA_N: f64 = 5.0;
    const LAMBDA_1: f64 = 0.11;
    const LAMBDA_3: f64 = 24.0;
    const LAMBDA_2: f64 = 7.0;
    const W_0: f64 = 0.4;
    const W_1: f64 = 2.7;
    const P_1: f64 = 1.5;
    const W_2: f64 = 0.27;
    const P_0: f64 = 1.0;
    const GRANULARITY: i32 = 1;

    pub fn calculate_sr_from_parsed_data(data: &ParsedData) -> Result<f64, String> {
        let od = data.od;
        let k = data.column_count;

        if k > 18 || k < 1 || (k > 10 && k % 2 == 1) {
            return Err("Unsupported key count".to_string());
        }

        let mut note_sequence: Vec<Note> = data.columns.iter().enumerate().map(|(i, &col)| {
            let h = data.note_starts[i];
            let t = if data.note_types[i] == 128 { data.note_ends[i] } else { -1 };
            Note::new(col, h, t)
        }).collect();

        if note_sequence.is_empty() {
            return Ok(0.0);
        }

        note_sequence.sort();

        let x: f64 = 0.3 * ((64.5 - (od * 3.0).ceil()) / 500.0).sqrt();
        let x = x.min(0.6 * (x - 0.09) + 0.09);

        let mut note_seq_by_column: Vec<Vec<Note>> = vec![vec![]; k as usize];
        for note in &note_sequence {
            if note.k >= 0 && (note.k as usize) < note_seq_by_column.len() {
                note_seq_by_column[note.k as usize].push(*note);
            }
        }

        let ln_seq: Vec<Note> = note_sequence.iter().filter(|n| n.t >= 0).cloned().collect();
        let mut ln_seq_sorted = ln_seq.clone();
        ln_seq_sorted.sort_by(NoteComparerByT::cmp);

        let t = note_sequence
            .iter()
            .map(|n| n.h)
            .max()
            .unwrap()
            .max(note_sequence.iter().map(|n| n.t).max().unwrap())
            + 1;

        let t = if t > 1000000 { 1000000 } else { t };

        let base_corners = Self::get_uniform_corners(t as f64);

        let key_usage = Self::get_key_usage(k, t, &note_sequence, &base_corners);
        let _active_columns: Vec<Vec<usize>> = (0..base_corners.len())
            .map(|i| (0..k as usize).filter(|&col| key_usage[col][i]).collect())
            .collect();

        let key_usage_400 = Self::get_key_usage_400(k, t, &note_sequence, &base_corners);
        let _anchor = Self::compute_anchor(k, &key_usage_400, &base_corners);

        let (j_bar, delta_ks) = Self::calculate_section_23(k, &note_seq_by_column, &base_corners, x);
        let j_bar_interp = j_bar.clone(); // No interpolation needed for uniform grid

        let x_bar = Self::calculate_section_24(k, &base_corners, &note_seq_by_column, x);
        let x_bar_interp = x_bar.clone(); // No interpolation needed for uniform grid

        let _ln_rep = Self::ln_bodies_count_sparse_representation(&ln_seq, t);

        let p_bar = Self::calculate_section_25(&base_corners, &ln_seq_sorted, &note_sequence, x);
        let p_bar_interp = p_bar.clone(); // No interpolation needed for uniform grid

        let (a_bar, _ks) = Self::calculate_section_26(&delta_ks, k, &base_corners, &note_sequence);
        let a_bar_interp = a_bar.clone(); // No interpolation needed for uniform grid

        let tail_seq: Vec<Note> = ln_seq_sorted.iter().cloned().collect();
        let (r_bar, _) = Self::calculate_section_27(&ln_seq, &tail_seq, &base_corners, &note_seq_by_column, x);
        let r_bar_interp = r_bar.clone(); // No interpolation needed for uniform grid

        let (c_arr, ks_arr) = Self::compute_c_and_ks(k, t, &note_sequence, &key_usage, &base_corners);
        let mut c_arr_interp = c_arr.clone(); // No interpolation needed for uniform grid
        let ks_arr_f64: Vec<f64> = ks_arr.iter().map(|&x| x as f64).collect();
        let ks_arr_interp = ks_arr_f64.clone(); // No interpolation needed for uniform grid

        let sr = Self::calculate_final_sr(
            &j_bar_interp,
            &x_bar_interp,
            &p_bar_interp,
            &a_bar_interp,
            &r_bar_interp,
            &ks_arr_interp,
            &mut c_arr_interp,
            &base_corners,
            &note_sequence,
            &ln_seq_sorted,
        );

        Ok(sr)
    }

    pub fn calculate_section_23(
        k: i32,
        note_seq_by_column: &[Vec<Note>],
        base_corners: &[f64],
        x: f64,
    ) -> (Vec<f64>, Vec<Vec<f64>>) {
        let grid_size = base_corners.len();
        let mut j_ks: Vec<Vec<f64>> = vec![vec![0.0; grid_size]; k as usize];
        let mut delta_ks: Vec<Vec<f64>> = vec![vec![1e9; grid_size]; k as usize];

        let x_pow_025 = x.sqrt().sqrt();
        let lambda1_x = Self::LAMBDA_1 * x_pow_025;

        // Parallel computation for each column
        j_ks.par_iter_mut().zip(delta_ks.par_iter_mut()).enumerate().for_each(|(k_idx, (j_k, delta_k))| {
            if k_idx < note_seq_by_column.len() && note_seq_by_column[k_idx].len() > 1 {
                for i in 0..note_seq_by_column[k_idx].len() - 1 {
                    let delta = 0.001
                        * (note_seq_by_column[k_idx][i + 1].h - note_seq_by_column[k_idx][i].h)
                            as f64;
                    let abs_delta = (delta - 0.08).abs();
                    let temp = 0.15 + abs_delta;
                    let temp4 = temp * temp * temp * temp;
                    let jack = 1.0 - 7e-5 * (1.0 / temp4);
                    let val = (1.0 / (delta * (delta + lambda1_x))) * jack;

                    let start = note_seq_by_column[k_idx][i].h as usize;
                    let end = note_seq_by_column[k_idx][i + 1].h as usize;
                    for s in start..end {
                        if s < delta_k.len() {
                            delta_k[s] = delta;
                            j_k[s] = val;
                        }
                    }
                }
            }
        });

        let j_bar_ks: Vec<Vec<f64>> = j_ks.iter().map(|jk| Self::smooth(jk, grid_size as i32)).collect();

        let mut j_bar = vec![0.0; grid_size];
        for s in (0..grid_size).step_by(Self::GRANULARITY as usize) {
            let mut weighted_sum = 0.0;
            let mut weight_sum = 0.0;
            for i in 0..k as usize {
                let val = j_bar_ks[i][s];
                let weight = 1.0 / delta_ks[i][s];
                weight_sum += weight;
                weighted_sum += val.max(0.0).powf(Self::LAMBDA_N) * weight;
            }
            weight_sum = weight_sum.max(1e-9);
            j_bar[s] = (weighted_sum / weight_sum).powf(1.0 / Self::LAMBDA_N);
        }

        (j_bar, delta_ks)
    }

    pub fn calculate_section_24(k: i32, base_corners: &[f64], note_seq_by_column: &[Vec<Note>], x: f64) -> Vec<f64> {
        let grid_size = base_corners.len();
        let mut x_ks: Vec<Vec<f64>> = vec![vec![0.0; grid_size]; (k + 1) as usize];

        for k_idx in 0..=(k as usize) {
            let notes_in_pair = if k_idx == 0 {
                note_seq_by_column.get(0).cloned().unwrap_or_default()
            } else if k_idx == k as usize {
                note_seq_by_column.last().cloned().unwrap_or_default()
            } else {
                let left_col = k_idx - 1;
                let right_col = k_idx;
                let mut left_notes = note_seq_by_column
                    .get(left_col)
                    .cloned()
                    .unwrap_or_default();
                let mut right_notes = note_seq_by_column
                    .get(right_col)
                    .cloned()
                    .unwrap_or_default();
                left_notes.append(&mut right_notes);
                left_notes.sort_by_key(|n| n.h);
                left_notes
            };

            for i in 1..notes_in_pair.len() {
                let delta = 0.001 * (notes_in_pair[i].h - notes_in_pair[i - 1].h) as f64;
                let max_xd = x.max(delta);
                let val = 0.16 / (max_xd * max_xd);

                let start = notes_in_pair[i - 1].h as usize;
                let end = notes_in_pair[i].h as usize;
                for s in start..end {
                    if s < x_ks[k_idx].len() {
                        x_ks[k_idx][s] = val;
                    }
                }
            }
        }

        let mut x_arr = vec![0.0; grid_size];
        for s in (0..grid_size).step_by(Self::GRANULARITY as usize) {
            x_arr[s] = 0.0;
            for k_idx in 0..=(k as usize) {
                x_arr[s] += x_ks[k_idx][s] * CrossMatrixProvider::get_matrix(k as usize).unwrap()[k_idx];
            }
        }

        Self::smooth(&x_arr, grid_size as i32)
    }

    pub fn calculate_section_25(base_corners: &[f64], ln_seq: &[Note], note_seq: &[Note], x: f64) -> Vec<f64> {
        let grid_size = base_corners.len();
        let mut p = vec![0.0; grid_size];
        let mut ln_bodies = vec![0.0; grid_size];

        ln_seq.par_iter().for_each(|note| {
            let t1 = (note.h + 80).min(note.t);
            for time in note.h..t1 {
                if time >= 0 && (time as usize) < grid_size {
                    // Atomic add would be needed for thread safety, but for simplicity using sequential
                }
            }
            for time in t1..note.t {
                if time >= 0 && (time as usize) < grid_size {
                    // Same issue
                }
            }
        });

        // Simplified implementation - need proper parallel accumulation
        for note in ln_seq {
            let t1 = (note.h + 80).min(note.t);
            for time in note.h..t1 {
                if time >= 0 && (time as usize) < ln_bodies.len() {
                    ln_bodies[time as usize] += 0.5;
                }
            }
            for time in t1..note.t {
                if time >= 0 && (time as usize) < ln_bodies.len() {
                    ln_bodies[time as usize] += 1.0;
                }
            }
        }

        let mut prefix_sum_ln_bodies = vec![0.0; grid_size + 1];
        for i in 1..=grid_size {
            prefix_sum_ln_bodies[i] = prefix_sum_ln_bodies[i - 1] + ln_bodies[i - 1];
        }

        let b = |delta: f64| {
            let val = 7.5 / delta;
            if val > 160.0 && val < 360.0 {
                let diff = val - 160.0;
                let diff2 = val - 360.0;
                1.0 + 1.4e-7 * diff * (diff2 * diff2)
            } else {
                1.0
            }
        };

        let lambda2_scaled = Self::LAMBDA_2 * 0.001;

        for i in 0..note_seq.len() - 1 {
            let delta = 0.001 * (note_seq[i + 1].h - note_seq[i].h) as f64;
            if delta < 1e-9 {
                if (note_seq[i].h as usize) < p.len() {
                    p[note_seq[i].h as usize] +=
                        1000.0 * (0.02 * (4.0 / x - Self::LAMBDA_3)).sqrt().sqrt();
                }
            } else {
                let h_l = note_seq[i].h as usize;
                let h_r = note_seq[i + 1].h as usize;
                let v =
                    1.0 + lambda2_scaled * (prefix_sum_ln_bodies[h_r] - prefix_sum_ln_bodies[h_l]);

                let base_val = if delta < 2.0 * x / 3.0 {
                    (0.08 / x * (1.0 - Self::LAMBDA_3 / x * (delta - x / 2.0) * (delta - x / 2.0)))
                        .sqrt()
                        .sqrt()
                        * b(delta)
                        * v
                        / delta
                } else {
                    (0.08 / x * (1.0 - Self::LAMBDA_3 / x * (x / 6.0) * (x / 6.0)))
                        .sqrt()
                        .sqrt()
                        * b(delta)
                        * v
                        / delta
                };

                for s in h_l..h_r {
                    if s < p.len() {
                        p[s] += base_val;
                    }
                }
            }
        }

        Self::smooth(&p, grid_size as i32)
    }

    pub fn calculate_section_26(
        delta_ks: &[Vec<f64>],
        k: i32,
        base_corners: &[f64],
        note_seq: &[Note],
    ) -> (Vec<f64>, Vec<i32>) {
        let grid_size = base_corners.len();
        if delta_ks.is_empty() || k <= 0 || grid_size == 0 {
            return (vec![1.0; grid_size], vec![1i32; grid_size]);
        }
        let mut ku_ks: Vec<Vec<bool>> = vec![vec![false; grid_size]; k as usize];

        // Sequential processing for ku_ks
        for note in note_seq {
            let start_time = (note.h - 500).max(0) as usize;
            let end_time = if note.t < 0 {
                (note.h + 500).min((grid_size - 1) as i32) as usize
            } else {
                (note.t + 500).min((grid_size - 1) as i32) as usize
            };
            for s in start_time..end_time {
                if note.k >= 0
                    && (note.k as usize) < ku_ks.len()
                    && s < ku_ks[note.k as usize].len()
                {
                    ku_ks[note.k as usize][s] = true;
                }
            }
        }

        let mut ks = vec![0i32; grid_size];
        let mut a = vec![1.0; grid_size];

        let mut dks: Vec<Vec<f64>> = vec![vec![0.0; grid_size]; k as usize];

        for s in (0..grid_size).step_by(Self::GRANULARITY as usize) {
            let mut cols = vec![];
            for k_idx in 0..k as usize {
                if ku_ks[k_idx][s] {
                    cols.push(k_idx);
                }
            }
            ks[s] = cols.len().max(1) as i32;

            for i in 0..cols.len().saturating_sub(1) {
                let col1 = cols[i];
                let col2 = cols[i + 1];
                if col1 >= delta_ks.len()
                    || col2 >= delta_ks.len()
                    || s >= delta_ks[col1].len()
                    || s >= delta_ks[col2].len()
                    || col1 >= dks.len()
                    || s >= dks[col1].len()
                {
                    continue;
                }
                let delta1 = delta_ks[col1][s];
                let delta2 = delta_ks[col2][s];
                dks[col1][s] = (delta1 - delta2).abs() + (delta1.max(delta2) - 0.3).max(0.0);

                let max_delta = delta1.max(delta2);
                if dks[col1][s] < 0.02 {
                    a[s] *= (0.75 + 0.5 * max_delta).min(1.0);
                } else if dks[col1][s] < 0.07 {
                    a[s] *= (0.65 + 5.0 * dks[col1][s] + 0.5 * max_delta).min(1.0);
                }
            }
        }

        let a_bar = Self::smooth(&a, grid_size as i32);
        (a_bar, ks)
    }

    pub fn calculate_section_27(
        ln_seq: &[Note],
        tail_seq: &[Note],
        base_corners: &[f64],
        note_seq_by_column: &[Vec<Note>],
        x: f64,
    ) -> (Vec<f64>, Vec<f64>) {
        let mut i_arr = vec![0.0; ln_seq.len()];

        i_arr.par_iter_mut().enumerate().for_each(|(i, i_val)| {
            let k = tail_seq[i].k;
            let h_i = tail_seq[i].h;
            let t_i = tail_seq[i].t;

            let column_notes = note_seq_by_column
                .get(k as usize)
                .cloned()
                .unwrap_or_default();
            let next_note = Self::find_next_note_in_column(&tail_seq[i], &column_notes);
            let h_j = next_note.h;

            let i_h = 0.001 * ((t_i - h_i - 80) as f64).abs() / x;
            let i_t = 0.001 * ((h_j - t_i - 80) as f64).abs() / x;
            *i_val = 2.0 / (2.0 + (-5.0 * (i_h - 0.75)).exp() + (-5.0 * (i_t - 0.75)).exp());
        });

        let grid_size = base_corners.len();
        let mut is_arr = vec![0.0; grid_size];
        let mut r = vec![0.0; grid_size];

        for i in 0..tail_seq.len().saturating_sub(1) {
            let delta_r = 0.001 * (tail_seq[i + 1].t - tail_seq[i].t) as f64;
            let is_val = 1.0 + i_arr[i];
            let r_val = 0.08
                * delta_r.powf(-0.5)
                * x.powf(-1.0)
                * (1.0 + 0.8 * (i_arr[i] + i_arr.get(i + 1).copied().unwrap_or(0.0)));

            let start = tail_seq[i].t as usize;
            let end = tail_seq[i + 1].t as usize;
            for s in start..end {
                if s < is_arr.len() {
                    is_arr[s] = is_val;
                }
                if s < r.len() {
                    r[s] = r_val;
                }
            }
        }

        let r_bar = Self::smooth(&r, grid_size as i32);
        (r_bar, is_arr)
    }

    fn find_next_note_in_column(note: &Note, column_notes: &[Note]) -> Note {
        let index = column_notes.partition_point(|n| n.h <= note.h);

        if index < column_notes.len() {
            column_notes[index]
        } else {
            Note::new(0, 1_000_000_000, 1_000_000_000)
        }
    }

    pub fn calculate_section_3(
        j_bar: Vec<f64>,
        x_bar: Vec<f64>,
        p_bar: Vec<f64>,
        a_bar: Vec<f64>,
        r_bar: Vec<f64>,
        ks: Vec<i32>,
        t: i32,
        note_seq: &[Note],
        ln_seq: &[Note],
    ) -> f64 {
        let mut c = vec![0.0; t as usize];
        let mut start = 0;
        let mut end = 0;

        for time in 0..t as usize {
            while start < note_seq.len() && note_seq[start].h < time as i32 - 500 {
                start += 1;
            }
            while end < note_seq.len() && note_seq[end].h < time as i32 + 500 {
                end += 1;
            }
            c[time] = (end - start) as f64;
        }

        let mut s = vec![0.0; t as usize];
        let mut d = vec![0.0; t as usize];

        for time in 0..t as usize {
            let j_bar_val = j_bar[time].max(0.0);
            let x_bar_val = x_bar[time].max(0.0);
            let p_bar_val = p_bar[time].max(0.0);
            let a_bar_val = a_bar[time].max(0.0);
            let r_bar_val = r_bar[time].max(0.0);

            let term1 = Self::W_0
                * (a_bar_val.powf(3.0 / ks[time] as f64) * j_bar_val.min(8.0 + 0.85 * j_bar_val))
                    .powf(1.5);
            let term2 = (1.0 - Self::W_0)
                * (a_bar_val.powf(2.0 / 3.0)
                    * (0.8 * p_bar_val + r_bar_val * 35.0 / (c[time] + 8.0)))
                    .powf(1.5);
            s[time] = (term1 + term2).powf(2.0 / 3.0);

            let t_t =
                a_bar_val.powf(3.0 / ks[time] as f64) * x_bar_val / (x_bar_val + s[time] + 1.0);
            d[time] = Self::W_1 * s[time].powf(0.5) * t_t.powf(Self::P_1) + s[time] * Self::W_2;
        }

        let mut c = c.to_vec();
        Self::forward_fill(&mut d);
        Self::forward_fill(&mut c);

        // New percentile-based calculation
        let d_list: Vec<f64> = d.clone();
        let weights: Vec<f64> = c.clone();

        // Sort D by value, keep weights
        let mut sorted_pairs: Vec<(f64, f64)> =
            d_list.into_iter().zip(weights.into_iter()).collect();
        sorted_pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let sorted_d: Vec<f64> = sorted_pairs.iter().map(|(d, _)| *d).collect();
        let sorted_weights: Vec<f64> = sorted_pairs.iter().map(|(_, w)| *w).collect();

        // Cumulative weights
        let total_weight: f64 = sorted_weights.iter().sum();
        let mut cum_weights = vec![0.0; sorted_weights.len()];
        cum_weights[0] = sorted_weights[0];
        for i in 1..cum_weights.len() {
            cum_weights[i] = cum_weights[i - 1] + sorted_weights[i];
        }

        let norm_cum_weights: Vec<f64> = cum_weights.iter().map(|cw| cw / total_weight).collect();

        let target_percentiles = [0.945, 0.935, 0.925, 0.915, 0.845, 0.835, 0.825, 0.815];

        let mut percentile93 = 0.0;
        for i in 0..4 {
            let idx = norm_cum_weights
                .binary_search_by(|&x| x.partial_cmp(&target_percentiles[i]).unwrap()).unwrap_or_else(|idx| idx)
            .min(sorted_d.len() - 1);
            percentile93 += sorted_d[idx];
        }
        percentile93 /= 4.0;

        let mut percentile83 = 0.0;
        for i in 4..8 {
            let idx = norm_cum_weights
                .binary_search_by(|&x| x.partial_cmp(&target_percentiles[i]).unwrap()).unwrap_or_else(|idx| idx)
            .min(sorted_d.len() - 1);
            percentile83 += sorted_d[idx];
        }
        percentile83 /= 4.0;

        let weighted_mean = (sorted_d
            .iter()
            .zip(sorted_weights.iter())
            .map(|(d, w)| d.powf(5.0) * w)
            .sum::<f64>()
            / sorted_weights.iter().sum::<f64>())
        .powf(1.0 / 5.0);

        let mut sr = 0.88 * percentile93 * 0.25 + 0.94 * percentile83 * 0.2 + weighted_mean * 0.55;
        sr = sr.powf(Self::P_0) / 8.0f64.powf(Self::P_0) * 8.0;
        let total_notes = note_seq.len() as f64 + 0.5 * ln_seq.len() as f64;
        sr *= total_notes / (total_notes + 60.0);

        sr = Self::rescale_high(sr);
        sr *= 0.975;
        sr
    }

    fn forward_fill(array: &mut [f64]) {
        let mut last_valid_value = 0.0;
        for val in array.iter_mut() {
            if !val.is_nan() && *val != 0.0 {
                last_valid_value = *val;
            } else {
                *val = last_valid_value;
            }
        }
    }

    fn smooth(lst: &[f64], t: i32) -> Vec<f64> {
        let mut prefix_sum = vec![0.0; (t + 1) as usize];
        for i in 1..=(t as usize) {
            prefix_sum[i] = prefix_sum[i - 1] + lst[i - 1];
        }

        let mut lst_bar = vec![0.0; t as usize];
        for s in (0..t as usize).step_by(Self::GRANULARITY as usize) {
            let left = (s as i32 - 500).max(0) as usize;
            let right = (s as i32 + 500).min(t) as usize;
            let sum = prefix_sum[right] - prefix_sum[left];
            lst_bar[s] = 0.001 * sum;
        }
        lst_bar
    }

    fn rescale_high(sr: f64) -> f64 {
        if sr <= 9.0 {
            sr
        } else {
            9.0 + (sr - 9.0) * (1.0 / 1.2)
        }
    }

    pub fn get_uniform_corners(t: f64) -> Vec<f64> {
        let mut corners = Vec::new();
        let mut current = 0.0;
        while current < t {
            corners.push(current);
            current += 1.0;
        }
        corners
    }

    pub fn get_key_usage(k: i32, t: i32, note_seq: &[Note], base_corners: &[f64]) -> Vec<Vec<bool>> {
        let mut key_usage = vec![vec![false; base_corners.len()]; k as usize];
        for note in note_seq {
            let start_time = (note.h as f64 - 150.0).max(0.0);
            let end_time = if note.t < 0 {
                note.h as f64 + 150.0
            } else {
                (note.t as f64 + 150.0).min(t as f64 - 1.0)
            };
            let left_idx = base_corners.partition_point(|&x| x < start_time);
            let right_idx = base_corners.partition_point(|&x| x < end_time);
            for idx in left_idx..right_idx {
                if idx < key_usage[note.k as usize].len() {
                    key_usage[note.k as usize][idx] = true;
                }
            }
        }
        key_usage
    }

    pub fn get_key_usage_400(k: i32, _t: i32, note_seq: &[Note], base_corners: &[f64]) -> Vec<Vec<f64>> {
        let mut key_usage_400 = vec![vec![0.0; base_corners.len()]; k as usize];
        for note in note_seq {
            let start_time = note.h as f64;
            let end_time = if note.t < 0 {
                note.h as f64
            } else {
                note.t as f64
            };
            let left400_idx = base_corners.partition_point(|&x| x < start_time - 400.0);
            let left_idx = base_corners.partition_point(|&x| x < start_time);
            let right_idx = base_corners.partition_point(|&x| x < end_time);
            let right400_idx = base_corners.partition_point(|&x| x < end_time + 400.0);
            for idx in left_idx..right_idx {
                if idx < key_usage_400[note.k as usize].len() {
                    key_usage_400[note.k as usize][idx] += 3.75 + ((end_time - start_time).min(1500.0) / 150.0);
                }
            }
            for idx in left400_idx..left_idx {
                if idx < key_usage_400[note.k as usize].len() {
                    let delta = start_time - base_corners[idx];
                    key_usage_400[note.k as usize][idx] += 3.75 - 3.75 / 400.0_f64.powi(2) * delta.powi(2);
                }
            }
            for idx in right_idx..right400_idx {
                if idx < key_usage_400[note.k as usize].len() {
                    let delta = base_corners[idx] - end_time;
                    key_usage_400[note.k as usize][idx] += 3.75 - 3.75 / 400.0_f64.powi(2) * delta.powi(2);
                }
            }
        }
        key_usage_400
    }

    pub fn compute_anchor(k: i32, key_usage_400: &[Vec<f64>], _base_corners: &[f64]) -> Vec<f64> {
        let mut anchor = vec![0.0; key_usage_400[0].len()];
        for idx in 0..anchor.len() {
            let mut counts: Vec<f64> = (0..k as usize).map(|ki| key_usage_400[ki][idx]).collect();
            counts.sort_by(|a, b| b.partial_cmp(a).unwrap()); // descending
            let nonzero_counts: Vec<f64> = counts.into_iter().filter(|&x| x != 0.0).collect();
            if nonzero_counts.len() > 1 {
                let walk: f64 = nonzero_counts.iter().enumerate().skip(1).map(|(i, &c)| (nonzero_counts[i-1] - c) * (1.0 - 4.0 * (0.5 - c / nonzero_counts[i-1]).powi(2))).sum();
                let max_walk: f64 = nonzero_counts.iter().sum();
                anchor[idx] = walk / max_walk;
            } else {
                anchor[idx] = 0.0;
            }
        }
        anchor = anchor.iter().map(|&a| 1.0 + (a - 0.18).min(5.0 * (a - 0.22).powi(3))).collect();
        anchor
    }

    pub fn ln_bodies_count_sparse_representation(ln_seq: &[Note], t: i32) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let mut diff: std::collections::HashMap<i32, f64> = std::collections::HashMap::new();
        for note in ln_seq {
            let t0 = (note.h as f64 + 60.0).min(note.t as f64) as i32;
            let t1 = (note.h as f64 + 120.0).min(note.t as f64) as i32;
            *diff.entry(t0).or_insert(0.0) += 1.3;
            *diff.entry(t1).or_insert(0.0) += -1.3 + 1.0; // net change at t1: -1.3 from first part, then +1
            *diff.entry(note.t).or_insert(0.0) -= 1.0;
        }

        let mut points: Vec<i32> = vec![0, t];
        points.extend(diff.keys().cloned());
        points.sort();
        points.dedup();

        let mut values = Vec::new();
        let mut cumsum = vec![0.0];
        let mut curr = 0.0;

        for i in 0..points.len() - 1 {
            let t_point = points[i];
            if let Some(&change) = diff.get(&t_point) {
                curr += change;
            }
            let v = curr.min(2.5 + 0.5 * curr);
            values.push(v);
            let seg_length = (points[i + 1] - points[i]) as f64;
            cumsum.push(cumsum.last().unwrap() + seg_length * v);
        }
        (points.into_iter().map(|x| x as f64).collect(), cumsum, values)
    }

    pub fn interp_values(new_x: &[f64], old_x: &[f64], old_vals: &[f64]) -> Vec<f64> {
        new_x.iter().map(|&x| {
            if x <= old_x[0] {
                old_vals[0]
            } else if x >= old_x[old_x.len() - 1] {
                old_vals[old_vals.len() - 1]
            } else {
                let idx = old_x.partition_point(|&val| val < x) - 1;
                if idx >= old_x.len() - 1 {
                    old_vals[old_vals.len() - 1]
                } else {
                    let x1 = old_x[idx];
                    let x2 = old_x[idx + 1];
                    let y1 = old_vals[idx];
                    let y2 = old_vals[idx + 1];
                    y1 + (y2 - y1) * (x - x1) / (x2 - x1)
                }
            }
        }).collect()
    }

    pub fn step_interp(new_x: &[f64], old_x: &[f64], old_vals: &[f64]) -> Vec<f64> {
        new_x.iter().map(|&x| {
            let idx = old_x.partition_point(|&val| val <= x).saturating_sub(1);
            old_vals[idx.min(old_vals.len() - 1)]
        }).collect()
    }

    pub fn compute_c_and_ks(k: i32, _t: i32, note_seq: &[Note], key_usage: &[Vec<bool>], base_corners: &[f64]) -> (Vec<f64>, Vec<i32>) {
        let mut note_hit_times: Vec<i32> = note_seq.iter().map(|n| n.h).collect();
        note_hit_times.sort();

        let c_step: Vec<f64> = base_corners.iter().map(|&s| {
            let low = s - 500.0;
            let high = s + 500.0;
            let left = note_hit_times.partition_point(|&time| (time as f64) < low);
            let right = note_hit_times.partition_point(|&time| (time as f64) < high);
            (right - left) as f64
        }).collect();

        let ks_step: Vec<i32> = (0..base_corners.len()).map(|i| {
            let count: i32 = (0..k as usize).map(|col| key_usage[col][i] as i32).sum();
            count.max(1)
        }).collect();

        (c_step, ks_step)
    }

    pub fn calculate_final_sr(
        j_bar: &[f64],
        x_bar: &[f64],
        p_bar: &[f64],
        a_bar: &[f64],
        r_bar: &[f64],
        ks: &[f64],
        c: &mut [f64],
        all_corners: &[f64],
        note_seq: &[Note],
        ln_seq: &[Note],
    ) -> f64 {
        let mut s = vec![0.0; all_corners.len()];
        let mut d = vec![0.0; all_corners.len()];

        for i in 0..all_corners.len() {
            let j_bar_val = j_bar[i].max(0.0);
            let x_bar_val = x_bar[i].max(0.0);
            let p_bar_val = p_bar[i].max(0.0);
            let a_bar_val = a_bar[i].max(0.0);
            let r_bar_val = r_bar[i].max(0.0);
            let ks_val = ks[i];
            let c_val = c[i];

            let term1 = Self::W_0 * (a_bar_val.powf(3.0 / ks_val) * j_bar_val.min(8.0 + 0.85 * j_bar_val)).powf(1.5);
            let term2 = (1.0 - Self::W_0) * (a_bar_val.powf(2.0 / 3.0) * (0.8 * p_bar_val + r_bar_val * 35.0 / (c_val + 8.0))).powf(1.5);
            s[i] = (term1 + term2).powf(2.0 / 3.0);

            let t_t = a_bar_val.powf(3.0 / ks_val) * x_bar_val / (x_bar_val + s[i] + 1.0);
            d[i] = Self::W_1 * s[i].powf(0.5) * t_t.powf(Self::P_1) + s[i] * Self::W_2;
        }

        Self::forward_fill(&mut d);
        Self::forward_fill(c);

        // Use C values directly as weights (matching C# implementation)
        let effective_weights: Vec<f64> = c.iter().map(|&c_val| c_val).collect();

        // Percentile calculation
        let mut d_with_weights: Vec<(f64, f64)> = d.iter().zip(effective_weights.iter()).map(|(&d_val, &w)| (d_val, w)).collect();
        d_with_weights.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let sorted_d: Vec<f64> = d_with_weights.iter().map(|(d, _)| *d).collect();
        let sorted_weights: Vec<f64> = d_with_weights.iter().map(|(_, w)| *w).collect();

        let total_weight: f64 = sorted_weights.iter().sum();
        let mut cum_weights = vec![0.0; sorted_weights.len()];
        for i in 0..sorted_weights.len() {
            cum_weights[i] = if i == 0 { sorted_weights[i] } else { cum_weights[i-1] + sorted_weights[i] };
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

        let weighted_mean = (sorted_d.iter().zip(sorted_weights.iter())
            .map(|(d, w)| d.powf(5.0) * w).sum::<f64>() / sorted_weights.iter().sum::<f64>())
            .powf(1.0 / 5.0);

        let mut sr = 0.88 * percentile_93 * 0.25 + 0.94 * percentile_83 * 0.2 + weighted_mean * 0.55;
        println!("Before scaling: percentile_93={}, percentile_83={}, weighted_mean={}, sr={}", percentile_93, percentile_83, weighted_mean, sr);
        sr = sr.powf(Self::P_0) / 8.0f64.powf(Self::P_0) * 8.0;
        println!("After power scaling: sr={}", sr);

        let total_notes = note_seq.len() as f64 + 0.5 * ln_seq.len() as f64;
        sr *= total_notes / (total_notes + 60.0);

        sr = Self::rescale_high(sr);
        sr *= 0.975;
        sr
    }
}