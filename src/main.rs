use std::env;
use std::path::Path;

use csv::Reader;

fn main() {
    // skip program name
    let args: Vec<String> = env::args().skip(1).collect();

    // flag
    let show_types = args.iter().any(|a| a == "--types");
    let show_summary = args.iter().any(|a| a == "--summary");

    // positional arguments
    let positional: Vec<&String> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .collect();

    if positional.is_empty() {
        eprintln!("Usage: datainspect [--types] <file>");
        std::process::exit(1);
    }

    let filename = positional.last().unwrap();

    let path = Path::new(filename);
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    match extension {
        "csv" => inspect_csv(filename, show_types, show_summary),
        "json" => inspect_json(filename, show_types),
        _ => {
            eprintln!("Unsupported file type: {}", extension);
            std::process::exit(1);
        }
    }
}

// show summary stats

#[derive(Default)]
struct NumericStats {
    count: usize, 
    sum: f64,
    min: f64,
    max: f64,
}

impl NumericStats {
    fn update(&mut self, value: f64) {
        if self.count == 0 {
            self.min = value;
            self.max = value;
        } else {
            self.min = self.min.min(value);
            self.max = self.max.max(value);
        }
        self.sum += value;
        self.count += 1;
    }

    fn mean(&self) -> f64 {
        self.sum / self.count as f64
    }
}

fn inspect_csv(filename: &str, show_types: bool, show_summary: bool) {
    let mut reader = Reader::from_path(filename)
        .expect("Failed to open CSV file");

    let headers = reader
        .headers()
        .expect("Failed to read CSV headers")
        .clone();

    let col_count = headers.len();

    let mut row_count = 0;
    let mut inferred: Vec<Option<&'static str>> = vec![None; col_count];

    let mut numeric_stats: Vec<NumericStats> =
        (0..col_count).map(|_| NumericStats::default()).collect();

    let mut non_null_counts: Vec<usize> = vec![0; col_count];

    for result in reader.records() {
        let record = result.expect("Failed to read record");
        row_count += 1;

        for (i, value) in record.iter().enumerate() {
            if value.is_empty() {
                continue;
            }

            non_null_counts[i] += 1;

            if inferred[i].is_none() {
                inferred[i] = Some(infer_type(value));
            }

            if show_summary {
                if let Ok(v) = value.parse::<f64>() {
                    numeric_stats[i].update(v);
                }
            }
        }
    }

    println!("File type: CSV");
    println!("Rows: {}", row_count);
    println!("Columns:");
    for header in headers.iter() {
        println!("  - {}", header);
    }

    if show_types {
        println!("Inferred types:");
        for (header, dtype) in headers.iter().zip(inferred.iter()) {
            println!("  - {}: {}", header, dtype.unwrap_or("unknown"));
        }
    }

    if show_summary {
        println!("Summary:");
        for i in 0..col_count {
            let name = &headers[i];
            let dtype = inferred[i].unwrap_or("unknown");

            if dtype == "integer" || dtype == "float" {
                let stats = &numeric_stats[i];
                if stats.count > 0 {
                    println!(
                        "  - {} ({}): count={} min={} max={} mean={}",
                        name,
                        dtype,
                        stats.count,
                        stats.min,
                        stats.max,
                        stats.mean()
                    );
                }
            } else {
                println!(
                    "  - {} ({}): count={}",
                    name,
                    dtype,
                    non_null_counts[i]
                );
            }
        }
    }
}


fn inspect_json(filename: &str, show_types: bool) {
    let contents = std::fs::read_to_string(filename)
        .expect("Failed to read JSON file");

    let json: serde_json::Value = serde_json::from_str(&contents)
        .expect("Invalid JSON");

    // Normalize JSON file
    let records: Vec<serde_json::Map<String, serde_json::Value>> =
        match json {
            serde_json::Value::Array(arr) => {
                // If array of objects → many records
                let objects: Vec<_> = arr.into_iter()
                    .filter_map(|v| v.as_object().cloned())
                    .collect();

                if objects.is_empty() {
                    // Array of primitives → single record
                    vec![serde_json::Map::new()]
                } else {
                    objects
                }
            }
            serde_json::Value::Object(obj) => {
                // Single object → single record
                vec![obj]
            }
            _ => {
                eprintln!("Unsupported JSON structure");
                std::process::exit(1);
            }
        };

    println!("File type: JSON");
    println!("Records: {}", records.len());

    if let Some(first) = records.first() {
        println!("Fields:");
        for (key, value) in first.iter() {
            if show_types {
                let dtype = match value {
                    serde_json::Value::Number(n) if n.is_i64() => "integer",
                    serde_json::Value::Number(_) => "float",
                    serde_json::Value::Bool(_) => "boolean",
                    serde_json::Value::String(_) => "string",
                    serde_json::Value::Null => "null",
                    serde_json::Value::Array(_) => "array",
                    serde_json::Value::Object(_) => "object",
                };
                println!("  - {}: {}", key, dtype);
            } else {
                println!("  - {}", key);
            }
        }
    }
}


#[allow(dead_code)]
fn infer_type(value: &str) -> &'static str {
    if value.parse::<i64>().is_ok() {
        "integer"
    } else if value.parse::<f64>().is_ok() {
        "float"
    } else if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
        "boolean"
    } else {
        "string"
    }
}

