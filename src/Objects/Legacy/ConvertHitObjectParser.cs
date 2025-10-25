// Copyright (c) SK_la. Licensed under the MIT Licence.

namespace LAsOsuBeatmapParser.Objects.Legacy
{
    /// <summary>
    ///     A HitObjectParser to parse legacy Beatmaps.
    /// </summary>
    public class ConvertHitObjectParser : HitObjectParser
    {
        /// <summary>
        ///     The .osu format (beatmap) version.
        /// </summary>
        private readonly int formatVersion;
        /// <summary>
        ///     The offset to apply to all time values.
        /// </summary>
        private readonly double offset;

        internal ConvertHitObjectParser(double offset, int formatVersion)
        {
            this.offset        = offset;
            this.formatVersion = formatVersion;
        }

        public override HitObject? Parse(string text)
        {
            string[] split = text.Split(',');

            if (split.Length < 4)
                return null;

            // Parse position
            if (!float.TryParse(split[0], out float x) || !float.TryParse(split[1], out float y))
                return null;

            // Parse start time
            if (!double.TryParse(split[2], out double startTime))
                return null;

            startTime += offset;

            // Parse type
            if (!int.TryParse(split[3], out int typeValue))
                return null;

            // For now, create a basic ConvertHitObject
            var result = new ConvertHitObject
            {
                StartTime = startTime,
                X         = x,
                Y         = y
            };

            // Parse samples if available
            if (split.Length > 4 && int.TryParse(split[4], out int hitSound))
            {
                // Add basic hit sample
                result.Samples.Add(new HitSampleInfo(HitSampleInfo.HIT_NORMAL));
            }

            return result;
        }
    }
}
