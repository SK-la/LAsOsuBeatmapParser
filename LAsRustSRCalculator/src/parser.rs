use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug)]
pub struct ParsedData {
    pub column_count: i32,
    pub columns: Vec<i32>,
    pub note_starts: Vec<i32>,
    pub note_ends: Vec<i32>,
    pub note_types: Vec<i32>,
    pub od: f64,
}

pub struct OsuParser {
    file_path: String,
    od: f64,
    column_count: i32,
    columns: Vec<i32>,
    note_starts: Vec<i32>,
    note_ends: Vec<i32>,
    note_types: Vec<i32>,
}

impl OsuParser {
    pub fn new(file_path: &str) -> Self {
        OsuParser {
            file_path: file_path.to_string(),
            od: -1.0,
            column_count: -1,
            columns: Vec::new(),
            note_starts: Vec::new(),
            note_ends: Vec::new(),
            note_types: Vec::new(),
        }
    }

    pub fn process(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open(&self.file_path)?;
        let reader = BufReader::new(file);

        let mut lines = reader.lines();
        let mut in_hit_objects = false;
        let mut line_count = 0;

        while let Some(line) = lines.next() {
            let line = line?;
            line_count += 1;
            
            // Check for section headers
            if line.starts_with("[HitObjects]") {
                in_hit_objects = true;
                continue;
            }
            
            if in_hit_objects {
                if line.trim().is_empty() {
                    continue;
                }
                if line.starts_with('[') {
                    break; // Next section
                }
                self.parse_hit_object(&line, self.column_count);
            } else {
                // Read metadata and difficulty settings
                self.read_metadata(&line);
                let temp_cc = self.read_column_count(&line);
                if temp_cc != -1 {
                    self.column_count = temp_cc;
                }
                let temp_od = self.read_overall_difficulty(&line);
                if temp_od != -1.0 {
                    self.od = temp_od;
                }
            }
        }
        
        println!("Total lines processed: {}, Hit objects parsed: {}", line_count, self.columns.len());
        Ok(())
    }

    fn read_metadata(&self, _line: &str) {
        // Skip metadata for now
    }

    fn read_overall_difficulty(&self, line: &str) -> f64 {
        if line.starts_with("OverallDifficulty:") {
            let temp: Vec<&str> = line.split(':').collect();
            if temp.len() > 1 {
                return temp[1].trim().parse().unwrap_or(-1.0);
            }
        }
        -1.0
    }

    fn read_column_count(&self, line: &str) -> i32 {
        if line.starts_with("CircleSize:") {
            let temp: Vec<&str> = line.split(':').collect();
            if temp.len() > 1 {
                let cs: i32 = temp[1].trim().parse().unwrap_or(0);
                return if cs == 0 { 10 } else { cs };
            }
        }
        -1
    }



    fn parse_hit_object(&mut self, object_line: &str, column_count: i32) {
        let params: Vec<&str> = object_line.split(',').collect();
        if params.len() < 6 {
            return;
        }

        let x: f64 = params[0].parse().unwrap_or(0.0);
        let offset = 256.0 / column_count as f64;
        let ratio = 512.0 / column_count as f64;
        let column = ((x - offset) / ratio).round() as i32;
        self.columns.push(column);

        let note_start: i32 = params[2].parse().unwrap_or(0);
        self.note_starts.push(note_start);

        let note_type: i32 = params[3].parse().unwrap_or(0);
        self.note_types.push(note_type);

        let last_param_chunk: Vec<&str> = params[5].split(':').collect();
        let note_end: i32 = last_param_chunk[0].parse().unwrap_or(0);
        self.note_ends.push(note_end);
    }

    pub fn get_parsed_data(&self) -> ParsedData {
        ParsedData {
            column_count: self.column_count,
            columns: self.columns.clone(),
            note_starts: self.note_starts.clone(),
            note_ends: self.note_ends.clone(),
            note_types: self.note_types.clone(),
            od: self.od,
        }
    }
}