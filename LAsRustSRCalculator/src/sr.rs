use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::note::{Note, NoteComparerByT};
use crate::cross_matrix::CrossMatrixProvider;

#[derive(Serialize, Deserialize)]
pub struct Beatmap {
    pub difficulty_section: DifficultySection,
    pub hit_objects: Vec<HitObject>,
}

#[derive(Serialize, Deserialize)]
pub struct DifficultySection {
    pub overall_difficulty: f64,
    pub circle_size: f64,
}

#[derive(Serialize, Deserialize)]
pub struct HitObject {
    pub position: Position,
    pub start_time: i32,
    pub end_time: i32,
}

#[derive(Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
}

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

    pub fn calculate_sr(beatmap: &Beatmap) -> Result<f64, String> {
        let od = beatmap.difficulty_section.overall_difficulty;
        let k = beatmap.difficulty_section.circle_size as i32;

        if k > 18 || k < 1 || (k > 10 && k % 2 == 1) {
            return Err("Unsupported key count".to_string());
        }

        let mut note_sequence: Vec<Note> = beatmap
            .hit_objects
            .iter()
            .map(|ho| {
                let col = ho.position.x as i32; // Since we set position.x = col
                let mut time = ho.start_time;
                let mut tail = if ho.end_time > ho.start_time {
                    ho.end_time
                } else {
                    -1
                };
                if time < 0 {
                    time = 0;
                }
                if tail < -1 {
                    tail = -1;
                }
                Note::new(col, time, tail)
            })
            .collect();

        if note_sequence.is_empty() {
            return Ok(0.0);
        }

        note_sequence.sort();

        let mut x = 0.3 * ((64.5 - (od * 3.0).ceil()) / 500.0).sqrt();
        x = x.min(0.6 * (x - 0.09) + 0.09);

        let mut note_seq_by_column: Vec<Vec<Note>> = vec![vec![]; k as usize];
        for note in &note_sequence {
            if note.k >= 0 && (note.k as usize) < note_seq_by_column.len() {
                note_seq_by_column[note.k as usize].push(*note);
            }
        }

        let ln_seq: Vec<Note> = note_sequence.iter().filter(|n| n.t >= 0).cloned().collect();
        let mut ln_seq_sorted = ln_seq.clone();
        ln_seq_sorted.sort_by(NoteComparerByT::cmp);

        let mut ln_dict: HashMap<i32, Vec<Note>> = HashMap::new();
        for note in &ln_seq_sorted {
            ln_dict.entry(note.k).or_insert(vec![]).push(*note);
        }

        let t = note_sequence
            .iter()
            .map(|n| n.h)
            .max()
            .unwrap()
            .max(note_sequence.iter().map(|n| n.t).max().unwrap())
            + 1;

        let t = if t > 1000000 { 1000000 } else { t };

        // Parallel computation of sections
        let (j_bar, delta_ks) = Self::calculate_section_23(k, &note_seq_by_column, t, x);
        let x_bar = Self::calculate_section_24(k, t, &note_seq_by_column, x);
        let p_bar = Self::calculate_section_25(t, &ln_seq_sorted, &note_sequence, x);

        let (a_bar, ks): (Vec<f64>, Vec<i32>) =
            Self::calculate_section_26(&delta_ks, k, t, &note_sequence);
        let (r_bar, _): (Vec<f64>, Vec<f64>) =
            Self::calculate_section_27(&ln_seq_sorted, &ln_seq_sorted, t, &note_seq_by_column, x);

        let result = Self::calculate_section_3(
            j_bar,
            x_bar,
            p_bar,
            a_bar,
            r_bar,
            ks,
            t,
            &note_sequence,
            &ln_seq_sorted,
        );

        Ok(result)
    }

    pub fn calculate_sr_from_data(data: &SRData) -> Result<f64, String> {
        let od = data.overall_difficulty;
        let k = data.circle_size as i32;

        if k > 18 || k < 1 || (k > 10 && k % 2 == 1) {
            return Err("Unsupported key count".to_string());
        }

        let mut note_sequence: Vec<Note> = data
            .hit_objects
            .iter()
            .map(|ho| {
                let col = ho.position.x as i32;
                let mut time = ho.start_time;
                let mut tail = ho.end_time;
                if tail <= time {
                    tail = -1;
                }
                if tail == -1 {
                    tail = time;
                }
                if time < 0 {
                    time = 0;
                }
                if tail < -1 {
                    tail = -1;
                }
                Note::new(col, time, tail)
            })
            .collect();

        if note_sequence.is_empty() {
            return Ok(0.0);
        }

        note_sequence.sort();

        let mut x = 0.3 * ((64.5 - (od * 3.0).ceil()) / 500.0).sqrt();
        x = x.min(0.6 * (x - 0.09) + 0.09);

        // Build note_seq_by_column more efficiently
        let mut note_seq_by_column: Vec<Vec<Note>> = vec![vec![]; k as usize];
        for note in &note_sequence {
            if note.k >= 0 && (note.k as usize) < note_seq_by_column.len() {
                note_seq_by_column[note.k as usize].push(*note);
            }
        }

        // Filter LN notes in one pass
        let mut ln_seq: Vec<Note> = note_sequence.iter().filter(|n| n.t != n.h).cloned().collect();
        ln_seq.sort_by(NoteComparerByT::cmp);

        // Calculate t more efficiently
        let mut max_h = 0;
        let mut max_t = 0;
        for note in &note_sequence {
            if note.h > max_h {
                max_h = note.h;
            }
            if note.t > max_t {
                max_t = note.t;
            }
        }
        let t = max_h.max(max_t) + 1;
        let t = if t > 1000000 { 1000000 } else { t };

        // Parallel computation of sections
        let (j_bar, delta_ks) = Self::calculate_section_23(k, &note_seq_by_column, t, x);
        let x_bar = Self::calculate_section_24(k, t, &note_seq_by_column, x);
        let p_bar = Self::calculate_section_25(t, &ln_seq, &note_sequence, x);

        let (a_bar, ks): (Vec<f64>, Vec<i32>) =
            Self::calculate_section_26(&delta_ks, k, t, &note_sequence);
        let (r_bar, _): (Vec<f64>, Vec<f64>) =
            Self::calculate_section_27(&ln_seq, &ln_seq, t, &note_seq_by_column, x);

        let result = Self::calculate_section_3(
            j_bar,
            x_bar,
            p_bar,
            a_bar,
            r_bar,
            ks,
            t,
            &note_sequence,
            &ln_seq,
        );

        Ok(result)
    }

    fn calculate_section_23(
        k: i32,
        note_seq_by_column: &[Vec<Note>],
        t: i32,
        x: f64,
    ) -> (Vec<f64>, Vec<Vec<f64>>) {
        let mut j_ks: Vec<Vec<f64>> = vec![vec![0.0; t as usize]; k as usize];
        let mut delta_ks: Vec<Vec<f64>> = vec![vec![1e9; t as usize]; k as usize];

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

        let j_bar_ks: Vec<Vec<f64>> = j_ks.iter().map(|jk| Self::smooth(jk, t)).collect();

        let mut j_bar = vec![0.0; t as usize];
        for s in (0..t as usize).step_by(Self::GRANULARITY as usize) {
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

    fn calculate_section_24(k: i32, t: i32, note_seq_by_column: &[Vec<Note>], x: f64) -> Vec<f64> {
        let mut x_ks: Vec<Vec<f64>> = vec![vec![0.0; t as usize]; (k + 1) as usize];

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

        let mut x_arr = vec![0.0; t as usize];
        for s in (0..t as usize).step_by(Self::GRANULARITY as usize) {
            x_arr[s] = 0.0;
            for k_idx in 0..=(k as usize) {
                x_arr[s] += x_ks[k_idx][s] * CrossMatrixProvider::get_matrix(k as usize).unwrap()[k_idx];
            }
        }

        Self::smooth(&x_arr, t)
    }

    fn calculate_section_25(t: i32, ln_seq: &[Note], note_seq: &[Note], x: f64) -> Vec<f64> {
        let mut p = vec![0.0; t as usize];
        let mut ln_bodies = vec![0.0; t as usize];

        ln_seq.par_iter().for_each(|note| {
            let t1 = (note.h + 80).min(note.t);
            for time in note.h..t1 {
                if time >= 0 && time < t {
                    // Atomic add would be needed for thread safety, but for simplicity using sequential
                }
            }
            for time in t1..note.t {
                if time >= 0 && time < t {
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

        let mut prefix_sum_ln_bodies = vec![0.0; (t + 1) as usize];
        for i in 1..=(t as usize) {
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

        Self::smooth(&p, t)
    }

    fn calculate_section_26(
        delta_ks: &[Vec<f64>],
        k: i32,
        t: i32,
        note_seq: &[Note],
    ) -> (Vec<f64>, Vec<i32>) {
        if delta_ks.is_empty() || k <= 0 || t <= 0 {
            return (vec![1.0; t.max(1) as usize], vec![1i32; t.max(1) as usize]);
        }
        let mut ku_ks: Vec<Vec<bool>> = vec![vec![false; t as usize]; k as usize];

        // Sequential processing for ku_ks
        for note in note_seq {
            let start_time = (note.h - 500).max(0) as usize;
            let end_time = if note.t < 0 {
                (note.h + 500).min(t - 1) as usize
            } else {
                (note.t + 500).min(t - 1) as usize
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

        let mut ks = vec![0i32; t as usize];
        let mut a = vec![1.0; t as usize];

        let mut dks: Vec<Vec<f64>> = vec![vec![0.0; t as usize]; k as usize];

        for s in (0..t as usize).step_by(Self::GRANULARITY as usize) {
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

        let a_bar = Self::smooth(&a, t);
        (a_bar, ks)
    }

    fn calculate_section_27(
        ln_seq: &[Note],
        tail_seq: &[Note],
        t: i32,
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

        let mut is_arr = vec![0.0; t as usize];
        let mut r = vec![0.0; t as usize];

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

        let r_bar = Self::smooth(&r, t);
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

    fn calculate_section_3(
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
}

#[no_mangle]
pub extern "C" fn calculate_sr_from_json(json_ptr: *const u8, len: usize) -> *mut u8 {
    // FFI interface for C#
    let json_bytes = unsafe { std::slice::from_raw_parts(json_ptr, len) };

    let json_str = match std::str::from_utf8(json_bytes) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let beatmap: Beatmap = match serde_json::from_str(json_str) {
        Ok(b) => b,
        Err(_) => return std::ptr::null_mut(),
    };

    let sr = match SRCalculator::calculate_sr(&beatmap) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let result = json!({ "sr": sr });
    let result_str = result.to_string();
    let c_str = std::ffi::CString::new(result_str).unwrap();
    c_str.into_raw() as *mut u8
}

#[repr(C)]
pub struct CHitObject {
    pub col: i32,
    pub start_time: i32,
    pub end_time: i32,
}

#[repr(C)]
pub struct CBeatmapData {
    pub overall_difficulty: f64,
    pub circle_size: f64,
    pub hit_objects_count: usize,
    pub hit_objects_ptr: *const CHitObject,
}

#[no_mangle]
pub extern "C" fn calculate_sr_from_struct(data: *const CBeatmapData) -> f64 {
    if data.is_null() {
        return -1.0;
    }

    let c_data = unsafe { &*data };

    // Convert C structures to Rust structures
    let hit_objects: Vec<HitObject> =
        unsafe { std::slice::from_raw_parts(c_data.hit_objects_ptr, c_data.hit_objects_count) }
            .iter()
            .map(|ho| HitObject {
                position: Position { x: ho.col as f64 }, // Use col as x for compatibility, but actually use col directly
                start_time: ho.start_time,
                end_time: ho.end_time,
            })
            .collect();

    let beatmap = Beatmap {
        difficulty_section: DifficultySection {
            overall_difficulty: c_data.overall_difficulty,
            circle_size: c_data.circle_size,
        },
        hit_objects,
    };

    SRCalculator::calculate_sr(&beatmap).unwrap_or_else(|_| -1.0)
}

#[derive(Serialize, Deserialize)]
pub struct SRData {
    pub overall_difficulty: f64,
    pub circle_size: f64,
    pub hit_objects: Vec<HitObject>,
}

impl SRData {
    pub fn from_osu_content(content: &str) -> Result<Self, String> {
        let mut overall_difficulty = 0.0;
        let mut circle_size = 0.0;
        let mut hit_objects = Vec::new();
        let mut in_difficulty_section = false;
        let mut in_hit_objects_section = false;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                let section = &line[1..line.len() - 1];
                in_difficulty_section = section == "Difficulty";
                in_hit_objects_section = section == "HitObjects";
                continue;
            }

            if in_difficulty_section {
                if let Some((key, value)) = parse_key_value(line) {
                    match key.as_str() {
                        "OverallDifficulty" => {
                            overall_difficulty =
                                value.parse().map_err(|_| "Invalid OverallDifficulty")?;
                        }
                        "CircleSize" => {
                            circle_size = value.parse().map_err(|_| "Invalid CircleSize")?;
                        }
                        _ => {}
                    }
                }
            } else if in_hit_objects_section {
                if let Some(obj) = parse_hit_object(line, circle_size) {
                    hit_objects.push(obj);
                }
            }
        }

        Ok(SRData {
            overall_difficulty,
            circle_size,
            hit_objects,
        })
    }

    pub fn from_osu_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
        Self::from_osu_content(&content)
    }
}

fn parse_key_value(line: &str) -> Option<(String, String)> {
    let colon_pos = line.find(':')?;
    let key = line[..colon_pos].trim().to_string();
    let value = line[colon_pos + 1..].trim().to_string();
    Some((key, value))
}

fn parse_hit_object(line: &str, circle_size: f64) -> Option<HitObject> {
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() < 4 {
        return None;
    }

    let x: f64 = parts[0].parse().ok()?;
    let start_time: i32 = parts[2].parse().ok()?;
    let obj_type: i32 = parts[3].parse().ok()?;

    // For mania, calculate column from x using reverse of editor formula
    let col = ((x * circle_size) / 512.0).floor() as i32;

    // Check if it's a long note (bit 7 set in type)
    let is_long_note = (obj_type & 128) != 0;
    let mut end_time = if is_long_note && parts.len() >= 6 {
        parts[5].split(':').next().unwrap_or("").parse().unwrap_or(-1)
    } else {
        -1
    };

    // For notes (not long notes), set end_time to start_time
    if end_time == -1 {
        end_time = start_time;
    }

    Some(HitObject {
        position: Position { x: col as f64 },
        start_time,
        end_time,
    })
}

#[no_mangle]
pub extern "C" fn calculate_sr_from_osu_content(content_ptr: *const u8, len: usize) -> *mut u8 {
    let content_bytes = unsafe { std::slice::from_raw_parts(content_ptr, len) };

    let content = match std::str::from_utf8(content_bytes) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let sr_data = match SRData::from_osu_content(content) {
        Ok(data) => {
            eprintln!("Rust parsed {} hit objects", data.hit_objects.len());
            data
        },
        Err(e) => {
            eprintln!("Rust parse error: {}", e);
            return std::ptr::null_mut();
        }
    };

    let beatmap = Beatmap {
        difficulty_section: DifficultySection {
            overall_difficulty: sr_data.overall_difficulty,
            circle_size: sr_data.circle_size,
        },
        hit_objects: sr_data.hit_objects,
    };

    let sr = match SRCalculator::calculate_sr(&beatmap) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let result = json!({ "sr": sr });
    let result_str = result.to_string();
    let c_str = std::ffi::CString::new(result_str).unwrap();
    c_str.into_raw() as *mut u8
}

#[no_mangle]
pub extern "C" fn calculate_sr_from_osu_file(path_ptr: *const u8, len: usize) -> *mut u8 {
    let path_bytes = unsafe { std::slice::from_raw_parts(path_ptr, len) };

    let path = match std::str::from_utf8(path_bytes) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let sr_data = match SRData::from_osu_file(path) {
        Ok(data) => data,
        Err(_) => return std::ptr::null_mut(),
    };

    let beatmap = Beatmap {
        difficulty_section: DifficultySection {
            overall_difficulty: sr_data.overall_difficulty,
            circle_size: sr_data.circle_size,
        },
        hit_objects: sr_data.hit_objects,
    };

    let sr = match SRCalculator::calculate_sr(&beatmap) {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let notes: Vec<_> = beatmap.hit_objects.iter().take(20).map(|ho| json!({
        "col": ho.position.x as i32,
        "start": ho.start_time,
        "end": ho.end_time
    })).collect();

    let result_str = json!({
        "sr": sr,
        "notes": notes
    }).to_string();
    let c_str = std::ffi::CString::new(result_str).unwrap();
    c_str.into_raw() as *mut u8
}
