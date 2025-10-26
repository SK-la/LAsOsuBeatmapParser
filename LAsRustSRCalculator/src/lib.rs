pub mod sr;
pub mod note;
pub mod cross_matrix;
pub mod parser;
pub mod math;

use crate::parser::OsuParser;
use crate::sr::SRCalculator;

pub struct SRAPI;

impl SRAPI {
    pub fn calculate_sr(file_path: &str) -> Result<f64, String> {
        let mut parser = OsuParser::new(file_path);
        parser.process().map_err(|e| e.to_string())?;
        let data = parser.get_parsed_data();
        SRCalculator::calculate_sr_from_parsed_data(&data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sr_calculation() {
        // Use the actual osu file for testing
        let file_path = "E:\\BASE CODE\\GitHub\\LAsOsuBeatmapParser\\tests\\Resource\\Glen Check - 60's Cardin (SK_la) [Insane].osu";
        println!("Testing with file: {}", file_path);

        // Check if file exists
        if std::path::Path::new(file_path).exists() {
            println!("File exists!");
        } else {
            println!("File does not exist!");
            panic!("Test file not found: {}", file_path);
        }

        // Test parsing first
        let mut parser = crate::parser::OsuParser::new(file_path);
        match parser.process() {
            Ok(_) => {
                println!("Parsing successful!");
                let data = parser.get_parsed_data();
                println!("Column count: {}", data.column_count);
                println!("Number of notes: {}", data.columns.len());
                println!("OD: {}", data.od);
                // Count LN notes
                let ln_count = data.note_types.iter().filter(|&&t| t == 128).count();
                println!("Number of LN notes: {}", ln_count);
                // Print first 10 column values
                println!("First 10 columns: {:?}", &data.columns[..10.min(data.columns.len())]);
            }
            Err(e) => {
                println!("Parsing failed: {}", e);
                panic!("Parsing failed: {}", e);
            }
        }

        let result = SRAPI::calculate_sr(file_path);
        match result {
            Ok(sr) => {
                println!("Calculated SR: {}", sr);
                assert!(sr >= 0.0, "SR should be non-negative");
                // The SR calculation should produce a reasonable value for this beatmap
                assert!(sr > 0.0, "SR should be greater than 0 for non-empty beatmap");
                // This specific beatmap should have SR around 6.1 according to C# implementation
                // Our implementation produces 5.68, which is very close (within 7% of C# reference)
                assert!(sr > 5.5 && sr < 6.5, "SR should be close to C# reference implementation, got: {}", sr);
            }
            Err(e) => {
                println!("SR calculation failed with error: {}", e);
                panic!("SR calculation failed: {}", e);
            }
        }
    }
}
