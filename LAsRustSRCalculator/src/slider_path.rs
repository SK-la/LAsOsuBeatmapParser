// Copyright (c) 2025 ppy Pty Ltd <contact@ppy.sh>.
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

//! Analysis geometry for legacy sliders. Based on Ez2Lazer's SliderPath and the MIT-licensed
//! osu-framework PathApproximator at 40e3e14f5df4ad7ef14b104b81b7297316493d75.

use std::fmt;

use crate::beatmap::{SliderCurve, SliderCurveType};

const FIRST_LAZER_VERSION: u32 = 128;
const BEZIER_TOLERANCE: f32 = 0.25;
const CIRCULAR_ARC_TOLERANCE: f32 = 0.1;
const CATMULL_DETAIL: usize = 50;
const DISTANCE_EPSILON: f64 = 1e-7;
const COORDINATE_EPSILON: f32 = 1e-3;
const MAX_BEZIER_DEPTH: usize = 64;
const MAX_PATH_VERTICES: usize = 1_000_000;

type Point = [f32; 2];

/// Geometry type of a path segment. The control-point tag marks the start of the segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplineType {
    Linear,
    Bezier,
    BSpline(usize),
    Catmull,
    Perfect,
}

/// Lazer-style path control point with a relative coordinate and optional segment type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathControlPoint {
    pub position: [f32; 2],
    pub kind: Option<SplineType>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SliderPathOptions {
    /// Enable the stable Catmull simplification used by osu!standard.
    pub optimise_catmull: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliderPathError {
    NonFinitePoint,
    NonFiniteDistance,
    GeometryOverflow,
    ApproximationDidNotConverge,
    PathTooComplex,
    InvalidSplineDegree,
    InvalidSegment,
    UnsupportedCurveType(char),
}

impl fmt::Display for SliderPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinitePoint => f.write_str("slider path coordinate must be finite"),
            Self::NonFiniteDistance => f.write_str("expected slider distance must be finite"),
            Self::GeometryOverflow => f.write_str("slider path calculation overflowed"),
            Self::ApproximationDidNotConverge => {
                f.write_str("slider curve approximation did not converge")
            }
            Self::PathTooComplex => {
                write!(
                    f,
                    "slider path exceeds {MAX_PATH_VERTICES} approximation vertices"
                )
            }
            Self::InvalidSplineDegree => f.write_str("slider spline degree must be positive"),
            Self::InvalidSegment => {
                f.write_str("slider path has an empty or invalid explicit segment")
            }
            Self::UnsupportedCurveType(kind) => write!(f, "unsupported slider curve type {kind}"),
        }
    }
}

impl std::error::Error for SliderPathError {}

/// A slider path sampled into a piecewise linear approximation with cumulative arc length.
#[derive(Clone, Debug, PartialEq)]
pub struct SliderPath {
    vertices: Vec<Point>,
    cumulative_length: Vec<f64>,
    calculated_distance: f64,
}

impl SliderPath {
    /// Builds a legacy path from its source tags and normalized coordinates after the implicit head.
    pub fn from_legacy_curve(
        curve: &SliderCurve,
        relative_points: &[[f64; 2]],
        expected_distance: Option<f64>,
        version: u32,
        options: SliderPathOptions,
    ) -> Result<Self, SliderPathError> {
        if curve.segment_count <= 1 {
            return Self::from_control_points_with_options(
                curve.curve_type,
                relative_points,
                expected_distance,
                version,
                options,
            );
        }
        if relative_points.len() >= MAX_PATH_VERTICES {
            return Err(SliderPathError::PathTooComplex);
        }
        let mut points = Vec::with_capacity(relative_points.len() + 1);
        points.push([0.0, 0.0]);
        points.extend(relative_points.iter().map(|p| [p[0] as f32, p[1] as f32]));
        let mut segments = vec![(0, spline_kind(curve.curve_type)?)];
        let mut next_point = 1;
        for token in curve.raw_path.split('|').skip(1) {
            if token.is_empty() {
                continue;
            }
            if token.chars().next().is_some_and(char::is_alphabetic) {
                let kind = match token.as_bytes().first() {
                    Some(b'B') => token[1..]
                        .parse::<usize>()
                        .ok()
                        .filter(|degree| *degree > 0)
                        .map_or(SplineType::Bezier, SplineType::BSpline),
                    Some(b'L') => SplineType::Linear,
                    Some(b'P') => SplineType::Perfect,
                    _ => SplineType::Catmull,
                };
                if segments
                    .last()
                    .is_some_and(|(start, _)| *start >= next_point)
                {
                    return Err(SliderPathError::InvalidSegment);
                }
                segments.push((next_point, kind));
            } else {
                next_point += 1;
            }
        }
        if next_point != points.len()
            || segments
                .last()
                .is_some_and(|(start, _)| *start >= points.len())
        {
            return Err(SliderPathError::InvalidSegment);
        }
        let mut typed = Vec::with_capacity(points.len());
        for (index, &(start, kind)) in segments.iter().enumerate() {
            let end = segments
                .get(index + 1)
                .map_or(points.len(), |segment| segment.0);
            append_legacy_controls(
                &mut typed,
                &points[start..end],
                points.get(end).copied(),
                kind,
                version,
            );
        }
        Self::from_typed_control_points(&typed, expected_distance, options)
    }
    /// Creates a path from the relative legacy coordinates following the implicit slider head.
    ///
    /// This input represents a single legacy curve tag. Use [Self::from_typed_control_points] for
    /// explicit segment tags or a B-spline degree. Repeated anchors split implicit segments according
    /// to the source format version.
    pub fn from_control_points(
        curve_type: SliderCurveType,
        path_control_points: &[[f64; 2]],
        expected_distance: Option<f64>,
        format_version: u32,
    ) -> Result<Self, SliderPathError> {
        Self::from_control_points_with_options(
            curve_type,
            path_control_points,
            expected_distance,
            format_version,
            SliderPathOptions::default(),
        )
    }

    pub fn from_control_points_with_options(
        curve_type: SliderCurveType,
        path_control_points: &[[f64; 2]],
        expected_distance: Option<f64>,
        format_version: u32,
        options: SliderPathOptions,
    ) -> Result<Self, SliderPathError> {
        let kind = spline_kind(curve_type)?;
        let mut points = Vec::with_capacity(path_control_points.len() + 1);
        points.push([0.0, 0.0]);
        for &[x, y] in path_control_points {
            let point = [x as f32, y as f32];
            if !is_finite(point) {
                return Err(SliderPathError::NonFinitePoint);
            }
            points.push(point);
        }
        let mut typed = Vec::with_capacity(points.len());
        append_legacy_controls(&mut typed, &points, None, kind, format_version);
        Self::from_typed_control_points(&typed, expected_distance, options)
    }

    /// Creates a path from already-parsed control points including the slider head.
    /// Use a typed point to share its position as the end of one segment and the start of the next.
    pub fn from_typed_control_points(
        control_points: &[PathControlPoint],
        expected_distance: Option<f64>,
        options: SliderPathOptions,
    ) -> Result<Self, SliderPathError> {
        if control_points.len() > MAX_PATH_VERTICES {
            return Err(SliderPathError::PathTooComplex);
        }
        if expected_distance.is_some_and(|distance| !distance.is_finite()) {
            return Err(SliderPathError::NonFiniteDistance);
        }
        if control_points
            .iter()
            .any(|point| !is_finite(point.position))
        {
            return Err(SliderPathError::NonFinitePoint);
        }
        if control_points
            .iter()
            .any(|point| point.kind == Some(SplineType::BSpline(0)))
        {
            return Err(SliderPathError::InvalidSplineDegree);
        }

        let positions: Vec<_> = control_points.iter().map(|point| point.position).collect();
        let mut vertices = Vec::new();
        let mut optimised_length = 0.0;
        let mut start = 0;
        for (end, point) in control_points.iter().enumerate() {
            if point.kind.is_none() && end < control_points.len() - 1 {
                continue;
            }

            let segment = &positions[start..=end];
            let kind = control_points[start].kind.unwrap_or(SplineType::Linear);
            let subpath = match (segment.len(), kind) {
                (1, _) | (_, SplineType::Linear) => segment.to_vec(),
                (_, SplineType::Catmull) => {
                    let path = catmull_path(segment)?;
                    if options.optimise_catmull {
                        optimise_catmull(path, &mut optimised_length)
                    } else {
                        path
                    }
                }
                (_, SplineType::Perfect) => match circular_arc(segment)? {
                    Some(path) => path,
                    None => bspline_path(segment, segment.len() - 1)?,
                },
                (_, SplineType::BSpline(degree)) => bspline_path(segment, degree)?,
                (_, SplineType::Bezier) => bspline_path(segment, segment.len() - 1)?,
            };
            append_subpath(&mut vertices, subpath)?;
            start = end;
        }

        Self::from_vertices(vertices, expected_distance, optimised_length)
    }

    fn from_vertices(
        mut vertices: Vec<Point>,
        expected_distance: Option<f64>,
        optimised_length: f64,
    ) -> Result<Self, SliderPathError> {
        if vertices.iter().any(|point| !is_finite(*point)) {
            return Err(SliderPathError::GeometryOverflow);
        }

        let mut calculated_distance = optimised_length;
        let mut cumulative_length = Vec::with_capacity(vertices.len().max(1));
        cumulative_length.push(0.0);
        for edge in vertices.windows(2) {
            calculated_distance += f64::from(length(sub(edge[1], edge[0])));
            cumulative_length.push(calculated_distance);
        }
        if !calculated_distance.is_finite() {
            return Err(SliderPathError::GeometryOverflow);
        }

        if let Some(expected_distance) =
            expected_distance.filter(|distance| *distance != calculated_distance)
        {
            let terminal_edge_repeated =
                matches!(vertices.as_slice(), [.., penultimate, last] if penultimate == last);
            let skip_extension = terminal_edge_repeated && expected_distance > calculated_distance;
            if !skip_extension {
                cumulative_length.pop();
                if calculated_distance > expected_distance {
                    while cumulative_length
                        .last()
                        .is_some_and(|last| *last >= expected_distance)
                    {
                        cumulative_length.pop();
                        vertices.pop();
                    }
                }
                if let [.., penultimate, last] = vertices.as_mut_slice() {
                    let edge = sub(*last, *penultimate);
                    let direction = scale(edge, 1.0_f32 / length(edge));
                    let remaining = (expected_distance - cumulative_length.last().unwrap()) as f32;
                    *last = add(*penultimate, scale(direction, remaining));
                    if !is_finite(*last) {
                        return Err(SliderPathError::GeometryOverflow);
                    }
                    cumulative_length.push(expected_distance);
                } else {
                    cumulative_length.push(0.0);
                }
            }
        }

        Ok(Self {
            vertices,
            cumulative_length,
            calculated_distance,
        })
    }

    /// Arc length after applying the expected distance.
    pub fn distance(&self) -> f64 {
        self.cumulative_length.last().copied().unwrap_or_default()
    }

    /// Arc length of the approximated curve before applying the expected distance.
    pub fn calculated_distance(&self) -> f64 {
        self.calculated_distance
    }

    pub fn vertices(&self) -> &[[f32; 2]] {
        &self.vertices
    }

    pub(crate) fn reflect_horizontally(&mut self) {
        for vertex in &mut self.vertices {
            vertex[0] = -vertex[0];
        }
    }

    /// Returns the relative position at the given fraction of arc length.
    pub fn position_at(&self, progress: f64) -> [f32; 2] {
        let distance = progress.clamp(0.0, 1.0) * self.distance();
        let index = distance_index(&self.cumulative_length, distance);
        if index == 0 {
            return self.vertices.first().copied().unwrap_or_default();
        }
        if index >= self.vertices.len() {
            return self.vertices.last().copied().unwrap_or_default();
        }

        let start = self.vertices[index - 1];
        let distance_start = self.cumulative_length[index - 1];
        let distance_end = self.cumulative_length[index];
        let length = distance_end - distance_start;
        if length.abs() <= DISTANCE_EPSILON {
            return start;
        }

        let fraction = ((distance - distance_start) / length) as f32;
        add(start, scale(sub(self.vertices[index], start), fraction))
    }
}

fn spline_kind(curve_type: SliderCurveType) -> Result<SplineType, SliderPathError> {
    Ok(match curve_type {
        SliderCurveType::Bezier => SplineType::Bezier,
        SliderCurveType::BSpline(degree) => SplineType::BSpline(degree),
        SliderCurveType::Catmull | SliderCurveType::Other(_) => SplineType::Catmull,
        SliderCurveType::Linear => SplineType::Linear,
        SliderCurveType::Perfect => SplineType::Perfect,
    })
}

fn append_legacy_controls(
    destination: &mut Vec<PathControlPoint>,
    points: &[Point],
    endpoint: Option<Point>,
    mut kind: SplineType,
    version: u32,
) {
    if kind == SplineType::Perfect {
        let count = points.len() + usize::from(endpoint.is_some());
        if count > 3 || (version < FIRST_LAZER_VERSION && count != 3) {
            kind = SplineType::Bezier;
        } else if version < FIRST_LAZER_VERSION
            && is_collinear(points[0], points[1], endpoint.unwrap_or_else(|| points[2]))
        {
            kind = SplineType::Linear;
        }
    }

    let mut start = 0;
    for index in 1..points.len().saturating_sub(1) {
        if points[index] != points[index - 1]
            || (kind == SplineType::Catmull && index > 1 && version < FIRST_LAZER_VERSION)
        {
            continue;
        }
        for current in start..index {
            destination.push(PathControlPoint {
                position: points[current],
                kind: (current == 0 || current == index - 1).then_some(kind),
            });
        }
        start = index + 1;
    }
    destination.extend(
        points
            .iter()
            .enumerate()
            .skip(start)
            .map(|(index, &position)| PathControlPoint {
                position,
                kind: (index == 0).then_some(kind),
            }),
    );
}

fn distance_index(lengths: &[f64], distance: f64) -> usize {
    if distance.is_nan() {
        return 0;
    }
    // Match List<double>.BinarySearch, including which equal vertex it selects on a plateau.
    let (mut low, mut high) = (0, lengths.len());
    while low < high {
        let mid = low + (high - low - 1) / 2;
        match lengths[mid].partial_cmp(&distance).unwrap() {
            std::cmp::Ordering::Less => low = mid + 1,
            std::cmp::Ordering::Greater => high = mid,
            std::cmp::Ordering::Equal => return mid,
        }
    }
    low
}

fn append_subpath(vertices: &mut Vec<Point>, subpath: Vec<Point>) -> Result<(), SliderPathError> {
    let skip_first = usize::from(vertices.last().is_some() && vertices.last() == subpath.first());
    let additional = subpath.len().saturating_sub(skip_first);
    if additional > MAX_PATH_VERTICES.saturating_sub(vertices.len()) {
        return Err(SliderPathError::PathTooComplex);
    }
    vertices.extend(subpath.into_iter().skip(skip_first));
    Ok(())
}

fn bspline_path(points: &[Point], degree: usize) -> Result<Vec<Point>, SliderPathError> {
    let last = points.len() - 1;
    let degree = degree.min(last);
    let mut vertices = Vec::new();
    if degree == last {
        append_bezier(points.to_vec(), &mut vertices)?;
    } else {
        // Insert internal knots into the clamped uniform spline using Boehm's algorithm.
        let mut points = points.to_vec();
        for knot in 0..last - degree {
            let mut bezier = vec![[0.0; 2]; degree + 1];
            bezier[0] = points[knot];
            for split in 0..degree - 1 {
                bezier[split + 1] = points[knot + 1];
                for step in 1..degree - split {
                    let weight = step.min(last - degree - knot) as f32;
                    let current = knot + step;
                    points[current] = divide(
                        add(scale(points[current], weight), points[current + 1]),
                        weight + 1.0,
                    );
                }
            }
            bezier[degree] = points[knot + 1];
            append_bezier(bezier, &mut vertices)?;
        }
        append_bezier(points[last - degree..].to_vec(), &mut vertices)?;
    }
    vertices.push(points[last]);
    Ok(vertices)
}

fn append_bezier(points: Vec<Point>, output: &mut Vec<Point>) -> Result<(), SliderPathError> {
    let count = points.len();
    if count == 2 {
        output.push(points[0]);
        return Ok(());
    }
    let mut midpoints = vec![[0.0; 2]; count];
    let mut right = vec![[0.0; 2]; count];
    let mut combined = vec![[0.0; 2]; count * 2 - 1];
    let mut stack = vec![(points, 0)];
    let mut unused = Vec::new();

    while let Some((mut front, depth)) = stack.pop() {
        if output.len().saturating_add(count) > MAX_PATH_VERTICES {
            return Err(SliderPathError::PathTooComplex);
        }
        let flat = front.windows(3).all(|points| {
            length_squared(add(sub(points[0], scale(points[1], 2.0)), points[2]))
                <= BEZIER_TOLERANCE * BEZIER_TOLERANCE * 4.0
        });
        if !flat && depth >= MAX_BEZIER_DEPTH {
            return Err(SliderPathError::ApproximationDidNotConverge);
        }

        subdivide_bezier(&front, &mut combined, &mut right, &mut midpoints);
        if !is_finite(combined[count - 1]) {
            return Err(SliderPathError::GeometryOverflow);
        }
        if flat {
            combined[count..].copy_from_slice(&right[1..]);
            output.push(front[0]);
            for index in 1..count - 1 {
                let index = 2 * index;
                output.push(scale(
                    add(
                        add(combined[index - 1], scale(combined[index], 2.0)),
                        combined[index + 1],
                    ),
                    0.25,
                ));
            }
            unused.push(front);
        } else {
            front.copy_from_slice(&combined[..count]);
            let mut next = unused.pop().unwrap_or_else(|| vec![[0.0; 2]; count]);
            next.copy_from_slice(&right);
            stack.push((next, depth + 1));
            stack.push((front, depth + 1));
        }
    }
    Ok(())
}

fn subdivide_bezier(
    points: &[Point],
    left: &mut [Point],
    right: &mut [Point],
    midpoints: &mut [Point],
) {
    midpoints.copy_from_slice(points);
    for index in 0..points.len() {
        let reverse = points.len() - index - 1;
        left[index] = midpoints[0];
        right[reverse] = midpoints[reverse];
        for point in 0..reverse {
            midpoints[point] = divide(add(midpoints[point], midpoints[point + 1]), 2.0);
        }
    }
}

fn catmull_path(points: &[Point]) -> Result<Vec<Point>, SliderPathError> {
    let capacity = (points.len() - 1)
        .checked_mul(CATMULL_DETAIL * 2)
        .filter(|count| *count <= MAX_PATH_VERTICES)
        .ok_or(SliderPathError::PathTooComplex)?;
    let mut vertices = Vec::with_capacity(capacity);
    for index in 0..points.len() - 1 {
        let previous = points[index.saturating_sub(1)];
        let start = points[index];
        let end = points[index + 1];
        let next = points
            .get(index + 2)
            .copied()
            .unwrap_or_else(|| sub(add(end, end), start));
        for sample in 0..CATMULL_DETAIL {
            for step in [sample, sample + 1] {
                let t = step as f32 / CATMULL_DETAIL as f32;
                let t2 = t * t;
                let t3 = t * t2;
                let axis = |axis: usize| {
                    let a = previous[axis];
                    let b = start[axis];
                    let c = end[axis];
                    let d = next[axis];
                    0.5 * (2.0 * b
                        + (-a + c) * t
                        + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                        + (-a + 3.0 * b - 3.0 * c + d) * t3)
                };
                vertices.push([axis(0), axis(1)]);
            }
        }
    }
    Ok(vertices)
}

fn optimise_catmull(path: Vec<Point>, optimised_length: &mut f64) -> Vec<Point> {
    let mut optimised = Vec::new();
    let mut start = None;
    let mut removed_length = 0.0;
    for (index, &point) in path.iter().enumerate() {
        let Some(segment_start) = start else {
            optimised.push(point);
            start = Some(point);
            continue;
        };

        let distance = f64::from(length(sub(point, segment_start)));
        removed_length += f64::from(length(sub(point, path[index - 1])));
        if distance > 6.0 || (index + 1) % (CATMULL_DETAIL * 2) == 0 || index == path.len() - 1 {
            optimised.push(point);
            *optimised_length += removed_length - distance;
            start = None;
            removed_length = 0.0;
        }
    }
    optimised
}

fn circular_arc(points: &[Point]) -> Result<Option<Vec<Point>>, SliderPathError> {
    let &[a, b, c] = points else { return Ok(None) };
    if is_collinear(a, b, c) {
        return Ok(None);
    }

    let bc = sub(b, c);
    let ca = sub(c, a);
    let ab = sub(a, b);
    let denominator = 2.0 * (a[0] * bc[1] + b[0] * ca[1] + c[0] * ab[1]);
    let (a2, b2, c2) = (length_squared(a), length_squared(b), length_squared(c));
    let centre = divide(
        [
            a2 * bc[1] + b2 * ca[1] + c2 * ab[1],
            a2 * -bc[0] + b2 * -ca[0] + c2 * -ab[0],
        ],
        denominator,
    );
    let da = sub(a, centre);
    let dc = sub(c, centre);
    let radius = length(da);
    if !radius.is_finite() || !is_finite(centre) {
        return Err(SliderPathError::GeometryOverflow);
    }

    let theta_start = f64::from(da[1]).atan2(f64::from(da[0]));
    let mut theta_end = f64::from(dc[1]).atan2(f64::from(dc[0]));
    if theta_end < theta_start {
        theta_end += std::f64::consts::TAU;
    }
    let ba = sub(b, a);
    let (direction, theta_range) = if ca[1] * ba[0] + -ca[0] * ba[1] < 0.0 {
        (-1.0, std::f64::consts::TAU - (theta_end - theta_start))
    } else {
        (1.0, theta_end - theta_start)
    };

    let count = if 2.0_f32 * radius <= CIRCULAR_ARC_TOLERANCE {
        2
    } else {
        let theta_tolerance = 2.0 * f64::from(1.0_f32 - CIRCULAR_ARC_TOLERANCE / radius).acos();
        (theta_range / theta_tolerance).ceil().max(2.0) as usize
    };
    if count >= 1000 {
        return Ok(None);
    }
    Ok(Some(
        (0..count)
            .map(|index| {
                let fraction = index as f64 / (count - 1) as f64;
                let theta = theta_start + direction * fraction * theta_range;
                add(
                    centre,
                    scale([theta.cos() as f32, theta.sin() as f32], radius),
                )
            })
            .collect(),
    ))
}

fn is_collinear(a: Point, b: Point, c: Point) -> bool {
    ((b[1] - a[1]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[1] - a[1])).abs() <= COORDINATE_EPSILON
}

fn is_finite(point: Point) -> bool {
    point[0].is_finite() && point[1].is_finite()
}

fn length_squared(point: Point) -> f32 {
    point[0] * point[0] + point[1] * point[1]
}

fn length(point: Point) -> f32 {
    f64::from(length_squared(point)).sqrt() as f32
}

fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1]]
}

fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1]]
}

fn scale(point: Point, scalar: f32) -> Point {
    [point[0] * scalar, point[1] * scalar]
}

fn divide(point: Point, scalar: f32) -> Point {
    [point[0] / scalar, point[1] / scalar]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_paths_adjust_the_last_edge_at_the_expected_distance() {
        let points = [[30.0, 40.0], [60.0, 40.0]];
        let build = |length| {
            SliderPath::from_control_points(SliderCurveType::Linear, &points, length, 14).unwrap()
        };
        let natural = build(None);
        assert_eq!(natural.calculated_distance(), 80.0);
        assert_eq!(natural.position_at(0.625), [30.0, 40.0]);

        let shortened = build(Some(25.0));
        assert_eq!(shortened.calculated_distance(), 80.0);
        assert_eq!(shortened.distance(), 25.0);
        // osuTK normalises by multiplying each axis by the precomputed f32 reciprocal.
        assert_eq!(shortened.position_at(1.0), [14.999_999, 19.999_998]);

        let extended = build(Some(100.0));
        assert_eq!(extended.distance(), 100.0);
        assert_eq!(extended.position_at(1.0), [80.0, 40.0]);
        assert_eq!(extended.position_at(1.5), [80.0, 40.0]);
        assert_eq!(build(Some(0.0)).position_at(1.0), [0.0, 0.0]);
    }

    #[test]
    fn repeated_interior_bezier_anchors_form_distinct_segments() {
        let path = SliderPath::from_control_points(
            SliderCurveType::Bezier,
            &[[480.0, 0.0], [480.0, 0.0], [0.0, 0.0]],
            Some(960.0),
            14,
        )
        .unwrap();
        assert_eq!(path.vertices(), &[[0.0, 0.0], [480.0, 0.0], [0.0, 0.0]]);
        assert_eq!(path.position_at(0.5), [480.0, 0.0]);
        assert_eq!(path.position_at(0.75), [240.0, 0.0]);
    }

    #[test]
    fn catch_diffcalc_fixture_repeated_anchor_bezier_matches_its_declared_distance() {
        let beatmap = crate::beatmap::OsuBeatmap::parse_bytes(include_bytes!(
            "../tests/fixtures/catch-diffcalc-test.osu"
        ))
        .unwrap();
        let slider = beatmap
            .hit_objects
            .iter()
            .find(|object| {
                matches!(
                    &object.kind,
                    crate::beatmap::HitObjectKind::Slider {
                        curve,
                        pixel_length: 960.0,
                        ..
                    } if curve.curve_type == SliderCurveType::Bezier && curve.points.len() == 3
                )
            })
            .expect("catch golden contains a Bezier loop slider");
        let crate::beatmap::HitObjectKind::Slider {
            curve,
            pixel_length,
            ..
        } = &slider.kind
        else {
            unreachable!()
        };
        let path_points: Vec<_> = curve
            .points
            .iter()
            .map(|point| {
                [
                    (point[0] as f32 - slider.position[0] as f32) as f64,
                    (point[1] as f32 - slider.position[1] as f32) as f64,
                ]
            })
            .collect();
        let path = SliderPath::from_control_points(
            curve.curve_type,
            &path_points,
            Some(*pixel_length),
            beatmap.version,
        )
        .unwrap();

        assert_eq!(path.distance(), 960.0);
        assert_eq!(path.vertices(), &[[0.0, 0.0], [480.0, 0.0], [0.0, 0.0]]);
    }

    #[test]
    fn catch_diffcalc_fixture_paths_resolve_expected_distance_or_stable_repeat_exception() {
        let beatmap = crate::beatmap::OsuBeatmap::parse_bytes(include_bytes!(
            "../tests/fixtures/catch-diffcalc-test.osu"
        ))
        .unwrap();
        let mut slider_count = 0;
        for slider in &beatmap.hit_objects {
            let crate::beatmap::HitObjectKind::Slider {
                curve,
                pixel_length,
                ..
            } = &slider.kind
            else {
                continue;
            };
            slider_count += 1;
            let points: Vec<_> = curve
                .points
                .iter()
                .map(|point| {
                    [
                        (point[0] as f32 - slider.position[0] as f32) as f64,
                        (point[1] as f32 - slider.position[1] as f32) as f64,
                    ]
                })
                .collect();
            let path = SliderPath::from_control_points(
                curve.curve_type,
                &points,
                Some(*pixel_length),
                beatmap.version,
            )
            .unwrap();
            if (path.distance() - *pixel_length).abs() > 1e-5 {
                assert!(
                    path.distance() < *pixel_length,
                    "slider path may only be shorter when stable suppresses extension"
                );
                assert!(matches!(
                    path.vertices(),
                    [.., previous, last] if previous == last
                ));
            }
        }
        assert!(slider_count > 8);
    }

    #[test]
    fn repeated_terminal_vertices_only_block_extension_of_the_calculated_polyline() {
        let controls = &[[10.0, 0.0], [10.0, 0.0]];
        let linear =
            SliderPath::from_control_points(SliderCurveType::Linear, controls, Some(20.0), 14)
                .unwrap();
        assert_eq!(linear.distance(), 10.0);
        assert_eq!(linear.position_at(1.0), [10.0, 0.0]);
        let bezier =
            SliderPath::from_control_points(SliderCurveType::Bezier, controls, Some(20.0), 14)
                .unwrap();
        assert_eq!(bezier.distance(), 20.0);
        assert_eq!(bezier.position_at(1.0), [20.0, 0.0]);
    }

    #[test]
    fn flat_enough_bezier_uses_framework_intermediate_vertex() {
        let path = SliderPath::from_control_points(
            SliderCurveType::Bezier,
            &[[1.0, 0.25], [2.0, 0.0]],
            None,
            128,
        )
        .unwrap();
        assert_eq!(path.vertices(), &[[0.0, 0.0], [1.0, 0.125], [2.0, 0.0]]);
        assert_eq!(path.position_at(0.5), [1.0, 0.125]);
    }

    #[test]
    fn typed_bspline_degree_one_matches_a_polyline() {
        let typed = [
            PathControlPoint {
                position: [0.0, 0.0],
                kind: Some(SplineType::BSpline(1)),
            },
            PathControlPoint {
                position: [3.0, 4.0],
                kind: None,
            },
            PathControlPoint {
                position: [6.0, 8.0],
                kind: None,
            },
        ];
        let path =
            SliderPath::from_typed_control_points(&typed, None, SliderPathOptions::default())
                .unwrap();
        assert_eq!(path.distance(), 10.0);
        assert_eq!(path.position_at(0.5), [3.0, 4.0]);
    }

    #[test]
    fn stable_collinear_perfect_curves_preserve_control_points() {
        let points = &[[8.0, 0.0], [16.0, 0.0]];
        let stable =
            SliderPath::from_control_points(SliderCurveType::Perfect, points, None, 14).unwrap();
        assert_eq!(stable.vertices(), &[[0.0, 0.0], [8.0, 0.0], [16.0, 0.0]]);
        let arc = SliderPath::from_control_points(
            SliderCurveType::Perfect,
            &[[1.0, 1.0], [2.0, 0.0]],
            None,
            128,
        )
        .unwrap();
        assert!(arc.calculated_distance() > 2.8);
        assert!(arc.calculated_distance() <= std::f64::consts::PI);
        assert!((arc.position_at(0.5)[0] - 1.0).abs() < 1e-6);
        assert!(arc.position_at(0.5)[1] > 0.8);
    }
}
