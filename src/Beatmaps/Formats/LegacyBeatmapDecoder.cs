using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using LAsOsuBeatmapParser.Beatmaps;
using LAsOsuBeatmapParser.Exceptions;

namespace LAsOsuBeatmapParser.Beatmaps.Formats;

/// <summary>
/// osu! .osu 文件解析器。
/// </summary>
public class LegacyBeatmapDecoder
{
    /// <summary>
    /// 创建 LegacyBeatmapDecoder。
    /// </summary>
    public LegacyBeatmapDecoder()
    {
    }

    /// <summary>
    /// 同步从文件路径解析谱面。
    /// </summary>
    /// <param name="filePath">.osu 文件路径。</param>
    /// <returns>解析得到的谱面对象。</returns>
    /// <exception cref="BeatmapParseException">解析失败时抛出。</exception>
    public Beatmap Decode(string filePath)
    {
        try
        {
            using var stream = File.OpenRead(filePath);
            return Decode(stream);
        }
        catch (Exception ex)
        {
            throw new BeatmapParseException($"无法从文件解析谱面: {filePath}", ex);
        }
    }

    /// <summary>
    /// 异步从文件路径解析谱面。
    /// </summary>
    /// <param name="filePath">.osu 文件路径。</param>
    /// <returns>解析得到的谱面对象。</returns>
    /// <exception cref="BeatmapParseException">解析失败时抛出。</exception>
    public async Task<Beatmap> DecodeAsync(string filePath)
    {
        try
        {
            using var stream = File.OpenRead(filePath);
            return await DecodeAsync(stream);
        }
        catch (Exception ex)
        {
            throw new BeatmapParseException($"无法从文件解析谱面: {filePath}", ex);
        }
    }

    /// <summary>
    /// 从流中同步解析谱面。
    /// </summary>
    /// <param name="stream">包含 .osu 数据的流。</param>
    /// <returns>解析得到的谱面对象。</returns>
    /// <exception cref="BeatmapParseException">解析失败时抛出。</exception>
    public Beatmap Decode(Stream stream)
    {
        using var reader = new StreamReader(stream);
        return ParseBeatmap(reader);
    }

    /// <summary>
    /// 从流中异步解析谱面。
    /// </summary>
    /// <param name="stream">包含 .osu 数据的流。</param>
    /// <returns>解析得到的谱面对象。</returns>
    /// <exception cref="BeatmapParseException">解析失败时抛出。</exception>
    public async Task<Beatmap> DecodeAsync(Stream stream)
    {
        using var reader = new StreamReader(stream);
        return await Task.Run(() => ParseBeatmap(reader));
    }

    private Beatmap ParseBeatmap(StreamReader reader)
    {
        var beatmap = new Beatmap();
        string? line;
        string currentSection = "";

        while ((line = reader.ReadLine()) != null)
        {
            line = line.Trim();

            if (string.IsNullOrEmpty(line))
                continue;

            // Handle version line
            if (line.StartsWith("osu file format v"))
            {
                if (int.TryParse(line.Substring("osu file format v".Length), out var version))
                {
                    beatmap.Version = version;
                }
                continue;
            }

            if (line.StartsWith("[") && line.EndsWith("]"))
            {
                currentSection = line[1..^1];
                continue;
            }

            // Skip global comments, but allow section-specific comments
            if (line.StartsWith("//") && currentSection != "Events")
                continue;

            ParseLine(beatmap, currentSection, line);
        }

        // 解析后计算 BPM 和 Matrix
        beatmap.BPM = beatmap.GetBPM();
        beatmap.Matrix = beatmap.BuildMatrix();

        return beatmap;
    }

    private void ParseLine(Beatmap beatmap, string section, string line)
    {
        try
        {
            switch (section)
            {
                case "General":
                    ParseGeneral(beatmap, line);
                    break;
                case "Metadata":
                    ParseMetadata(beatmap, line);
                    break;
                case "Difficulty":
                    ParseDifficulty(beatmap, line);
                    break;
                case "TimingPoints":
                    ParseTimingPoint(beatmap, line);
                    break;
                case "HitObjects":
                    ParseHitObject(beatmap, line);
                    break;
                case "Events":
                    ParseEvent(beatmap, line);
                    break;
                case "Editor":
                    ParseEditor(beatmap, line);
                    break;
                    // 根据需要添加其他部分
            }
        }
        catch (Exception)
        {
            // 如果需要，记录解析错误
        }
    }

    private void ParseGeneral(Beatmap beatmap, string line)
    {
        var parts = line.Split(':', 2);
        if (parts.Length != 2) return;

        var key = parts[0].Trim();
        var value = parts[1].Trim();

        switch (key)
        {
            case "AudioFilename":
                beatmap.Metadata.AudioFile = value;
                break;
            case "AudioLeadIn":
                if (int.TryParse(value, out var audioLeadIn))
                {
                    beatmap.AudioLeadIn = audioLeadIn;
                    beatmap.AudioLeadInSet = true;
                }
                break;
            case "PreviewTime":
                if (int.TryParse(value, out var previewTime))
                    beatmap.Metadata.PreviewTime = previewTime;
                break;
            case "Countdown":
                if (int.TryParse(value, out var countdownInt) && Enum.IsDefined(typeof(CountdownType), countdownInt))
                {
                    beatmap.Countdown = (CountdownType)countdownInt;
                    beatmap.CountdownSet = true;
                }
                break;
            case "SampleSet":
                beatmap.SampleSet = value;
                beatmap.SampleSetSet = true;
                break;
            case "StackLeniency":
                if (double.TryParse(value, out var stackLeniency))
                {
                    beatmap.StackLeniency = stackLeniency;
                    beatmap.StackLeniencySet = true;
                }
                break;
            case "Mode":
                if (int.TryParse(value, out var modeInt) && Enum.IsDefined(typeof(GameMode), modeInt))
                {
                    beatmap.Mode = (GameMode)modeInt;
                }
                break;
            case "LetterboxInBreaks":
                if (int.TryParse(value, out var letterboxInt))
                    beatmap.LetterboxInBreaks = letterboxInt != 0;
                else if (bool.TryParse(value, out var letterboxInBreaks))
                    beatmap.LetterboxInBreaks = letterboxInBreaks;
                beatmap.LetterboxInBreaksSet = true;
                break;
            case "StoryFireInFront":
                if (int.TryParse(value, out var storyFireInt))
                    beatmap.StoryFireInFront = storyFireInt != 0;
                else if (bool.TryParse(value, out var storyFireInFront))
                    beatmap.StoryFireInFront = storyFireInFront;
                beatmap.StoryFireInFrontSet = true;
                break;
            case "UseSkinSprites":
                if (int.TryParse(value, out var useSkinInt))
                    beatmap.UseSkinSprites = useSkinInt != 0;
                else if (bool.TryParse(value, out var useSkinSprites))
                    beatmap.UseSkinSprites = useSkinSprites;
                beatmap.UseSkinSpritesSet = true;
                break;
            case "AlwaysShowPlayfield":
                if (int.TryParse(value, out var alwaysShowInt))
                    beatmap.AlwaysShowPlayfield = alwaysShowInt != 0;
                else if (bool.TryParse(value, out var alwaysShowPlayfield))
                    beatmap.AlwaysShowPlayfield = alwaysShowPlayfield;
                beatmap.AlwaysShowPlayfieldSet = true;
                break;
            case "OverlayPosition":
                beatmap.OverlayPosition = value;
                beatmap.OverlayPositionSet = true;
                break;
            case "SkinPreference":
                beatmap.SkinPreference = value;
                beatmap.SkinPreferenceSet = true;
                break;
            case "EpilepsyWarning":
                if (int.TryParse(value, out var epilepsyInt))
                    beatmap.EpilepsyWarning = epilepsyInt != 0;
                else if (bool.TryParse(value, out var epilepsyWarning))
                    beatmap.EpilepsyWarning = epilepsyWarning;
                beatmap.EpilepsyWarningSet = true;
                break;
            case "CountdownOffset":
                if (int.TryParse(value, out var countdownOffset))
                {
                    beatmap.CountdownOffset = countdownOffset;
                    beatmap.CountdownOffsetSet = true;
                }
                break;
            case "SpecialStyle":
                if (int.TryParse(value, out var specialInt))
                    beatmap.SpecialStyle = specialInt != 0;
                else if (bool.TryParse(value, out var specialStyle))
                    beatmap.SpecialStyle = specialStyle;
                beatmap.SpecialStyleSet = true;
                break;
            case "WidescreenStoryboard":
                if (int.TryParse(value, out var widescreenInt))
                    beatmap.WidescreenStoryboard = widescreenInt != 0;
                else if (bool.TryParse(value, out var widescreenStoryboard))
                    beatmap.WidescreenStoryboard = widescreenStoryboard;
                beatmap.WidescreenStoryboardSet = true;
                break;
            case "SamplesMatchPlaybackRate":
                if (int.TryParse(value, out var samplesInt))
                    beatmap.SamplesMatchPlaybackRate = samplesInt != 0;
                else if (bool.TryParse(value, out var samplesMatchPlaybackRate))
                    beatmap.SamplesMatchPlaybackRate = samplesMatchPlaybackRate;
                beatmap.SamplesMatchPlaybackRateSet = true;
                break;
        }
    }

    private void ParseMetadata(Beatmap beatmap, string line)
    {
        var parts = line.Split(':', 2);
        if (parts.Length != 2) return;

        var key = parts[0].Trim();
        var value = parts[1].Trim();

        switch (key)
        {
            case "Title":
                beatmap.Metadata.Title = value;
                break;
            case "TitleUnicode":
                beatmap.Metadata.TitleUnicode = value;
                break;
            case "Artist":
                beatmap.Metadata.Artist = value;
                break;
            case "ArtistUnicode":
                beatmap.Metadata.ArtistUnicode = value;
                break;
            case "Creator":
                beatmap.Metadata.Creator = value;
                beatmap.Metadata.Author.Username = value;
                break;
            case "Version":
                beatmap.Metadata.Version = value;
                beatmap.BeatmapInfo.DifficultyName = value;
                break;
            case "BeatmapID":
                if (int.TryParse(value, out var id))
                    beatmap.Metadata.BeatmapID = id;
                break;
            case "BeatmapSetID":
                if (int.TryParse(value, out var setId))
                    beatmap.Metadata.BeatmapSetID = setId;
                break;
            case "Source":
                beatmap.Metadata.Source = value;
                break;
            case "Tags":
                beatmap.Metadata.Tags = value;
                break;
                // 添加其他元数据
        }
    }

    private void ParseDifficulty(Beatmap beatmap, string line)
    {
        var parts = line.Split(':', 2);
        if (parts.Length != 2) return;

        var key = parts[0].Trim();
        var value = parts[1].Trim();

        switch (key)
        {
            case "HPDrainRate":
                if (float.TryParse(value, out var hp))
                    beatmap.Difficulty.HPDrainRate = hp;
                break;
            case "CircleSize":
                if (float.TryParse(value, out var cs))
                    beatmap.Difficulty.CircleSize = cs;
                break;
            case "OverallDifficulty":
                if (float.TryParse(value, out var od))
                    beatmap.Difficulty.OverallDifficulty = od;
                break;
            case "ApproachRate":
                if (float.TryParse(value, out var ar))
                    beatmap.Difficulty.ApproachRate = ar;
                break;
            case "SliderMultiplier":
                if (float.TryParse(value, out var sm))
                    beatmap.Difficulty.SliderMultiplier = sm;
                break;
            case "SliderTickRate":
                if (float.TryParse(value, out var str))
                    beatmap.Difficulty.SliderTickRate = str;
                break;
        }
    }

    private void ParseTimingPoint(Beatmap beatmap, string line)
    {
        var parts = line.Split(',');
        if (parts.Length < 2) return;

        var timingPoint = new TimingPoint();

        if (double.TryParse(parts[0], out var time))
            timingPoint.Time = time;

        if (double.TryParse(parts[1], out var beatLength))
            timingPoint.BeatLength = beatLength;

        if (parts.Length > 2 && int.TryParse(parts[2], out var meter))
            timingPoint.Meter = meter;

        if (parts.Length > 3 && int.TryParse(parts[3], out var sampleSet))
            timingPoint.SampleSet = sampleSet;

        if (parts.Length > 4 && int.TryParse(parts[4], out var sampleIndex))
            timingPoint.SampleIndex = sampleIndex;

        if (parts.Length > 5 && int.TryParse(parts[5], out var volume))
            timingPoint.Volume = volume;

        if (parts.Length > 6 && int.TryParse(parts[6], out var inherited))
            timingPoint.Inherited = inherited == 0;

        if (parts.Length > 7 && int.TryParse(parts[7], out var effects))
            timingPoint.Effects = effects;

        beatmap.TimingPoints.Add(timingPoint);
    }

    private void ParseHitObject(Beatmap beatmap, string line)
    {
        var parts = line.Split(',');
        if (parts.Length < 4) return;

        HitObject hitObject;

        if (int.TryParse(parts[3], out var typeInt))
        {
            var type = (HitObjectType)typeInt;

            if (type.HasFlag(HitObjectType.ManiaHold) && beatmap.Mode == GameMode.Mania)
            {
                hitObject = ParseManiaHold(beatmap, parts);
            }
            else if (type.HasFlag(HitObjectType.Spinner))
            {
                hitObject = ParseSpinner(parts);
            }
            else if (type.HasFlag(HitObjectType.Slider))
            {
                hitObject = ParseSlider(parts);
            }
            else if (beatmap.Mode == GameMode.Mania)
            {
                hitObject = ParseManiaHitObject(beatmap, parts);
            }
            else
            {
                hitObject = ParseHitCircle(parts);
            }

            hitObject.Type = type;
            beatmap.HitObjects.Add(hitObject);
        }
    }

    private Note ParseHitCircle(string[] parts)
    {
        var hitCircle = new Note();

        if (double.TryParse(parts[2], out var startTime))
            hitCircle.StartTime = startTime;

        if (float.TryParse(parts[0], out var x) && float.TryParse(parts[1], out var y))
            hitCircle.Position = (x, y);

        if (parts.Length > 4 && int.TryParse(parts[4], out var hitsound))
            hitCircle.Hitsound = hitsound;

        if (parts.Length > 5)
            hitCircle.ObjectParams = string.Join(",", parts[5..]);

        return hitCircle;
    }

    private Slider ParseSlider(string[] parts)
    {
        var slider = new Slider();

        if (double.TryParse(parts[2], out var startTime))
            slider.StartTime = startTime;

        if (float.TryParse(parts[0], out var x) && float.TryParse(parts[1], out var y))
            slider.Position = (X: x, Y: y);

        if (parts.Length > 5)
        {
            var curveParts = parts[5].Split('|');
            foreach (var curvePoint in curveParts.Skip(1)) // 跳过曲线类型
            {
                var coords = curvePoint.Split(':');
                if (coords.Length == 2 &&
                    float.TryParse(coords[0], out var cx) &&
                    float.TryParse(coords[1], out var cy))
                {
                    slider.CurvePoints.Add((X: cx, Y: cy));
                }
            }
        }

        if (parts.Length > 7 && double.TryParse(parts[7], out var duration))
            slider.Duration = duration;

        return slider;
    }

    private Spinner ParseSpinner(string[] parts)
    {
        var spinner = new Spinner();

        if (double.TryParse(parts[2], out var startTime))
            spinner.StartTime = startTime;

        if (parts.Length > 5 && double.TryParse(parts[5], out var endTime))
            spinner.EndTimeValue = endTime;

        return spinner;
    }

    private ManiaHoldNote ParseManiaHold(Beatmap beatmap, string[] parts)
    {
        var hold = new ManiaHoldNote();

        if (double.TryParse(parts[2], out var startTime))
            hold.StartTime = startTime;

        if (parts.Length > 5)
        {
            var holdParts = parts[5].Split(':');
            if (holdParts.Length > 0 && double.TryParse(holdParts[0], out var endTime))
                hold.EndTime = endTime;
        }

        // 根据 x 位置和键数计算列索引
        if (float.TryParse(parts[0], out var x))
        {
            // 对于 Mania，x 位置决定音符所在列
            // 使用官方的坐标转换公式：column = floor(x / (512 / keyCount))
            int keyCount = (int)beatmap.Difficulty.CircleSize;
            float ratio = 512f / keyCount;
            hold.Column = (int)(x / ratio);
            hold.KeyCount = keyCount;
        }

        return hold;
    }

    private ManiaHitObject ParseManiaHitObject(Beatmap beatmap, string[] parts)
    {
        var maniaHit = new ManiaHitObject();

        if (double.TryParse(parts[2], out var startTime))
            maniaHit.StartTime = startTime;

        // 根据 x 位置和键数计算列索引
        if (float.TryParse(parts[0], out var x) && float.TryParse(parts[1], out var y))
        {
            int keyCount = (int)beatmap.Difficulty.CircleSize;

            if (y == 0)
            {
                // Old format: x directly represents column index
                maniaHit.Column = (int)x;
            }
            else
            {
                // Standard format: calculate column from x position
                // 使用官方的坐标转换公式：column = floor(x / (512 / keyCount))
                float ratio = 512f / keyCount;
                maniaHit.Column = (int)(x / ratio);
            }

            maniaHit.KeyCount = keyCount;
        }

        if (parts.Length > 4 && int.TryParse(parts[4], out var hitsound))
            maniaHit.Hitsound = hitsound;

        if (parts.Length > 5)
            maniaHit.ObjectParams = string.Join(",", parts[5..]);

        return maniaHit;
    }

    private void ParseEvent(Beatmap beatmap, string line)
    {
        // Check if this is a comment line
        if (line.TrimStart().StartsWith("//"))
        {
            var commentEvent = new Event
            {
                IsComment = true,
                Params = line
            };
            beatmap.Events.Add(commentEvent);
            return;
        }

        var parts = line.Split(',');
        if (parts.Length < 2) return;

        var eventObj = new Event
        {
            Type = parts[0],
            StartTime = double.TryParse(parts[1], out var time) ? time : 0,
            Params = parts.Length > 2 ? string.Join(",", parts[2..]) : ""
        };

        // Check if this is a background event (type 0)
        if (eventObj.Type == "0" && parts.Length >= 4)
        {
            // Extract background filename from params like "filename.jpg",0,0
            var paramParts = eventObj.Params.Split(',');
            if (paramParts.Length >= 1)
            {
                var filename = paramParts[0].Trim('"'); // Remove quotes
                if (!string.IsNullOrEmpty(filename))
                {
                    beatmap.Metadata.BackgroundFile = filename;
                }
            }
        }

        beatmap.Events.Add(eventObj);
    }

    private void ParseEditor(Beatmap beatmap, string line)
    {
        var parts = line.Split(':', 2);
        if (parts.Length != 2) return;

        var key = parts[0].Trim();
        var value = parts[1].Trim();

        switch (key)
        {
            case "Bookmarks":
                if (!string.IsNullOrEmpty(value))
                {
                    var bookmarkStrings = value.Split(',');
                    var bookmarks = new List<int>();
                    foreach (var bookmarkStr in bookmarkStrings)
                    {
                        if (int.TryParse(bookmarkStr.Trim(), out var bookmark))
                            bookmarks.Add(bookmark);
                    }
                    beatmap.Bookmarks = bookmarks.ToArray();
                    beatmap.BookmarksSet = true;
                }
                break;
            case "DistanceSpacing":
                if (double.TryParse(value, out var distanceSpacing))
                {
                    beatmap.DistanceSpacing = distanceSpacing;
                    beatmap.DistanceSpacingSet = true;
                }
                break;
            case "BeatDivisor":
                if (int.TryParse(value, out var beatDivisor))
                    beatmap.BeatmapInfo.BeatDivisor = beatDivisor;
                break;
            case "GridSize":
                if (int.TryParse(value, out var gridSize))
                {
                    beatmap.GridSize = gridSize;
                    beatmap.GridSizeSet = true;
                }
                break;
            case "TimelineZoom":
                if (double.TryParse(value, out var timelineZoom))
                {
                    beatmap.TimelineZoom = timelineZoom;
                    beatmap.TimelineZoomSet = true;
                }
                break;
        }
    }
}
