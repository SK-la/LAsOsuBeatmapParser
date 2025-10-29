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

        let (all_corners, base_corners, a_corners) = Self::get_corners(t, &note_seq);

        // Get key usage
        let key_usage = Self::get_key_usage(k, t, &note_seq, &base_corners);
        let active_columns = Self::derive_active_columns(&key_usage);

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
