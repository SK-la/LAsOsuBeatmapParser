//! Pure chart-side skillset bucket classification for Dan.
//!
//! This module consumes CSI and LeoBlack products as typed input. It does not calculate those
//! features or aggregate player histories.

use crate::dan_lookup::DanSide;
use crate::skills::{EzMinaSkillAxis, SkillAxis, SkillVector};
/// One chart's already-computed CSI/LeoBlack data needed by Dan bucket filing.
///
/// `None` fields mean that the producer did not have that measurement. In particular, the
/// classifier does not synthesize LeoBlack clusters, motion features, or pattern tags.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DanChartFeatures<'a> {
    pub skill_tags: &'a [DanChartSkillTag],
    pub key_count: Option<usize>,
    pub length_seconds: Option<f64>,
    pub ln_ratio: Option<f64>,
    pub dan_eligible: bool,
    pub vibro: bool,
    pub jack_demand: bool,
    pub jack_share: Option<f64>,
    pub stream_share: Option<f64>,
    pub tech_category: Option<bool>,
    pub cluster_trill: Option<bool>,
    pub handstream_cluster: Option<bool>,
    pub tech_score: f64,
    pub chordjack_score: f64,
    pub jack_score: Option<f64>,
    pub motion: Option<DanMotionFeatures>,
}

impl Default for DanChartFeatures<'_> {
    fn default() -> Self {
        Self {
            skill_tags: &[],
            key_count: None,
            length_seconds: None,
            ln_ratio: None,
            dan_eligible: true,
            vibro: false,
            jack_demand: false,
            jack_share: None,
            stream_share: None,
            tech_category: None,
            cluster_trill: None,
            handstream_cluster: None,
            tech_score: 0.0,
            chordjack_score: 0.0,
            jack_score: None,
            motion: None,
        }
    }
}

/// Pattern tags consumed by the C# Dan filing tables, as stable wire IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DanChartSkillTag {
    Chordjack,
    Speedjack,
    Jack,
    Tech,
    Delay,
    Chordstream,
    Bracket,
    LnGeneral,
    Ln,
    LnTech,
    LnInverse,
    LnReleasePattern,
}

impl DanChartSkillTag {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Chordjack => "chordjack",
            Self::Speedjack => "speedjack",
            Self::Jack => "jack",
            Self::Tech => "tech",
            Self::Delay => "delay",
            Self::Chordstream => "chordstream",
            Self::Bracket => "bracket",
            Self::LnGeneral => "ln_general",
            Self::Ln => "ln",
            Self::LnTech => "ln_tech",
            Self::LnInverse => "ln_inverse",
            Self::LnReleasePattern => "ln_release_pattern",
        }
    }
}

/// Motion features used by the C# speed/tech logistic filing model.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DanMotionFeatures {
    pub rhythm_break: f64,
    pub cross_hand_trill: f64,
    pub mini_jack: f64,
    pub same_hand: f64,
}

/// Stable bucket IDs returned by chart-side filing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DanBucketId {
    Jack,
    Tech,
    Speed,
    Stamina,
    Stream,
    LnGeneral,
    LnTech,
    LnInverse,
    LnReleasePattern,
}

impl DanBucketId {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Jack => "jack",
            Self::Tech => "tech",
            Self::Speed => "speed",
            Self::Stamina => "stamina",
            Self::Stream => "stream",
            Self::LnGeneral => "ln_general",
            Self::LnTech => "ln_tech",
            Self::LnInverse => "ln_inverse",
            Self::LnReleasePattern => "ln_release_pattern",
        }
    }
}

/// Small ordered bucket vector. Its IDs remain typed and its storage does not allocate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DanBucketVector {
    values: [Option<DanBucketId>; 4],
    len: u8,
}

impl DanBucketVector {
    pub const fn new() -> Self {
        Self {
            values: [None; 4],
            len: 0,
        }
    }

    pub fn push(&mut self, bucket: DanBucketId) {
        if self.iter().any(|existing| existing == bucket) || self.len as usize == self.values.len()
        {
            return;
        }
        self.values[self.len as usize] = Some(bucket);
        self.len += 1;
    }

    pub fn len(self) -> usize {
        self.len as usize
    }

    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    pub fn get(self, index: usize) -> Option<DanBucketId> {
        self.values.get(index).copied().flatten()
    }

    pub fn iter(self) -> impl Iterator<Item = DanBucketId> {
        self.values.into_iter().take(self.len as usize).flatten()
    }
}

/// Typed input for filing one chart or one play into Dan skillset buckets.
#[derive(Clone, Copy, Debug)]
pub struct DanFilingInput<'a> {
    pub key_count: usize,
    pub side: DanSide,
    pub mina_values: &'a SkillVector<EzMinaSkillAxis>,
    pub chart_features: Option<&'a DanChartFeatures<'a>>,
    pub length_seconds: Option<f64>,
    pub rate: f64,
}

/// Files one chart/play into zero or more skillset buckets using the EzLazer filing rules.
pub fn file_chart_skillsets(input: DanFilingInput<'_>) -> DanBucketVector {
    classify_skillset_buckets(
        input.key_count,
        input.side,
        input.mina_values,
        input.chart_features,
        input.length_seconds,
        input.rate,
    )
}
/// Pure per-chart bucket assignment. A player-history aggregator can call this for each clear
/// using that play's Mina vector and the chart's cached CSI features.
pub fn classify_skillset_buckets(
    key_count: usize,
    side: DanSide,
    values: &SkillVector<EzMinaSkillAxis>,
    chart: Option<&DanChartFeatures<'_>>,
    length_seconds: Option<f64>,
    rate: f64,
) -> DanBucketVector {
    let buckets = bucket_definitions(key_count, side);
    if buckets.is_empty() {
        return DanBucketVector::new();
    }

    if chart.is_some_and(|info| !info.dan_eligible || info.vibro) {
        return DanBucketVector::new();
    }

    let top = bucketing_skillset(values, length_seconds, rate, chart);
    buckets_for_clear(&buckets, top, chart, values, rate)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MinaSkillset {
    Stream,
    Jumpstream,
    Handstream,
    Stamina,
    JackSpeed,
    Chordjack,
    Technical,
}

impl MinaSkillset {
    fn from_axis(axis: EzMinaSkillAxis) -> Option<Self> {
        match axis {
            EzMinaSkillAxis::Stream => Some(Self::Stream),
            EzMinaSkillAxis::Jumpstream => Some(Self::Jumpstream),
            EzMinaSkillAxis::Handstream => Some(Self::Handstream),
            EzMinaSkillAxis::Stamina => Some(Self::Stamina),
            EzMinaSkillAxis::JackSpeed => Some(Self::JackSpeed),
            EzMinaSkillAxis::Chordjack => Some(Self::Chordjack),
            EzMinaSkillAxis::Technical => Some(Self::Technical),
            EzMinaSkillAxis::Overall => None,
        }
    }
}

#[derive(Clone, Copy)]
struct BucketDef {
    id: DanBucketId,
    tags: &'static [DanChartSkillTag],
    skillsets: &'static [MinaSkillset],
    cluster_family: Option<DanBucketId>,
}

const NO_SKILLSETS: &[MinaSkillset] = &[];
const NO_TAGS: &[DanChartSkillTag] = &[];
const JACK_4K_TAGS: &[DanChartSkillTag] =
    &[DanChartSkillTag::Chordjack, DanChartSkillTag::Speedjack];
const JACK_4K_SKILLSETS: &[MinaSkillset] = &[MinaSkillset::JackSpeed, MinaSkillset::Chordjack];
const TECH_4K_SKILLSETS: &[MinaSkillset] = &[MinaSkillset::Technical, MinaSkillset::Jumpstream];
const SPEED_4K_SKILLSETS: &[MinaSkillset] = &[MinaSkillset::Stream];
const STAMINA_4K_SKILLSETS: &[MinaSkillset] = &[MinaSkillset::Handstream, MinaSkillset::Stamina];
const LN_GENERAL_TAGS: &[DanChartSkillTag] = &[DanChartSkillTag::LnGeneral, DanChartSkillTag::Ln];
const LN_TECH_TAGS: &[DanChartSkillTag] = &[DanChartSkillTag::LnTech];
const LN_INVERSE_TAGS: &[DanChartSkillTag] = &[DanChartSkillTag::LnInverse];
const LN_RELEASE_TAGS: &[DanChartSkillTag] = &[DanChartSkillTag::LnReleasePattern];
const JACK_TAGS: &[DanChartSkillTag] = &[DanChartSkillTag::Jack];
const TECH_TAGS: &[DanChartSkillTag] = &[DanChartSkillTag::Tech];
const DELAY_TAGS: &[DanChartSkillTag] = &[DanChartSkillTag::Delay];
const STREAM_TAGS: &[DanChartSkillTag] =
    &[DanChartSkillTag::Chordstream, DanChartSkillTag::Bracket];

const RC_4K: [BucketDef; 4] = [
    BucketDef {
        id: DanBucketId::Jack,
        tags: JACK_4K_TAGS,
        skillsets: JACK_4K_SKILLSETS,
        cluster_family: None,
    },
    BucketDef {
        id: DanBucketId::Tech,
        tags: NO_TAGS,
        skillsets: TECH_4K_SKILLSETS,
        cluster_family: None,
    },
    BucketDef {
        id: DanBucketId::Speed,
        tags: NO_TAGS,
        skillsets: SPEED_4K_SKILLSETS,
        cluster_family: None,
    },
    BucketDef {
        id: DanBucketId::Stamina,
        tags: NO_TAGS,
        skillsets: STAMINA_4K_SKILLSETS,
        cluster_family: None,
    },
];
const RC_PATTERN: [BucketDef; 4] = [
    BucketDef {
        id: DanBucketId::Jack,
        tags: JACK_TAGS,
        skillsets: NO_SKILLSETS,
        cluster_family: Some(DanBucketId::Jack),
    },
    BucketDef {
        id: DanBucketId::Tech,
        tags: TECH_TAGS,
        skillsets: NO_SKILLSETS,
        cluster_family: Some(DanBucketId::Tech),
    },
    BucketDef {
        id: DanBucketId::Speed,
        tags: DELAY_TAGS,
        skillsets: NO_SKILLSETS,
        cluster_family: None,
    },
    BucketDef {
        id: DanBucketId::Stream,
        tags: STREAM_TAGS,
        skillsets: NO_SKILLSETS,
        cluster_family: Some(DanBucketId::Stream),
    },
];
const LN_7K: [BucketDef; 4] = [
    BucketDef {
        id: DanBucketId::LnGeneral,
        tags: LN_GENERAL_TAGS,
        skillsets: NO_SKILLSETS,
        cluster_family: None,
    },
    BucketDef {
        id: DanBucketId::LnTech,
        tags: LN_TECH_TAGS,
        skillsets: NO_SKILLSETS,
        cluster_family: None,
    },
    BucketDef {
        id: DanBucketId::LnInverse,
        tags: LN_INVERSE_TAGS,
        skillsets: NO_SKILLSETS,
        cluster_family: None,
    },
    BucketDef {
        id: DanBucketId::LnReleasePattern,
        tags: LN_RELEASE_TAGS,
        skillsets: NO_SKILLSETS,
        cluster_family: None,
    },
];

fn bucket_definitions(key_count: usize, side: DanSide) -> &'static [BucketDef] {
    match (key_count, side) {
        (4, DanSide::Rc) => &RC_4K,
        (7, DanSide::Ln) => &LN_7K,
        (_, DanSide::Ln) => &[],
        (_, DanSide::Rc) => &RC_PATTERN,
    }
}

fn classify_bucket_tag_match(bucket: &BucketDef, chart: Option<&DanChartFeatures<'_>>) -> bool {
    let Some(chart) = chart else {
        return false;
    };
    match bucket.cluster_family {
        Some(DanBucketId::Tech) => {
            if let Some(category) = chart.tech_category {
                return category;
            }
        }
        Some(DanBucketId::Jack) => {
            if let Some(share) = chart.jack_share {
                return share >= CLUSTER_SHARE_MIN;
            }
        }
        Some(DanBucketId::Stream) => {
            if let Some(share) = chart.stream_share {
                return share >= CLUSTER_SHARE_MIN;
            }
        }
        _ => {}
    }
    bucket.tags.iter().any(|tag| chart.skill_tags.contains(tag))
}

fn dominant_skillset(values: &SkillVector<EzMinaSkillAxis>) -> Option<MinaSkillset> {
    let mut best = None;
    let mut best_value = 0.0;
    for axis in EzMinaSkillAxis::ALL.iter().copied() {
        let Some(skillset) = MinaSkillset::from_axis(axis) else {
            continue;
        };
        let value = finite_value(values, axis);
        if value > best_value {
            best = Some(skillset);
            best_value = value;
        }
    }
    best
}

fn bucketing_skillset(
    values: &SkillVector<EzMinaSkillAxis>,
    length_seconds: Option<f64>,
    rate: f64,
    chart: Option<&DanChartFeatures<'_>>,
) -> Option<MinaSkillset> {
    bucketing_skillset_in_pool(values, None, length_seconds, rate, chart)
}

fn bucketing_skillset_in_pool(
    values: &SkillVector<EzMinaSkillAxis>,
    pool: Option<&[MinaSkillset]>,
    length_seconds: Option<f64>,
    rate: f64,
    chart: Option<&DanChartFeatures<'_>>,
) -> Option<MinaSkillset> {
    let top = dominant_skillset_in_pool(values, pool)?;
    if has_handstream_endurance_in_pool(values, pool)
        && !jack_contaminated(chart.and_then(|info| info.jack_share))
    {
        return Some(MinaSkillset::Handstream);
    }

    let stream = pool_value(values, pool, MinaSkillset::Stream);
    let endurance = endurance_seconds(length_seconds, rate);
    let demands_endurance =
        endurance.is_some_and(|seconds| seconds >= STAMINA_TILE_MIN_LENGTH_SECONDS);
    if top == MinaSkillset::Stamina
        && demands_endurance
        && stamina_hold_rival_in_pool(values, pool) >= stream - STAMINA_HOLD_BASE_BAND
        && !jack_contaminated(chart.and_then(|info| info.jack_share))
    {
        return Some(top);
    }

    let best = pool_value(values, pool, top);
    if top != MinaSkillset::Handstream
        && chart.and_then(|info| info.handstream_cluster) == Some(true)
        && !jack_contaminated(chart.and_then(|info| info.jack_share))
    {
        let handstream = pool_value(values, pool, MinaSkillset::Handstream);
        if handstream > 0.0 && handstream >= best - HANDSTREAM_NEAR_TIE_MSD {
            return Some(MinaSkillset::Handstream);
        }
    }

    let near_tie =
        if top == MinaSkillset::Stream || (stream > 0.0 && stream >= best - SPEED_NEAR_TIE_MSD) {
            MinaSkillset::Stream
        } else {
            top
        };
    if near_tie == MinaSkillset::Stream {
        let technical = pool_value(values, pool, MinaSkillset::Technical);
        let tech_score = chart.map_or(0.0, |info| info.tech_score);
        let tech_backed = tech_score >= TECH_NEAR_TIE_MIN_SCORE
            && technical > 0.0
            && technical >= stream - SPEED_NEAR_TIE_MSD;
        let lead_backed = tech_score >= PATTERN_TAG_MIN_SCORE
            && technical > 0.0
            && technical - stream >= TECH_NEAR_TIE_MSD_LEAD;
        return Some(if tech_backed || lead_backed {
            MinaSkillset::Technical
        } else {
            MinaSkillset::Stream
        });
    }

    if near_tie == MinaSkillset::Handstream
        && jack_contaminated(chart.and_then(|info| info.jack_share))
    {
        const RICE_POOL: &[MinaSkillset] = &[
            MinaSkillset::Stream,
            MinaSkillset::Jumpstream,
            MinaSkillset::JackSpeed,
            MinaSkillset::Chordjack,
            MinaSkillset::Technical,
        ];
        return bucketing_skillset_in_pool(values, Some(RICE_POOL), None, 1.0, chart);
    }
    if near_tie != MinaSkillset::Stamina || length_seconds.is_none() {
        return Some(near_tie);
    }
    if demands_endurance && !jack_contaminated(chart.and_then(|info| info.jack_share)) {
        return Some(near_tie);
    }
    const BASE_POOL: &[MinaSkillset] = &[
        MinaSkillset::Stream,
        MinaSkillset::Jumpstream,
        MinaSkillset::Handstream,
        MinaSkillset::JackSpeed,
        MinaSkillset::Chordjack,
        MinaSkillset::Technical,
    ];
    bucketing_skillset_in_pool(values, Some(BASE_POOL), None, 1.0, chart)
}

fn has_handstream_endurance(values: &SkillVector<EzMinaSkillAxis>) -> bool {
    dominant_skillset(values) == Some(MinaSkillset::Stamina)
        && dominant_skillset_with_pool(
            values,
            &[
                MinaSkillset::Stream,
                MinaSkillset::Jumpstream,
                MinaSkillset::Handstream,
                MinaSkillset::JackSpeed,
                MinaSkillset::Chordjack,
                MinaSkillset::Technical,
            ],
        ) == Some(MinaSkillset::Handstream)
}

fn has_handstream_endurance_in_pool(
    values: &SkillVector<EzMinaSkillAxis>,
    pool: Option<&[MinaSkillset]>,
) -> bool {
    dominant_skillset_in_pool(values, pool) == Some(MinaSkillset::Stamina)
        && dominant_skillset_from_list(
            values,
            &[
                MinaSkillset::Stream,
                MinaSkillset::Jumpstream,
                MinaSkillset::Handstream,
                MinaSkillset::JackSpeed,
                MinaSkillset::Chordjack,
                MinaSkillset::Technical,
            ],
            pool,
        ) == Some(MinaSkillset::Handstream)
}

fn dominant_skillset_with_pool(
    values: &SkillVector<EzMinaSkillAxis>,
    pool: &[MinaSkillset],
) -> Option<MinaSkillset> {
    dominant_skillset_from_list(values, pool, None)
}

fn dominant_skillset_in_pool(
    values: &SkillVector<EzMinaSkillAxis>,
    allowed: Option<&[MinaSkillset]>,
) -> Option<MinaSkillset> {
    dominant_skillset_from_list(
        values,
        &[
            MinaSkillset::Stream,
            MinaSkillset::Jumpstream,
            MinaSkillset::Handstream,
            MinaSkillset::Stamina,
            MinaSkillset::JackSpeed,
            MinaSkillset::Chordjack,
            MinaSkillset::Technical,
        ],
        allowed,
    )
}

fn dominant_skillset_from_list(
    values: &SkillVector<EzMinaSkillAxis>,
    ordered: &[MinaSkillset],
    allowed: Option<&[MinaSkillset]>,
) -> Option<MinaSkillset> {
    let mut best = None;
    let mut best_value = 0.0;
    for skillset in ordered {
        if allowed.is_some_and(|pool| !pool.contains(skillset)) {
            continue;
        }
        let current = value(values, *skillset);
        if current > best_value {
            best = Some(*skillset);
            best_value = current;
        }
    }
    best
}

fn stamina_hold_rival_in_pool(
    values: &SkillVector<EzMinaSkillAxis>,
    pool: Option<&[MinaSkillset]>,
) -> f64 {
    [
        MinaSkillset::Technical,
        MinaSkillset::Jumpstream,
        MinaSkillset::Handstream,
    ]
    .into_iter()
    .map(|skillset| pool_value(values, pool, skillset))
    .fold(0.0, f64::max)
}

fn pool_value(
    values: &SkillVector<EzMinaSkillAxis>,
    pool: Option<&[MinaSkillset]>,
    skillset: MinaSkillset,
) -> f64 {
    if pool.is_some_and(|allowed| !allowed.contains(&skillset)) {
        0.0
    } else {
        value(values, skillset)
    }
}

fn value(values: &SkillVector<EzMinaSkillAxis>, skillset: MinaSkillset) -> f64 {
    let axis = match skillset {
        MinaSkillset::Stream => EzMinaSkillAxis::Stream,
        MinaSkillset::Jumpstream => EzMinaSkillAxis::Jumpstream,
        MinaSkillset::Handstream => EzMinaSkillAxis::Handstream,
        MinaSkillset::Stamina => EzMinaSkillAxis::Stamina,
        MinaSkillset::JackSpeed => EzMinaSkillAxis::JackSpeed,
        MinaSkillset::Chordjack => EzMinaSkillAxis::Chordjack,
        MinaSkillset::Technical => EzMinaSkillAxis::Technical,
    };
    finite_value(values, axis)
}

fn finite_value(values: &SkillVector<EzMinaSkillAxis>, axis: EzMinaSkillAxis) -> f64 {
    let value = values.get(axis);
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

fn endurance_seconds(length_seconds: Option<f64>, rate: f64) -> Option<f64> {
    let length = length_seconds?;
    let safe_rate = if rate.is_finite() && rate > 0.0 {
        rate
    } else {
        1.0
    };
    Some(length.max(length / safe_rate))
}

fn jack_contaminated(jack_share: Option<f64>) -> bool {
    jack_share.is_some_and(|share| share >= STAMINA_TILE_JACK_VETO_SHARE)
}

fn buckets_for_clear(
    buckets: &[BucketDef],
    top: Option<MinaSkillset>,
    chart: Option<&DanChartFeatures<'_>>,
    values: &SkillVector<EzMinaSkillAxis>,
    rate: f64,
) -> DanBucketVector {
    let mut filed = DanBucketVector::new();

    if chart.is_some_and(|info| info.jack_demand || trill_is_jack(info)) {
        if let Some(jack) = buckets
            .iter()
            .find(|bucket| bucket.id == DanBucketId::Jack && !bucket.skillsets.is_empty())
        {
            filed.push(jack.id);
            return resolve_tiles_for_clear(filed, buckets, chart, values);
        }
    }

    if let Some(override_bucket) = buckets.iter().find(|bucket| {
        !bucket.skillsets.is_empty()
            && !bucket.tags.is_empty()
            && classify_bucket_tag_match(bucket, chart)
    }) {
        filed.push(override_bucket.id);
        return resolve_tiles_for_clear(filed, buckets, chart, values);
    }

    if chart
        .is_some_and(|info| info.tech_category == Some(false) && info.cluster_trill == Some(false))
        && chart.is_some_and(|info| is_handstream_endurance(info, values))
        && !jack_contaminated(chart.and_then(|info| info.jack_share))
    {
        if let Some(stamina) = buckets
            .iter()
            .find(|bucket| bucket.id == DanBucketId::Stamina && !bucket.skillsets.is_empty())
        {
            filed.push(stamina.id);
            return filed;
        }
    }

    let contaminated = jack_contaminated(chart.and_then(|info| info.jack_share));
    let effective_top = match (top, chart.and_then(|info| info.cluster_trill)) {
        (Some(MinaSkillset::Jumpstream), Some(true)) => {
            let endurance =
                endurance_seconds(chart.and_then(|info| info.length_seconds), rate).unwrap_or(0.0);
            if endurance >= TRILL_RUNNER_UP_MIN_LENGTH_SECONDS {
                jumpstream_runner_up(values, contaminated)
            } else {
                top
            }
        }
        (Some(MinaSkillset::Jumpstream), Some(false))
            if chart.and_then(|info| info.tech_category) == Some(true) =>
        {
            jumpstream_runner_up(values, contaminated)
        }
        (Some(MinaSkillset::Jumpstream), Some(false)) => Some(if contaminated {
            MinaSkillset::Jumpstream
        } else {
            MinaSkillset::Stamina
        }),
        _ => top,
    };

    for bucket in buckets {
        let matches = if bucket.skillsets.is_empty() {
            classify_bucket_tag_match(bucket, chart)
        } else {
            effective_top.is_some_and(|skillset| bucket.skillsets.contains(&skillset))
        };
        if matches {
            filed.push(bucket.id);
        }
    }
    resolve_tiles_for_clear(filed, buckets, chart, values)
}

fn resolve_tiles_for_clear(
    filed: DanBucketVector,
    buckets: &[BucketDef],
    chart: Option<&DanChartFeatures<'_>>,
    values: &SkillVector<EzMinaSkillAxis>,
) -> DanBucketVector {
    if filed.len() != 1 {
        return filed;
    }
    let primary = filed.get(0).unwrap();
    let Some(primary_def) = buckets.iter().find(|bucket| bucket.id == primary) else {
        return filed;
    };
    if primary_def.skillsets.is_empty() {
        return filed;
    }

    if matches!(primary, DanBucketId::Tech | DanBucketId::Speed) {
        let modelled = speed_tech_tiles(
            values,
            chart.and_then(|info| info.motion),
            chart.map_or(0.0, |info| info.tech_score),
        );
        let Some((chosen, shared)) = modelled else {
            return filed;
        };
        if !shared {
            return singleton(chosen);
        }
        let other = if chosen == DanBucketId::Tech {
            DanBucketId::Speed
        } else {
            DanBucketId::Tech
        };
        return pair(chosen, other);
    }

    if primary == DanBucketId::Stamina {
        if dominant_skillset(values) != Some(MinaSkillset::Stamina) {
            return filed;
        }
        let base = dominant_skillset_with_pool(
            values,
            &[
                MinaSkillset::Stream,
                MinaSkillset::Jumpstream,
                MinaSkillset::Handstream,
                MinaSkillset::JackSpeed,
                MinaSkillset::Chordjack,
                MinaSkillset::Technical,
            ],
        );
        if !matches!(base, Some(MinaSkillset::Stream | MinaSkillset::Technical)) {
            return filed;
        }
        if speed_tech_tiles(
            values,
            chart.and_then(|info| info.motion),
            chart.map_or(0.0, |info| info.tech_score),
        )
        .is_some_and(|(chosen, _)| chosen == DanBucketId::Tech)
        {
            return pair(primary, DanBucketId::Tech);
        }
    }
    filed
}

fn speed_tech_tiles(
    values: &SkillVector<EzMinaSkillAxis>,
    motion: Option<DanMotionFeatures>,
    tech_score: f64,
) -> Option<(DanBucketId, bool)> {
    let motion = motion?;
    let inputs = [
        motion.rhythm_break,
        motion.cross_hand_trill,
        motion.mini_jack,
        motion.same_hand,
        value(values, MinaSkillset::Technical) - value(values, MinaSkillset::Stream),
        tech_score,
    ];
    const TERMS: [(f64, f64, f64); 6] = [
        (0.0439, 0.0659, 0.7207),
        (0.0551, 0.0456, 0.9009),
        (0.0044, 0.0065, 0.5595),
        (0.2300, 0.0534, 0.2510),
        (-0.3886, 0.7180, 0.1252),
        (0.3927, 0.1981, 1.2100),
    ];
    let mut z = -0.5869;
    for (input, (mean, sd, weight)) in inputs.into_iter().zip(TERMS) {
        if !input.is_finite() {
            return None;
        }
        z += weight * ((input - mean) / sd);
    }
    let probability = 1.0 / (1.0 + (-z).exp());
    Some((
        if probability >= 0.5 {
            DanBucketId::Tech
        } else {
            DanBucketId::Speed
        },
        probability > SPEED_TECH_DUAL_LOW && probability < SPEED_TECH_DUAL_HIGH,
    ))
}

fn jumpstream_runner_up(
    values: &SkillVector<EzMinaSkillAxis>,
    contaminated: bool,
) -> Option<MinaSkillset> {
    let pool = if contaminated {
        &[
            MinaSkillset::Stream,
            MinaSkillset::JackSpeed,
            MinaSkillset::Chordjack,
            MinaSkillset::Technical,
        ][..]
    } else {
        &[
            MinaSkillset::Stream,
            MinaSkillset::Handstream,
            MinaSkillset::Stamina,
            MinaSkillset::JackSpeed,
            MinaSkillset::Chordjack,
            MinaSkillset::Technical,
        ][..]
    };
    dominant_skillset_with_pool(values, pool).or(Some(MinaSkillset::Jumpstream))
}

fn is_handstream_endurance(
    chart: &DanChartFeatures<'_>,
    values: &SkillVector<EzMinaSkillAxis>,
) -> bool {
    if chart.key_count != Some(4)
        || chart
            .ln_ratio
            .is_some_and(|ratio| ratio >= LN_PRIMARY_MIN_RATIO)
    {
        return false;
    }
    has_handstream_endurance(values)
}

fn trill_is_jack(chart: &DanChartFeatures<'_>) -> bool {
    if chart.cluster_trill != Some(true) {
        return false;
    }
    chart.chordjack_score >= TRILL_JACK_MIN_CHORDJACK
        || (chart.chordjack_score >= TRILL_JACK_CORROBORATED_CHORDJACK
            && chart.jack_share.unwrap_or(0.0) >= TRILL_JACK_CORROBORATED_SHARE)
}

fn singleton(bucket: DanBucketId) -> DanBucketVector {
    let mut result = DanBucketVector::new();
    result.push(bucket);
    result
}

fn pair(first: DanBucketId, second: DanBucketId) -> DanBucketVector {
    let mut result = singleton(first);
    result.push(second);
    result
}

const TRILL_JACK_MIN_CHORDJACK: f64 = 0.60;
const TRILL_JACK_CORROBORATED_CHORDJACK: f64 = 0.55;
const TRILL_JACK_CORROBORATED_SHARE: f64 = 0.15;
const TRILL_RUNNER_UP_MIN_LENGTH_SECONDS: f64 = 240.0;
const STAMINA_TILE_JACK_VETO_SHARE: f64 = 0.30;
const STAMINA_TILE_MIN_LENGTH_SECONDS: f64 = 240.0;
const STAMINA_HOLD_BASE_BAND: f64 = 0.5;
const SPEED_NEAR_TIE_MSD: f64 = 1.25;
const TECH_NEAR_TIE_MIN_SCORE: f64 = 0.8;
const TECH_NEAR_TIE_MSD_LEAD: f64 = 0.35;
const HANDSTREAM_NEAR_TIE_MSD: f64 = 0.95;
const SPEED_TECH_DUAL_LOW: f64 = 0.35;
const SPEED_TECH_DUAL_HIGH: f64 = 0.75;
const PATTERN_TAG_MIN_SCORE: f64 = 0.5;
const CLUSTER_SHARE_MIN: f64 = 0.4;
const LN_PRIMARY_MIN_RATIO: f64 = 0.45;

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> SkillVector<EzMinaSkillAxis> {
        let mut axes = SkillVector::zeros();
        axes.set(EzMinaSkillAxis::Overall, 16.0);
        axes.set(EzMinaSkillAxis::Chordjack, 20.0);
        axes.set(EzMinaSkillAxis::Stream, 6.0);
        axes
    }

    #[test]
    fn four_key_rc_files_dominant_chordjack_clear_into_jack() {
        let axes = values();
        let result = file_chart_skillsets(DanFilingInput {
            key_count: 4,
            side: DanSide::Rc,
            mina_values: &axes,
            chart_features: None,
            length_seconds: Some(90.0),
            rate: 1.0,
        });
        assert_eq!(result.iter().collect::<Vec<_>>(), vec![DanBucketId::Jack]);
        assert_eq!(result.get(0).unwrap().id(), "jack");
    }

    #[test]
    fn seven_key_rc_files_pattern_tags_without_leoblack_features() {
        let axes = values();
        let tags = [DanChartSkillTag::Delay];
        let chart = DanChartFeatures {
            skill_tags: &tags,
            key_count: Some(7),
            dan_eligible: true,
            ..Default::default()
        };
        let result = file_chart_skillsets(DanFilingInput {
            key_count: 7,
            side: DanSide::Rc,
            mina_values: &axes,
            chart_features: Some(&chart),
            length_seconds: Some(90.0),
            rate: 1.0,
        });
        assert_eq!(result.iter().collect::<Vec<_>>(), vec![DanBucketId::Speed]);
    }

    #[test]
    fn seven_key_ln_general_and_release_tags_are_both_stamped() {
        let axes = values();
        let tags = [
            DanChartSkillTag::LnGeneral,
            DanChartSkillTag::Ln,
            DanChartSkillTag::LnReleasePattern,
        ];
        let chart = DanChartFeatures {
            skill_tags: &tags,
            key_count: Some(7),
            ln_ratio: Some(0.5),
            dan_eligible: true,
            ..Default::default()
        };
        let result = file_chart_skillsets(DanFilingInput {
            key_count: 7,
            side: DanSide::Ln,
            mina_values: &axes,
            chart_features: Some(&chart),
            length_seconds: Some(120.0),
            rate: 1.0,
        });
        assert_eq!(
            result.iter().collect::<Vec<_>>(),
            vec![DanBucketId::LnGeneral, DanBucketId::LnReleasePattern]
        );
    }

    #[test]
    fn unavailable_chart_metadata_yields_no_non_four_key_filing() {
        let axes = values();
        let result = file_chart_skillsets(DanFilingInput {
            key_count: 7,
            side: DanSide::Rc,
            mina_values: &axes,
            chart_features: None,
            length_seconds: Some(90.0),
            rate: 1.0,
        });
        assert!(result.is_empty());
    }

    #[test]
    fn chart_vibro_and_ineligible_gate_clears_before_filing() {
        let axes = values();
        let vibro = DanChartFeatures {
            vibro: true,
            ..Default::default()
        };
        let ineligible = DanChartFeatures {
            dan_eligible: false,
            ..Default::default()
        };
        for chart in [&vibro, &ineligible] {
            let result = file_chart_skillsets(DanFilingInput {
                key_count: 4,
                side: DanSide::Rc,
                mina_values: &axes,
                chart_features: Some(chart),
                length_seconds: Some(90.0),
                rate: 1.0,
            });
            assert!(result.is_empty());
        }
    }
}
