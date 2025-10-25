using System;
using System.Collections.Generic;
using System.Text.Json;
using System.Text.Json.Serialization;
using LAsOsuBeatmapParser.Beatmaps;

namespace LAsOsuBeatmapParser.Converters
{
    /// <summary>
    ///     用于处理 List&lt;HitObject&gt; 多态的 JSON 转换器。
    /// </summary>
    public class HitObjectListConverter : JsonConverter<List<HitObject>>
    {
        /// <summary>
        /// </summary>
        /// <param name="reader"></param>
        /// <param name="typeToConvert"></param>
        /// <param name="options"></param>
        /// <returns></returns>
        /// <exception cref="JsonException"></exception>
        public override List<HitObject> Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
        {
            var list = new List<HitObject>();

            if (reader.TokenType != JsonTokenType.StartArray)
                throw new JsonException();

            while (reader.Read())
            {
                if (reader.TokenType == JsonTokenType.EndArray)
                    break;

                if (reader.TokenType == JsonTokenType.StartObject)
                {
                    // 读取对象以判断类型
                    using JsonDocument doc  = JsonDocument.ParseValue(ref reader);
                    JsonElement        root = doc.RootElement;

                    if (root.TryGetProperty("Type", out JsonElement typeProp))
                    {
                        int typeInt = typeProp.GetInt32();
                        var type    = (HitObjectType)typeInt;

                        HitObject? hitObject;

                        if (type.HasFlag(HitObjectType.ManiaHold))
                            hitObject = JsonSerializer.Deserialize<ManiaHoldNote>(root.GetRawText(), options);
                        else if (type.HasFlag(HitObjectType.Spinner))
                            hitObject = JsonSerializer.Deserialize<Spinner>(root.GetRawText(), options);
                        else if (type.HasFlag(HitObjectType.Slider))
                            hitObject = JsonSerializer.Deserialize<Slider>(root.GetRawText(), options);
                        else
                            hitObject = JsonSerializer.Deserialize<Note>(root.GetRawText(), options);

                        if (hitObject != null)
                            list.Add(hitObject);
                    }
                }
            }

            return list;
        }

        /// <summary>
        /// </summary>
        /// <param name="writer"></param>
        /// <param name="value"></param>
        /// <param name="options"></param>
        public override void Write(Utf8JsonWriter writer, List<HitObject> value, JsonSerializerOptions options)
        {
            writer.WriteStartArray();

            foreach (HitObject hitObject in value) JsonSerializer.Serialize(writer, hitObject, hitObject.GetType(), options);

            writer.WriteEndArray();
        }
    }
}
