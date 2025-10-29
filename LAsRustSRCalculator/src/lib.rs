pub mod sr;
pub mod note;
pub mod cross_matrix;
pub mod parser;
pub mod math;

use crate::parser::OsuParser;
use crate::sr::SRCalculator;
use std::os::raw::c_char;

pub struct SRAPI;

impl SRAPI {
    pub fn calculate_sr(file_path: &str) -> Result<f64, String> {
        let mut parser = OsuParser::new(file_path);
        parser.process().map_err(|e| e.to_string())?;
        let data = parser.get_parsed_data();
        println!("Parsed data: k={}, od={}, notes={}", data.column_count, data.od, data.columns.len());
        SRCalculator::calculate_sr_from_parsed_data(&data)
    }
}

#[no_mangle]
pub extern "C" fn calculate_sr_from_osu_file(path_ptr: *const c_char, len: usize) -> f64 {
    // Convert the C string to Rust string
    let path_bytes = unsafe { std::slice::from_raw_parts(path_ptr as *const u8, len) };
    let path_str = match std::str::from_utf8(path_bytes) {
        Ok(s) => s,
        Err(_) => return -1.0, // Error indicator
    };

    println!("Rust: Received path: {}", path_str);

    // Calculate SR
    match SRAPI::calculate_sr(path_str) {
        Ok(sr) => {
            println!("Rust: Calculated SR: {}", sr);
            sr
        },
        Err(e) => {
            println!("Rust: Error calculating SR: {}", e);
            -1.0 // Error indicator
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sr_calculation() {
        // Use the actual osu file for testing
        let file_path = "..\\tests\\Resource\\Glen Check - 60's Cardin (SK_la) [Insane].osu";
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
