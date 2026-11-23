use std::collections::HashMap;

pub(super) fn ratio(value: usize, count: usize) -> f64 {
    positive_ratio(value as f64, count as f64)
}

pub(super) fn positive_ratio(value: f64, divisor: f64) -> f64 {
    if divisor > 0.0 { value / divisor } else { 0.0 }
}

pub(super) fn average(values: &[f64]) -> f64 {
    positive_ratio(values.iter().sum(), values.len() as f64)
}

pub(super) fn quantile(values: &[f64], q: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut values = values.to_vec();
    let index = ((values.len() - 1) as f64 * q).floor() as usize;
    *values.select_nth_unstable_by(index, f64::total_cmp).1
}

pub(super) fn count_in_window(times: &[f64], window_ms: f64) -> usize {
    let mut best = 0;
    let mut start = 0;
    for end in 0..times.len() {
        while times[end] - times[start] > window_ms {
            start += 1;
        }
        best = best.max(end - start + 1);
    }
    best
}

fn bucket_counts(values: &[f64], bucket_size: f64) -> Vec<(f64, usize)> {
    let mut indices = HashMap::<u64, usize>::new();
    let mut counts = Vec::<(f64, usize)>::new();
    for &value in values {
        let bucket = (value / bucket_size).round_ties_even() * bucket_size;
        let index = indices.entry(bucket.to_bits()).or_insert_with(|| {
            counts.push((bucket, 0));
            counts.len() - 1
        });
        counts[*index].1 += 1;
    }
    counts
}

pub(super) fn bucket_entropy(values: &[f64], bucket_size: f64) -> f64 {
    let mut entropy = 0.0;
    for (_, count) in bucket_counts(values, bucket_size) {
        let probability = ratio(count, values.len());
        entropy -= probability * probability.log2();
    }
    entropy
}

pub(super) fn rao_quadratic_entropy_log(
    values: &[f64],
    bucket_size: f64,
    log_iterations: usize,
) -> f64 {
    let counts = bucket_counts(values, bucket_size);
    let total = values.len() as f64;
    let mut entropy = 0.0;
    for &(left, left_count) in &counts {
        for &(right, right_count) in &counts {
            let mut distance = (left - right).abs();
            for _ in 0..log_iterations {
                distance = (1.0 + distance).ln();
            }
            entropy += (left_count as f64 / total) * (right_count as f64 / total) * distance;
        }
    }
    entropy
}

pub(super) fn strain_spikiness(values: &[f64], weights: &[f64]) -> f64 {
    if values.len() < 3 || values.len() != weights.len() {
        return 0.0;
    }

    let weight_sum: f64 = weights.iter().sum();
    if weight_sum <= 0.0 {
        return 0.0;
    }
    let sum: f64 = values
        .iter()
        .zip(weights)
        .map(|(value, weight)| value.powf(5.0) * weight)
        .sum();
    let mean = (sum / weight_sum).powf(1.0 / 5.0);
    if mean <= 0.0 {
        return 0.0;
    }
    let variance = values
        .iter()
        .zip(weights)
        .map(|(value, weight)| {
            let difference = value.powf(8.0) - mean.powf(8.0);
            difference * difference * weight
        })
        .sum::<f64>()
        / weight_sum;
    variance.powf(1.0 / 8.0).sqrt() / mean
}
