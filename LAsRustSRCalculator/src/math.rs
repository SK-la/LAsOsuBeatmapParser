pub fn cumulative_sum(x: &[f64], f: &[f64]) -> Vec<f64> {
    let mut f_cum = vec![0.0; x.len()];
    for i in 1..x.len() {
        f_cum[i] = f_cum[i - 1] + f[i - 1] * (x[i] - x[i - 1]);
    }
    f_cum
}

pub fn query_cumsum(q: f64, x: &[f64], f_cum: &[f64], f: &[f64]) -> f64 {
    if q <= x[0] {
        return 0.0;
    }
    if q >= x[x.len() - 1] {
        return f_cum[f_cum.len() - 1];
    }
    let i = x.partition_point(|&val| val < q) - 1;
    f_cum[i] + f[i] * (q - x[i])
}

pub fn smooth_on_corners(x: &[f64], f: &[f64], window: f64, scale: f64, mode: &str) -> Vec<f64> {
    let f_cum = cumulative_sum(x, f);
    let mut g = vec![0.0; f.len()];
    for i in 0..x.len() {
        let a = (x[i] - window).max(x[0]);
        let b = (x[i] + window).min(x[x.len() - 1]);
        let val = query_cumsum(b, x, &f_cum, f) - query_cumsum(a, x, &f_cum, f);
        g[i] = if mode == "avg" {
            if (b - a) > 0.0 { val / (b - a) } else { 0.0 }
        } else {
            scale * val
        };
    }
    g
}

pub fn interp_values(new_x: &[f64], old_x: &[f64], old_vals: &[f64]) -> Vec<f64> {
    new_x.iter().map(|&x| {
        if x <= old_x[0] {
            old_vals[0]
        } else if x >= old_x[old_x.len() - 1] {
            old_vals[old_vals.len() - 1]
        } else {
            let i = old_x.partition_point(|&val| val < x) - 1;
            let t = (x - old_x[i]) / (old_x[i + 1] - old_x[i]);
            old_vals[i] * (1.0 - t) + old_vals[i + 1] * t
        }
    }).collect()
}

pub fn step_interp(new_x: &[f64], old_x: &[f64], old_vals: &[f64]) -> Vec<f64> {
    new_x.iter().map(|&x| {
        let i = old_x.partition_point(|&val| val < x).saturating_sub(1);
        old_vals[i.min(old_vals.len() - 1)]
    }).collect()
}