//! Strict parser for the text-based osu! `.osu` beatmap format.
//!
//! The parser keeps raw timing/object data needed by analysis code and does not serialize through JSON.
//! It skips unknown and storyboard-only sections; slider duration calculation and external sample asset
//! resolution belong to later analysis/runtime stages.

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Cursor, Read};
use std::path::Path;

pub const MIN_SUPPORTED_VERSION: u32 = 1;
pub const MAX_SUPPORTED_VERSION: u32 = i32::MAX as u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum GameMode {
    Standard = 0,
    Taiko = 1,
    Catch = 2,
    Mania = 3,
}

impl TryFrom<i32> for GameMode {
    type Error = String;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Standard),
            1 => Ok(Self::Taiko),
            2 => Ok(Self::Catch),
            3 => Ok(Self::Mania),
            _ => Err(format!("unsupported game mode {value}; expected 0..=3")),
        }
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub line: Option<usize>,
    pub message: String,
    source: Option<io::Error>,
}

impl ParseError {
    fn at(line: usize, message: impl Into<String>) -> Self {
        Self {
            line: Some(line),
            message: message.into(),
            source: None,
        }
    }

    fn global(message: impl Into<String>) -> Self {
        Self {
            line: None,
            message: message.into(),
            source: None,
        }
    }

    fn io(error: io::Error) -> Self {
        Self {
            line: None,
            message: "failed to read beatmap".into(),
            source: Some(error),
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|error| error as &(dyn Error + 'static))
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GeneralSettings {
    pub audio_filename: Option<String>,
    pub audio_lead_in: Option<i32>,
    pub preview_time: Option<i32>,
    pub countdown: Option<i32>,
    pub sample_set: Option<String>,
    pub stack_leniency: Option<f64>,
    pub mode: GameMode,
}

impl Default for GameMode {
    fn default() -> Self {
        Self::Standard
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub creator: Option<String>,
    pub version: Option<String>,
    pub beatmap_id: Option<i32>,
    pub beatmap_set_id: Option<i32>,
    pub source: Option<String>,
    pub tags: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DifficultySettings {
    pub hp_drain_rate: Option<f64>,
    pub circle_size: Option<f64>,
    pub overall_difficulty: Option<f64>,
    pub approach_rate: Option<f64>,
    pub slider_multiplier: Option<f64>,
    pub slider_tick_rate: Option<f64>,
}

impl DifficultySettings {
    pub fn mania_key_count(&self) -> Option<usize> {
        let value = self.circle_size?;
        value
            .is_finite()
            .then(|| value.round_ties_even().clamp(1.0, 18.0) as usize)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimingPoint {
    pub time: f64,
    pub beat_length: f64,
    pub meter: i32,
    pub sample_set: i32,
    pub sample_index: i32,
    pub volume: i32,
    /// True for an uninherited (base timing) point.
    pub uninherited: bool,
    pub effects: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ControlPoints {
    pub timing: Vec<TimingControlPoint>,
    pub difficulty: Vec<DifficultyControlPoint>,
    pub sample: Vec<SampleControlPoint>,
    pub effect: Vec<EffectControlPoint>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimingControlPoint {
    pub time: f64,
    pub beat_length: f64,
    pub meter: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DifficultyControlPoint {
    pub time: f64,
    pub speed_multiplier: f64,
    pub generate_ticks: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SampleControlPoint {
    pub time: f64,
    pub sample_set: i32,
    pub sample_index: i32,
    pub volume: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectControlPoint {
    pub time: f64,
    pub kiai: bool,
    pub omit_first_bar_line: bool,
    pub scroll_speed: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BreakPeriod {
    pub start_time: f64,
    pub end_time: f64,
}

impl BreakPeriod {
    pub fn duration(&self) -> f64 {
        self.end_time - self.start_time
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HitSample {
    pub normal_set: i32,
    pub addition_set: i32,
    pub index: i32,
    pub volume: i32,
    pub filename: Option<String>,
    pub raw: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliderCurveType {
    Bezier,
    Catmull,
    Linear,
    Perfect,
    Other(char),
}

impl SliderCurveType {
    fn parse(value: char) -> Self {
        match value {
            'B' => Self::Bezier,
            'C' => Self::Catmull,
            'L' => Self::Linear,
            'P' => Self::Perfect,
            other => Self::Other(other),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SliderCurve {
    pub curve_type: SliderCurveType,
    /// Control points as `(x, y)` pairs; the object's position is the curve start.
    pub points: Vec<[f64; 2]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SliderEdgeSet {
    pub normal_set: i32,
    pub addition_set: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HitObject {
    /// Zero-based order in the source file, before objects are sorted by start time.
    pub source_order: usize,
    pub position: [f64; 2],
    pub start_time: f64,
    pub type_flags: u32,
    pub hitsound: i32,
    pub new_combo: bool,
    pub combo_skip: u8,
    pub column: Option<usize>,
    pub kind: HitObjectKind,
    pub hit_sample: Option<HitSample>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HitObjectKind {
    Circle,
    Slider {
        curve: SliderCurve,
        repeats: u32,
        pixel_length: f64,
        edge_sounds: Vec<i32>,
        edge_sets: Vec<SliderEdgeSet>,
    },
    Spinner {
        end_time: f64,
    },
    Hold {
        end_time: f64,
    },
}

impl HitObject {
    pub fn end_time(&self) -> f64 {
        match self.kind {
            HitObjectKind::Spinner { end_time } | HitObjectKind::Hold { end_time } => end_time,
            _ => self.start_time,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeatmapEvent {
    pub event_type: String,
    pub start_time: f64,
    pub parameters: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OsuBeatmap {
    pub version: u32,
    pub mode: GameMode,
    pub general: GeneralSettings,
    pub metadata: Metadata,
    pub difficulty: DifficultySettings,
    pub timing_points: Vec<TimingPoint>,
    pub control_points: ControlPoints,
    pub breaks: Vec<BreakPeriod>,
    pub events: Vec<BeatmapEvent>,
    pub hit_objects: Vec<HitObject>,
}

impl OsuBeatmap {
    /// Parse a UTF-8 `.osu` byte slice.
    pub fn parse_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        Self::parse_bufread(Cursor::new(bytes))
    }

    /// Parse from any readable stream. The stream is consumed but not closed by the parser.
    pub fn parse_reader<R: Read>(reader: R) -> Result<Self, ParseError> {
        Self::parse_bufread(BufReader::new(reader))
    }

    /// Parse from a buffered stream. The stream is consumed but not closed by the parser.
    pub fn parse_bufread<R: BufRead>(mut reader: R) -> Result<Self, ParseError> {
        let mut beatmap = BeatmapBuilder::default();
        let mut section = String::new();
        let mut line_no = 0;
        let mut header_seen = false;
        let mut line = String::new();

        loop {
            line.clear();
            let count = reader.read_line(&mut line).map_err(ParseError::io)?;
            if count == 0 {
                break;
            }
            line_no += 1;
            let mut text = line.trim();
            if line_no == 1 {
                text = text.trim_start_matches('\u{feff}').trim();
            }
            if text.is_empty() {
                continue;
            }

            if let Some(version_text) = text.strip_prefix("osu file format v") {
                if header_seen {
                    return Err(ParseError::at(line_no, "duplicate file format header"));
                }
                let version = parse_value::<u32>(version_text, line_no, "file format version")?;
                if !(MIN_SUPPORTED_VERSION..=MAX_SUPPORTED_VERSION).contains(&version) {
                    return Err(ParseError::at(
                        line_no,
                        format!(
                            "unsupported file format version {version}; supported range is {MIN_SUPPORTED_VERSION}..={MAX_SUPPORTED_VERSION}"
                        ),
                    ));
                }
                beatmap.version = Some(version);
                header_seen = true;
                continue;
            }

            if text.starts_with('[') && text.ends_with(']') {
                section = text[1..text.len() - 1].to_owned();
                continue;
            }

            if text.starts_with("//") && section != "Events" {
                continue;
            }

            match section.as_str() {
                "General" => parse_general(&mut beatmap, text, line_no)?,
                "Metadata" => parse_metadata(&mut beatmap, text, line_no)?,
                "Difficulty" => parse_difficulty(&mut beatmap, text, line_no)?,
                "TimingPoints" => beatmap
                    .timing_points
                    .push(parse_timing_point(text, line_no)?),
                "Events" => parse_event(&mut beatmap, text, line_no)?,
                "HitObjects" => {
                    beatmap.hit_object_lines.push((line_no, text.to_owned()));
                }
                _ => {} // Unknown/visual-only sections are outside this analysis model.
            }
        }

        let version = beatmap
            .version
            .ok_or_else(|| ParseError::global("missing `osu file format vN` header"))?;
        let mode = beatmap.general.mode;
        let key_count = if mode == GameMode::Mania {
            Some(beatmap.difficulty.mania_key_count().ok_or_else(|| {
                ParseError::global("mania beatmap requires an integer CircleSize in 1..=18")
            })?)
        } else {
            None
        };

        let mut hit_objects = Vec::with_capacity(beatmap.hit_object_lines.len());
        for (source_order, (line_no, object_line)) in
            beatmap.hit_object_lines.into_iter().enumerate()
        {
            let mut object = parse_hit_object(&object_line, line_no, mode, key_count)?;
            object.source_order = source_order;
            hit_objects.push(object);
        }
        hit_objects.sort_by(|a, b| a.start_time.total_cmp(&b.start_time));

        beatmap
            .timing_points
            .sort_by(|a, b| a.time.total_cmp(&b.time));
        beatmap
            .breaks
            .sort_by(|a, b| a.start_time.total_cmp(&b.start_time));
        let control_points = build_control_points(&beatmap.timing_points, mode);

        Ok(Self {
            version,
            mode,
            general: beatmap.general,
            metadata: beatmap.metadata,
            difficulty: beatmap.difficulty,
            timing_points: beatmap.timing_points,
            control_points,
            breaks: beatmap.breaks,
            events: beatmap.events,
            hit_objects,
        })
    }

    /// Parse a `.osu` file from disk.
    pub fn parse_file(path: impl AsRef<Path>) -> Result<Self, ParseError> {
        let file = File::open(path).map_err(ParseError::io)?;
        Self::parse_reader(file)
    }
}

#[derive(Default)]
struct BeatmapBuilder {
    version: Option<u32>,
    general: GeneralSettings,
    metadata: Metadata,
    difficulty: DifficultySettings,
    timing_points: Vec<TimingPoint>,
    breaks: Vec<BreakPeriod>,
    events: Vec<BeatmapEvent>,
    hit_object_lines: Vec<(usize, String)>,
}

fn parse_general(map: &mut BeatmapBuilder, line: &str, line_no: usize) -> Result<(), ParseError> {
    let Some((key, value)) = line.split_once(':') else {
        return Err(ParseError::at(
            line_no,
            "expected `key: value` in [General]",
        ));
    };
    let key = key.trim();
    let value = value.trim();
    match key {
        "AudioFilename" => map.general.audio_filename = Some(value.to_owned()),
        "AudioLeadIn" => map.general.audio_lead_in = Some(parse_value(value, line_no, key)?),
        "PreviewTime" => map.general.preview_time = Some(parse_value(value, line_no, key)?),
        "Countdown" => map.general.countdown = Some(parse_value(value, line_no, key)?),
        "SampleSet" => map.general.sample_set = Some(value.to_owned()),
        "StackLeniency" => map.general.stack_leniency = Some(parse_finite(value, line_no, key)?),
        "Mode" => {
            let mode = parse_value::<i32>(value, line_no, key)?;
            map.general.mode =
                GameMode::try_from(mode).map_err(|error| ParseError::at(line_no, error))?;
        }
        _ => {}
    }
    Ok(())
}

fn parse_metadata(map: &mut BeatmapBuilder, line: &str, line_no: usize) -> Result<(), ParseError> {
    let Some((key, value)) = line.split_once(':') else {
        return Err(ParseError::at(
            line_no,
            "expected `key: value` in [Metadata]",
        ));
    };
    let key = key.trim();
    let value = value.trim();
    match key {
        "Title" => map.metadata.title = Some(value.to_owned()),
        "TitleUnicode" => map.metadata.title_unicode = Some(value.to_owned()),
        "Artist" => map.metadata.artist = Some(value.to_owned()),
        "ArtistUnicode" => map.metadata.artist_unicode = Some(value.to_owned()),
        "Creator" => map.metadata.creator = Some(value.to_owned()),
        "Version" => map.metadata.version = Some(value.to_owned()),
        "BeatmapID" => map.metadata.beatmap_id = Some(parse_value(value, line_no, key)?),
        "BeatmapSetID" => map.metadata.beatmap_set_id = Some(parse_value(value, line_no, key)?),
        "Source" => map.metadata.source = Some(value.to_owned()),
        "Tags" => map.metadata.tags = Some(value.to_owned()),
        _ => {}
    }
    Ok(())
}

fn parse_difficulty(
    map: &mut BeatmapBuilder,
    line: &str,
    line_no: usize,
) -> Result<(), ParseError> {
    let Some((key, value)) = line.split_once(':') else {
        return Err(ParseError::at(
            line_no,
            "expected `key: value` in [Difficulty]",
        ));
    };
    let key = key.trim();
    let value = value.trim();
    let parsed = || parse_finite(value, line_no, key);
    match key {
        "HPDrainRate" => map.difficulty.hp_drain_rate = Some(parsed()?),
        "CircleSize" => map.difficulty.circle_size = Some(parsed()?),
        "OverallDifficulty" => map.difficulty.overall_difficulty = Some(parsed()?),
        "ApproachRate" => map.difficulty.approach_rate = Some(parsed()?),
        "SliderMultiplier" => map.difficulty.slider_multiplier = Some(parsed()?),
        "SliderTickRate" => map.difficulty.slider_tick_rate = Some(parsed()?),
        _ => {}
    }
    Ok(())
}

fn parse_timing_point(line: &str, line_no: usize) -> Result<TimingPoint, ParseError> {
    let fields: Vec<&str> = line.split(',').collect();
    if fields.len() < 2 {
        return Err(ParseError::at(
            line_no,
            "timing point requires time and beat length",
        ));
    }
    if fields.len() > 8 {
        return Err(ParseError::at(
            line_no,
            "timing point has more than 8 fields",
        ));
    }
    let time = parse_finite(fields[0], line_no, "timing point time")?;
    let beat_length = parse_value::<f64>(fields[1], line_no, "beat length")?;
    let uninherited = fields
        .get(6)
        .filter(|value| !value.trim().is_empty())
        .map_or(true, |value| value.trim().starts_with('1'));
    if (!beat_length.is_finite() && !(beat_length.is_nan() && !uninherited))
        || (beat_length.is_nan() && uninherited)
    {
        return Err(ParseError::at(
            line_no,
            "beat length must be finite, except inherited NaN may disable slider ticks",
        ));
    }

    let meter = optional_field(fields.get(2), 4, line_no, "meter")?.max(1);
    let sample_set = optional_field(fields.get(3), 0, line_no, "sample set")?;
    let sample_index = optional_field(fields.get(4), 0, line_no, "sample index")?.max(0);
    let volume = optional_field(fields.get(5), 100, line_no, "volume")?.clamp(0, 100);
    let effects = optional_field(fields.get(7), 0, line_no, "effects")?;
    Ok(TimingPoint {
        time,
        beat_length,
        meter,
        sample_set,
        sample_index,
        volume,
        uninherited,
        effects,
    })
}

fn optional_field(
    fields: Option<&&str>,
    default: i32,
    line_no: usize,
    name: &str,
) -> Result<i32, ParseError> {
    match fields {
        Some(value) if !value.trim().is_empty() => parse_value(value, line_no, name),
        _ => Ok(default),
    }
}

fn parse_event(map: &mut BeatmapBuilder, line: &str, line_no: usize) -> Result<(), ParseError> {
    if line.starts_with("//") {
        return Ok(());
    }
    let fields: Vec<&str> = line.split(',').collect();
    let event_type = fields.first().map(|value| value.trim()).unwrap_or_default();
    // Storyboard commands share [Events] but are not beatmap-level events. They are intentionally
    // outside the analysis model; retain standard event records and ignore those command rows.
    if event_type.parse::<i32>().is_err()
        && !matches!(
            event_type.to_ascii_lowercase().as_str(),
            "background" | "video" | "break" | "sample" | "colour" | "color"
        )
    {
        return Ok(());
    }
    if fields.len() < 2 {
        return Err(ParseError::at(
            line_no,
            "event requires a type and start time",
        ));
    }
    let event_type = event_type.to_owned();
    let start_time = parse_finite(fields[1], line_no, "event start time")?;
    let parameters: Vec<String> = fields
        .iter()
        .skip(2)
        .map(|part| part.trim().trim_matches('"').to_owned())
        .collect();
    if event_type == "2" {
        if fields.len() < 3 {
            return Err(ParseError::at(line_no, "break event requires an end time"));
        }
        let end_time = parse_finite(fields[2], line_no, "break end time")?;
        if end_time < start_time {
            return Err(ParseError::at(
                line_no,
                "break end time precedes its start time",
            ));
        }
        map.breaks.push(BreakPeriod {
            start_time,
            end_time,
        });
    }
    map.events.push(BeatmapEvent {
        event_type,
        start_time,
        parameters,
    });
    Ok(())
}

fn parse_hit_object(
    line: &str,
    line_no: usize,
    mode: GameMode,
    key_count: Option<usize>,
) -> Result<HitObject, ParseError> {
    let fields: Vec<&str> = line.split(',').collect();
    if fields.len() < 5 {
        return Err(ParseError::at(
            line_no,
            "hit object requires at least x, y, time, type, and hitsound",
        ));
    }
    let x = parse_finite(fields[0], line_no, "object x")?;
    let y = parse_finite(fields[1], line_no, "object y")?;
    let start_time = parse_finite(fields[2], line_no, "object start time")?;
    let type_flags = parse_value::<u32>(fields[3], line_no, "object type")?;
    if type_flags == 0 {
        return Err(ParseError::at(line_no, "object type flags cannot be zero"));
    }
    let hitsound = parse_value::<i32>(fields[4], line_no, "object hitsound")?;
    let major_flags = type_flags & (1 | 2 | 8 | 128);
    if major_flags.count_ones() > 1 {
        return Err(ParseError::at(
            line_no,
            "hit object has conflicting primary type flags",
        ));
    }
    let column = if mode == GameMode::Mania {
        let count = key_count.expect("mania key count was validated");
        Some(((x * count as f64 / 512.0).floor() as isize).clamp(0, count as isize - 1) as usize)
    } else {
        None
    };

    let (kind, sample_field) = if type_flags & 128 != 0 {
        if mode != GameMode::Mania {
            return Err(ParseError::at(
                line_no,
                "hold objects are only valid in mania mode",
            ));
        }
        if fields.len() < 6 {
            return Err(ParseError::at(
                line_no,
                "mania hold requires `endTime:hitSample`",
            ));
        }
        let (end, sample) = split_hold_tail(fields[5], line_no)?;
        if end < start_time {
            return Err(ParseError::at(
                line_no,
                "hold end time precedes its start time",
            ));
        }
        (HitObjectKind::Hold { end_time: end }, sample)
    } else if type_flags & 8 != 0 {
        if fields.len() < 6 {
            return Err(ParseError::at(line_no, "spinner requires an end time"));
        }
        let end = parse_finite(fields[5], line_no, "spinner end time")?;
        if end < start_time {
            return Err(ParseError::at(
                line_no,
                "spinner end time precedes its start time",
            ));
        }
        (
            HitObjectKind::Spinner { end_time: end },
            fields.get(6).copied(),
        )
    } else if type_flags & 2 != 0 {
        if fields.len() < 8 {
            return Err(ParseError::at(
                line_no,
                "slider requires curve, repeats, and pixel length",
            ));
        }
        let curve = parse_curve(fields[5], line_no)?;
        let repeats = parse_value::<u32>(fields[6], line_no, "slider repeats")?;
        if repeats == 0 {
            return Err(ParseError::at(line_no, "slider repeats must be at least 1"));
        }
        let pixel_length = parse_finite(fields[7], line_no, "slider pixel length")?;
        if pixel_length < 0.0 {
            return Err(ParseError::at(
                line_no,
                "slider pixel length cannot be negative",
            ));
        }
        let edge_sounds = match fields.get(8) {
            Some(value) if !value.is_empty() => {
                parse_int_list(value, line_no, "slider edge sounds")?
            }
            _ => Vec::new(),
        };
        let edge_sets = match fields.get(9) {
            Some(value) if !value.is_empty() => parse_edge_sets(value, line_no)?,
            _ => Vec::new(),
        };
        let sample = fields.get(10).copied();
        (
            HitObjectKind::Slider {
                curve,
                repeats,
                pixel_length,
                edge_sounds,
                edge_sets,
            },
            sample,
        )
    } else if type_flags & 1 != 0 {
        (HitObjectKind::Circle, fields.get(5).copied())
    } else {
        return Err(ParseError::at(
            line_no,
            format!("unsupported hit object type flags {type_flags}"),
        ));
    };

    let hit_sample = sample_field
        .filter(|value| !value.is_empty())
        .map(|raw| parse_hit_sample(raw, line_no))
        .transpose()?;
    Ok(HitObject {
        source_order: 0,
        position: [x, y],
        start_time,
        type_flags,
        hitsound,
        new_combo: type_flags & 4 != 0,
        combo_skip: ((type_flags & 0x70) >> 4) as u8,
        column,
        kind,
        hit_sample,
    })
}

fn split_hold_tail(value: &str, line_no: usize) -> Result<(f64, Option<&str>), ParseError> {
    let (time, sample) = value
        .split_once(':')
        .map_or((value, None), |(time, sample)| (time, Some(sample)));
    let time = parse_finite(time, line_no, "hold end time")?;
    Ok((time, sample))
}

fn parse_curve(value: &str, line_no: usize) -> Result<SliderCurve, ParseError> {
    let mut parts = value.split('|');
    let curve_type = parts
        .next()
        .and_then(|part| part.chars().next())
        .ok_or_else(|| ParseError::at(line_no, "slider curve type is empty"))?;
    let points = parts
        .map(|part| {
            let (x, y) = part
                .split_once(':')
                .ok_or_else(|| ParseError::at(line_no, "slider curve point must be x:y"))?;
            Ok([
                parse_finite(x, line_no, "curve x")?,
                parse_finite(y, line_no, "curve y")?,
            ])
        })
        .collect::<Result<Vec<_>, ParseError>>()?;
    if points.is_empty() {
        return Err(ParseError::at(
            line_no,
            "slider curve requires at least one control point",
        ));
    }
    Ok(SliderCurve {
        curve_type: SliderCurveType::parse(curve_type),
        points,
    })
}

fn parse_int_list(value: &str, line_no: usize, name: &str) -> Result<Vec<i32>, ParseError> {
    value
        .split('|')
        .map(|part| parse_value(part, line_no, name))
        .collect()
}

fn parse_edge_sets(value: &str, line_no: usize) -> Result<Vec<SliderEdgeSet>, ParseError> {
    value
        .split('|')
        .map(|part| {
            let (normal, addition) = part.split_once(':').ok_or_else(|| {
                ParseError::at(line_no, "slider edge set must be normal:addition")
            })?;
            Ok(SliderEdgeSet {
                normal_set: parse_value(normal, line_no, "edge normal sample set")?,
                addition_set: parse_value(addition, line_no, "edge addition sample set")?,
            })
        })
        .collect()
}

fn parse_hit_sample(value: &str, line_no: usize) -> Result<HitSample, ParseError> {
    let fields: Vec<&str> = value.split(':').collect();
    if fields.len() < 4 || fields.len() > 5 {
        return Err(ParseError::at(
            line_no,
            "hit sample requires 4 or 5 colon-separated fields",
        ));
    }
    Ok(HitSample {
        normal_set: parse_value(fields[0], line_no, "hit sample normal set")?,
        addition_set: parse_value(fields[1], line_no, "hit sample addition set")?,
        index: parse_value(fields[2], line_no, "hit sample index")?,
        volume: parse_value(fields[3], line_no, "hit sample volume")?,
        filename: fields
            .get(4)
            .filter(|value| !value.is_empty())
            .map(|value| (*value).to_owned()),
        raw: value.to_owned(),
    })
}

fn build_control_points(timing_points: &[TimingPoint], mode: GameMode) -> ControlPoints {
    let mut result = ControlPoints::default();
    let mut index = 0;

    while index < timing_points.len() {
        let time = timing_points[index].time;
        let start = index;
        while index + 1 < timing_points.len() && timing_points[index + 1].time == time {
            index += 1;
        }
        let same_time = &timing_points[start..=index];
        let timing_point = same_time.iter().find(|point| point.uninherited);
        let inherited_point = same_time.iter().rev().find(|point| !point.uninherited);
        let property_point = inherited_point
            .or(timing_point)
            .expect("group has a timing point");

        if let Some(point) = timing_point {
            result.timing.push(TimingControlPoint {
                time: point.time,
                beat_length: point.beat_length.clamp(0.6, 600_000.0),
                meter: point.meter,
            });
        }

        let speed_multiplier = if property_point.beat_length < 0.0 {
            -100.0 / property_point.beat_length
        } else {
            1.0
        };
        result.difficulty.push(DifficultyControlPoint {
            time,
            speed_multiplier,
            generate_ticks: !property_point.beat_length.is_nan(),
        });
        result.sample.push(SampleControlPoint {
            time,
            sample_set: property_point.sample_set,
            sample_index: property_point.sample_index,
            volume: property_point.volume,
        });
        result.effect.push(EffectControlPoint {
            time,
            kiai: property_point.effects & 1 != 0,
            omit_first_bar_line: property_point.effects & 8 != 0,
            scroll_speed: matches!(mode, GameMode::Taiko | GameMode::Mania)
                .then_some(speed_multiplier),
        });

        index += 1;
    }
    result
}

fn parse_value<T: std::str::FromStr>(
    value: &str,
    line_no: usize,
    name: &str,
) -> Result<T, ParseError> {
    value
        .trim()
        .parse()
        .map_err(|_| ParseError::at(line_no, format!("invalid {name}: `{}`", value.trim())))
}

fn parse_finite(value: &str, line_no: usize, name: &str) -> Result<f64, ParseError> {
    let parsed = parse_value::<f64>(value, line_no, name)?;
    if !parsed.is_finite() {
        return Err(ParseError::at(line_no, format!("{name} must be finite")));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const PREFIX: &str =
        "osu file format v14\n[General]\nMode:3\n[Difficulty]\nCircleSize:4\nOverallDifficulty:8\n";

    #[test]
    fn parses_bytes_and_buffered_stream_with_objects_timing_and_breaks() {
        let text = format!(
            "{PREFIX}[TimingPoints]\n0,500,4,2,0,80,1,1\n500,-50,4,2,0,60,0,9\n[Events]\n2,1000,1500\nSprite,Background,Centre,\"bg.jpg\",0,0\n[HitObjects]\n64,192,100,1,0,0:0:0:0:\n192,192,200,128,0,600:0:0:0:0:\n"
        );
        let bytes_map = OsuBeatmap::parse_bytes(text.as_bytes()).unwrap();
        let stream_map = OsuBeatmap::parse_bufread(Cursor::new(text.as_bytes())).unwrap();
        assert_eq!(bytes_map, stream_map);
        assert_eq!(bytes_map.mode, GameMode::Mania);
        assert_eq!(bytes_map.hit_objects[0].column, Some(0));
        assert!(matches!(
            bytes_map.hit_objects[1].kind,
            HitObjectKind::Hold { end_time: 600.0 }
        ));
        assert_eq!(bytes_map.control_points.timing.len(), 1);
        assert_eq!(bytes_map.control_points.difficulty[1].speed_multiplier, 2.0);
        assert_eq!(bytes_map.breaks[0].duration(), 500.0);
    }

    #[test]
    fn parses_existing_mania_beatmap_fixture() {
        let bytes = include_bytes!(
            "../../tests/Resource/Jumpstream - Happy Hardcore Synthesizer (SK_la) [5k-1].osu"
        );
        let beatmap = OsuBeatmap::parse_bytes(bytes).unwrap();
        assert_eq!(beatmap.mode, GameMode::Mania);
        assert_eq!(beatmap.difficulty.mania_key_count(), Some(5));
        assert!(!beatmap.hit_objects.is_empty());
        assert!(!beatmap.timing_points.is_empty());
    }

    #[test]
    fn parses_each_rulesets_official_difficulty_fixture() {
        let fixtures = [
            (
                include_bytes!("../tests/fixtures/osu-diffcalc-test.osu").as_slice(),
                GameMode::Standard,
            ),
            (
                include_bytes!("../tests/fixtures/taiko-diffcalc-test.osu").as_slice(),
                GameMode::Taiko,
            ),
            (
                include_bytes!("../tests/fixtures/catch-diffcalc-test.osu").as_slice(),
                GameMode::Catch,
            ),
        ];

        for (bytes, expected_mode) in fixtures {
            let beatmap = OsuBeatmap::parse_bytes(bytes).unwrap();
            assert_eq!(beatmap.mode, expected_mode);
            assert!(!beatmap.hit_objects.is_empty());
            assert!(!beatmap.timing_points.is_empty());
        }
    }

    #[test]
    fn mania_columns_round_circle_size_to_nearest_even_and_clamp() {
        assert_eq!(
            DifficultySettings {
                circle_size: Some(2.5),
                ..Default::default()
            }
            .mania_key_count(),
            Some(2)
        );
        assert_eq!(
            DifficultySettings {
                circle_size: Some(3.5),
                ..Default::default()
            }
            .mania_key_count(),
            Some(4)
        );
        assert_eq!(
            DifficultySettings {
                circle_size: Some(0.2),
                ..Default::default()
            }
            .mania_key_count(),
            Some(1)
        );
        assert_eq!(
            DifficultySettings {
                circle_size: Some(20.0),
                ..Default::default()
            }
            .mania_key_count(),
            Some(18)
        );
    }

    #[test]
    fn parses_all_four_modes_and_primary_object_kinds() {
        for mode in 0..=3 {
            let key_setting = if mode == 3 {
                "\n[Difficulty]\nCircleSize:4"
            } else {
                ""
            };
            let text = format!(
                "osu file format v128\n[General]\nMode:{mode}{key_setting}\n[HitObjects]\n64,192,100,1,0,0:0:0:0:\n64,192,200,2,0,B|128:192,2,100,0|0|0,0:0|0:0,0:0:0:0:\n64,192,300,8,0,400,0:0:0:0:\n{}",
                if mode == 3 {
                    "64,192,500,128,0,700:0:0:0:0:"
                } else {
                    ""
                }
            );
            let beatmap = OsuBeatmap::parse_bytes(text.as_bytes()).unwrap();
            assert_eq!(beatmap.mode as i32, mode);
            assert!(matches!(beatmap.hit_objects[0].kind, HitObjectKind::Circle));
            assert!(matches!(
                beatmap.hit_objects[1].kind,
                HitObjectKind::Slider {
                    repeats: 2,
                    pixel_length: 100.0,
                    ..
                }
            ));
            assert!(matches!(
                beatmap.hit_objects[2].kind,
                HitObjectKind::Spinner { end_time: 400.0 }
            ));
            if mode == 3 {
                assert!(matches!(
                    beatmap.hit_objects[3].kind,
                    HitObjectKind::Hold { end_time: 700.0 }
                ));
            }
        }
    }

    #[test]
    fn parses_file_and_plain_read_stream() {
        let text = "osu file format v3\n[General]\nMode:0\n[HitObjects]\n1,2,3,1,0\n";
        #[cfg(target_os = "wasi")]
        let temp_path = std::path::PathBuf::from("/tmp/osu-parser-file-test.osu");
        #[cfg(not(target_os = "wasi"))]
        let temp_path = std::env::temp_dir().join(format!("osu-parser-{}.osu", std::process::id()));
        std::fs::write(&temp_path, text).unwrap();
        let from_file = OsuBeatmap::parse_file(&temp_path).unwrap();
        let from_reader = OsuBeatmap::parse_reader(Cursor::new(text.as_bytes())).unwrap();
        let _ = std::fs::remove_file(temp_path);
        assert_eq!(from_file, from_reader);
    }

    #[test]
    fn same_time_timing_points_merge_with_green_point_priority() {
        let beatmap = OsuBeatmap::parse_bytes(
            b"osu file format v14\n[General]\nMode:3\n[Difficulty]\nCircleSize:4\n[TimingPoints]\n0,500,4,1,0,100,1,0\n0,-50,4,2,0,30,0,1\n0,-25,4,3,0,40,0,0\n0,-20,4,4,0,50,2,0\n",
        )
        .unwrap();
        assert_eq!(beatmap.timing_points.len(), 4);
        assert_eq!(beatmap.control_points.timing.len(), 1);
        assert_eq!(beatmap.control_points.timing[0].beat_length, 500.0);
        assert_eq!(beatmap.control_points.difficulty.len(), 1);
        assert_eq!(beatmap.control_points.difficulty[0].speed_multiplier, 5.0);
        assert_eq!(beatmap.control_points.sample[0].sample_set, 4);
        assert_eq!(beatmap.control_points.sample[0].volume, 50);
        assert_eq!(beatmap.control_points.effect[0].scroll_speed, Some(5.0));

        let no_ticks = OsuBeatmap::parse_bytes(
            b"osu file format v14\n[General]\nMode:0\n[TimingPoints]\n0,500,4,0,0,100,1,0\n10,NaN,4,0,0,100,0,0\n",
        )
        .unwrap();
        assert_eq!(no_ticks.control_points.difficulty.len(), 2);
        assert!(!no_ticks.control_points.difficulty[1].generate_ticks);
        assert_eq!(no_ticks.control_points.difficulty[1].speed_multiplier, 1.0);
    }

    #[test]
    fn rejects_version_boundaries_and_malformed_data() {
        for version in [MIN_SUPPORTED_VERSION, 4, 128, 129, MAX_SUPPORTED_VERSION] {
            let text = format!("osu file format v{version}\n[General]\nMode:0\n");
            assert_eq!(
                OsuBeatmap::parse_bytes(text.as_bytes()).unwrap().version,
                version
            );
        }
        for version in [MIN_SUPPORTED_VERSION - 1, MAX_SUPPORTED_VERSION + 1] {
            let text = format!("osu file format v{version}\n");
            assert!(OsuBeatmap::parse_bytes(text.as_bytes()).is_err());
        }
        assert!(OsuBeatmap::parse_bytes(b"osu file format v14\n[General]\nMode:bad\n").is_err());
        assert!(OsuBeatmap::parse_bytes(b"osu file format v14\n[HitObjects]\n1,2,3,1\n").is_err());
        assert!(
            OsuBeatmap::parse_bytes(
                b"osu file format v14\n[General]\nMode:0\n[TimingPoints]\n0,nope\n"
            )
            .is_err()
        );
    }

    #[test]
    fn preserves_identical_hit_object_rows() {
        let text = "osu file format v14\n[General]\nMode:0\n[HitObjects]\n1,2,3,1,0\n1,2,3,1,0\n";
        let beatmap = OsuBeatmap::parse_bytes(text.as_bytes()).unwrap();
        assert_eq!(beatmap.hit_objects.len(), 2);
    }

    #[test]
    fn preserves_slider_curve_and_sample_details() {
        let text = "osu file format v14\n[General]\nMode:0\n[HitObjects]\n10,20,30,2,4,B|40:50|60:70,3,123.5,0|2|8,0:0|1:2|3:4,1:2:3:45:custom.wav\n";
        let beatmap = OsuBeatmap::parse_bytes(text.as_bytes()).unwrap();
        let HitObjectKind::Slider {
            curve,
            repeats,
            pixel_length,
            edge_sounds,
            edge_sets,
        } = &beatmap.hit_objects[0].kind
        else {
            panic!("slider expected")
        };
        assert_eq!(*repeats, 3);
        assert_eq!(*pixel_length, 123.5);
        assert_eq!(curve.curve_type, SliderCurveType::Bezier);
        assert_eq!(curve.points, vec![[40.0, 50.0], [60.0, 70.0]]);
        assert_eq!(edge_sounds, &[0, 2, 8]);
        assert_eq!(edge_sets.len(), 3);
        assert_eq!(
            beatmap.hit_objects[0]
                .hit_sample
                .as_ref()
                .unwrap()
                .filename
                .as_deref(),
            Some("custom.wav")
        );
    }
}
