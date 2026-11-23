//! Lightweight video and storyboard detection for legacy beatmap text.

use std::io::{self, BufRead, BufReader, Cursor, Read};

/// The event declarations identified by Ez2Lazer's beatmap tag parser.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EzBeatmapTagSummary {
    pub has_video: bool,
    pub has_storyboard: bool,
}

impl EzBeatmapTagSummary {
    pub const EMPTY: Self = Self {
        has_video: false,
        has_storyboard: false,
    };

    pub fn has_any_tag(self) -> bool {
        self.has_video || self.has_storyboard
    }

    fn both_tags(self) -> bool {
        self.has_video && self.has_storyboard
    }
}

pub struct EzBeatmapTagParser;

impl EzBeatmapTagParser {
    /// Scans the shared storyboard first, then the beatmap if either tag is still missing.
    /// Readers must be positioned at the start of their respective files.
    pub fn parse(
        beatmap: Option<&mut dyn BufRead>,
        shared_storyboard: Option<&mut dyn BufRead>,
    ) -> io::Result<EzBeatmapTagSummary> {
        let mut summary = EzBeatmapTagSummary::EMPTY;
        if let Some(reader) = shared_storyboard {
            scan_events(reader, &mut summary)?;
        }
        if !summary.both_tags() {
            if let Some(reader) = beatmap {
                scan_events(reader, &mut summary)?;
            }
        }
        Ok(summary)
    }

    /// Scans raw bytes from a beatmap and its optional shared storyboard.
    pub fn parse_bytes(
        beatmap: Option<&[u8]>,
        shared_storyboard: Option<&[u8]>,
    ) -> EzBeatmapTagSummary {
        let mut beatmap = beatmap.map(Cursor::new);
        let mut shared_storyboard = shared_storyboard.map(Cursor::new);
        Self::parse(
            beatmap.as_mut().map(|reader| reader as &mut dyn BufRead),
            shared_storyboard
                .as_mut()
                .map(|reader| reader as &mut dyn BufRead),
        )
        .expect("byte slice readers are infallible")
    }

    /// Computes the shared storyboard filename from beatmap set metadata.
    pub fn main_storyboard_filename(
        artist: &str,
        title: &str,
        audio_file: &str,
        author_username: &str,
    ) -> String {
        let mut name = if artist.is_empty() {
            let filename = audio_file.rsplit(['/', '\\']).next().unwrap_or_default();
            filename
                .rsplit_once('.')
                .map_or(filename, |(stem, _)| stem)
                .to_owned()
        } else {
            format!("{artist} - {title}")
        };
        if !author_username.is_empty() {
            name.push_str(" (");
            name.push_str(author_username);
            name.push(')');
        }
        name.push_str(".osb");
        name.retain(|c| {
            c >= '\u{20}' && !matches!(c, '"' | '<' | '>' | '|' | ':' | '*' | '?' | '\\' | '/')
        });
        name
    }
}

fn scan_events(reader: &mut dyn BufRead, summary: &mut EzBeatmapTagSummary) -> io::Result<()> {
    let mut prefix = Vec::with_capacity(4);
    (&mut *reader).take(4).read_to_end(&mut prefix)?;
    let mut in_events = false;

    let (offset, encoding) = if prefix.starts_with(&[0xff, 0xfe, 0, 0]) {
        (4, Some((4, true)))
    } else if prefix.starts_with(&[0, 0, 0xfe, 0xff]) {
        (4, Some((4, false)))
    } else if prefix.starts_with(&[0xff, 0xfe]) {
        (2, Some((2, true)))
    } else if prefix.starts_with(&[0xfe, 0xff]) {
        (2, Some((2, false)))
    } else {
        (
            usize::from(prefix.starts_with(&[0xef, 0xbb, 0xbf])) * 3,
            None,
        )
    };

    if let Some((width, little_endian)) = encoding {
        let mut data = prefix.split_off(offset);
        reader.read_to_end(&mut data)?;
        let mut decoded = if width == 2 {
            let units: Vec<_> = data
                .chunks_exact(2)
                .map(|unit| {
                    let pair = [unit[0], unit[1]];
                    if little_endian {
                        u16::from_le_bytes(pair)
                    } else {
                        u16::from_be_bytes(pair)
                    }
                })
                .collect();
            String::from_utf16_lossy(&units)
        } else {
            data.chunks_exact(4)
                .map(|unit| {
                    let word = [unit[0], unit[1], unit[2], unit[3]];
                    let value = if little_endian {
                        u32::from_le_bytes(word)
                    } else {
                        u32::from_be_bytes(word)
                    };
                    char::from_u32(value).unwrap_or(char::REPLACEMENT_CHARACTER)
                })
                .collect()
        };
        if data.len() % width != 0 {
            decoded.push(char::REPLACEMENT_CHARACTER);
        }
        for line in decoded.split(['\r', '\n']) {
            scan_line(line, &mut in_events, summary);
            if summary.both_tags() {
                break;
            }
        }
        return Ok(());
    }

    let mut reader = BufReader::new(Cursor::new(&prefix[offset..]).chain(reader));
    let mut line = Vec::new();
    while !summary.both_tags() {
        let bytes = reader.fill_buf()?;
        if bytes.is_empty() {
            scan_line(&String::from_utf8_lossy(&line), &mut in_events, summary);
            break;
        }
        let end = bytes.iter().position(|byte| matches!(byte, b'\n' | b'\r'));
        if let Some(end) = end {
            line.extend_from_slice(&bytes[..end]);
            scan_line(&String::from_utf8_lossy(&line), &mut in_events, summary);
            reader.consume(end + 1);
            line.clear();
        } else {
            line.extend_from_slice(bytes);
            let consumed = bytes.len();
            reader.consume(consumed);
        }
    }
    Ok(())
}

fn scan_line(raw: &str, in_events: &mut bool, summary: &mut EzBeatmapTagSummary) {
    let line = raw.trim();
    if line.starts_with('[') {
        *in_events = line.eq_ignore_ascii_case("[Events]");
        return;
    }
    if !*in_events || line.starts_with('_') || line.starts_with("//") {
        return;
    }
    let event = line
        .split_once("//")
        .map_or(line, |(declaration, _)| declaration)
        .split(',')
        .next()
        .unwrap_or_default()
        .trim();
    if event == "1" || event.eq_ignore_ascii_case("Video") {
        summary.has_video = true;
    } else if matches!(event, "4" | "6")
        || event.eq_ignore_ascii_case("Sprite")
        || event.eq_ignore_ascii_case("Animation")
    {
        summary.has_storyboard = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_video_only_event_excerpt_from_lazer_fixture() {
        // Event declarations from osu.Game.Tests/Resources/storyboard_only_video.osu.
        let beatmap = b"osu file format v14\n\n[Events]\n//Background and Video events\n\
                        0,0,\"BG.jpg\",0,0\nVideo,0,\"video.avi\"\n\
                        [TimingPoints]\n1674,333.333333333333,4,2,1,70,1,0\n";
        assert_eq!(
            EzBeatmapTagParser::parse_bytes(Some(beatmap), None),
            EzBeatmapTagSummary {
                has_video: true,
                has_storyboard: false,
            }
        );
    }

    #[test]
    fn combines_separate_streams_and_recognises_numeric_types() {
        let shared = b"\xef\xbb\xbf[EVENTS]\r\n4,Foreground,Centre,\"image.png\",320,240\r\n";
        let beatmap = b"[General]\r4,Outside\r[events]\r1,0,\"video.mp4\" // inline\r";
        assert_eq!(
            EzBeatmapTagParser::parse_bytes(Some(beatmap), Some(shared)),
            EzBeatmapTagSummary {
                has_video: true,
                has_storyboard: true,
            }
        );
        for animation in [b"[Events]\n6,".as_slice(), b"[Events]\nAnimation,"] {
            assert!(EzBeatmapTagParser::parse_bytes(Some(animation), None).has_storyboard);
        }
    }

    #[test]
    fn ignores_comments_commands_and_other_sections() {
        let beatmap = b"Video,0,x\n[Events]\n//Video,0,x\n_Sprite,Foreground\n\
                        _1,0,x\n+1,0,x\n04,Foreground\n0,0,x\n5,0,y\n\
                        [TimingPoints]\nAnimation,Foreground\n";
        let summary = EzBeatmapTagParser::parse_bytes(Some(beatmap), None);
        assert_eq!(summary, EzBeatmapTagSummary::EMPTY);
        assert!(!summary.has_any_tag());
    }

    #[test]
    fn stream_reader_handles_utf16_and_utf32_boms() {
        let events = "[Events]\r\n1,0,x\nSprite,Foreground";
        let mut wide = vec![0xfe, 0xff];
        wide.extend(events.encode_utf16().flat_map(u16::to_be_bytes));
        let mut wider = vec![0xff, 0xfe, 0, 0];
        wider.extend(events.chars().flat_map(|c| u32::from(c).to_le_bytes()));
        for encoded in [wide, wider] {
            let summary = EzBeatmapTagParser::parse_bytes(Some(&encoded), None);
            assert!(summary.has_video && summary.has_storyboard);
        }
    }

    #[test]
    fn matches_lazer_shared_storyboard_filename_rules() {
        assert_eq!(
            EzBeatmapTagParser::main_storyboard_filename("A/B:", "Title?", "other.mp3", "M|a*p"),
            "AB - Title (Map).osb"
        );
        assert_eq!(
            EzBeatmapTagParser::main_storyboard_filename("", "", r"folder\audio.mix.mp3", ""),
            "audio.mix.osb"
        );
    }
}
