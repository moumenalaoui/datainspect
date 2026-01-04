use std::env;
use std::path::Path;
use std::collections::HashSet;
use csv::Reader;

fn print_help() {
    println!(
        "datainspect - CLI Data Inspection tool

USAGE: 
    datainspect <file> [options]

OPTIONS:
  --summary        Show per-column statistical summary
  --types          Show inferred column types
  --help           Show this help message

SUPPORTED FILES:
  .csv
  .json

EXAMPLES:
  datainspect data.csv --summary
  datainspect data.csv --types
  datainspect data.json --types"
    );
}

fn main() {
    // skip program name
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help") {
        print_help();
        return;
    }

    // flag
    let show_types = args.iter().any(|a| a == "--types");
    let show_summary = args.iter().any(|a| a == "--summary");
    let show_diagnose = args.iter().any(|a| a == "--diagnose");

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
        "csv" => inspect_csv(filename, show_types, show_summary, show_diagnose),
        "json" => inspect_json(filename, show_types),
        _ => {
            eprintln!("Unsupported file type: {}", extension);
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ColumnType {
    Numeric,
    Categorical,
}

#[derive(Debug)]
struct ColumnStats {
    name: String,
    kind: ColumnType,

    total: usize,
    missing: usize,

    // num stats 
    min: Option<f64>,
    max: Option<f64>, 
    mean: f64,
    m2: f64, 

    // categorical stats
    uniques: HashSet<String>,

    //diagnostics helpers
    numeric_parse_failures: usize,

    //outliers 
    outlier_count: usize,
}

// Welford's ALGORITHM -> streaming mean + variance

impl ColumnStats {
    fn new(name: &str, kind: ColumnType) -> Self {
        Self {
            name: name.to_string(),
            kind,
            total: 0,
            missing: 0,
            min: None,
            max: None,
            mean: 0.0,
            m2: 0.0,
            uniques: HashSet::new(),
            numeric_parse_failures: 0,
            outlier_count: 0,
        }
    }

    fn update(&mut self, value: &str) {
        self.total += 1;

        if value.is_empty() {
            self.missing += 1;
            return;
        }

        match self.kind {
            ColumnType::Numeric => {
                if let Ok(x) = value.parse::<f64>() {
                    let previous_count = self.total - self.missing - 1;
                    
                    if previous_count >= 2 {
                        let prev_stddev = (self.m2 / (previous_count as f64 - 1.0)).sqrt();
                        if prev_stddev > 0.0 {
                            let z = (x - self.mean).abs() / prev_stddev;
                            if z >= 5.0 {
                                self.outlier_count += 1;
                            }
                        }
                    }
                    
                    // update stats with current value
                    let count = previous_count + 1;
                    let delta = x - self.mean;
                    self.mean += delta / count as f64;
                    self.m2 += delta * (x - self.mean);

                    self.min = Some(self.min.map_or(x, |m| m.min(x)));
                    self.max = Some(self.max.map_or(x, |m| m.max(x)));
                } else {
                    self.numeric_parse_failures += 1;
                }
            }
            ColumnType::Categorical => {
                self.uniques.insert(value.to_string());
            }
        }
    }

    fn stddev(&self) -> Option<f64> {
        let count = self.total - self.missing;
        if count > 1 {
            Some((self.m2 / (count as f64 - 1.0)).sqrt())
        } else {
            None
        }
    }
}

fn inspect_csv(filename: &str, show_types: bool, show_summary: bool, show_diagnose: bool) {
    let mut reader = Reader::from_path(filename)
        .expect("Failed to open CSV file");

    let headers = reader
        .headers()
        .expect("Failed to read CSV headers")
        .clone();

    let col_count = headers.len();

    let mut row_count = 0;
    let mut inferred: Vec<Option<&'static str>> = vec![None; col_count];
    let mut column_stats: Vec<Option<ColumnStats>> = (0..col_count).map(|_| None).collect();

    for result in reader.records() {
        let record = result.expect("Failed to read record");
        row_count += 1;

        for (i, value) in record.iter().enumerate() {
            if inferred[i].is_none() {
                let kind = if value.is_empty() {
                    // temporarily unknown, treat as categorical for now
                    ColumnType::Categorical
                } else {
                    match infer_type(value) {
                        "integer" | "float" => ColumnType::Numeric,
                        _ => ColumnType::Categorical,
                    }
                };

                inferred[i] = Some(match kind {
                    ColumnType::Numeric => "numeric",
                    ColumnType::Categorical => "categorical",
                });

                column_stats[i] = Some(ColumnStats::new(&headers[i], kind));
            }

            if let Some(stats) = &mut column_stats[i] {
                if stats.kind == ColumnType::Categorical && !value.is_empty() {
                    if matches!(infer_type(value), "integer" | "float") {
                        // Upgrade categorical → numeric
                        stats.kind = ColumnType::Numeric;
                        stats.uniques.clear(); // no longer needed
                        inferred[i] = Some("numeric");
                    }
                }

                stats.update(value);
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

        for stats_opt in column_stats.iter().flatten() {
            match stats_opt.kind {
                ColumnType::Numeric => {
                    let count = stats_opt.total - stats_opt.missing;

                    if count > 0 {
                        println!(
                            "  - {} (numeric): count={} missing={} min={} max={} mean={} stddev={}",
                            stats_opt.name,
                            count,
                            stats_opt.missing,
                            stats_opt.min.unwrap(),
                            stats_opt.max.unwrap(),
                            stats_opt.mean,
                            stats_opt.stddev().unwrap_or(0.0)
                        );
                    }
                }
                ColumnType::Categorical => {
                    println!(
                        "  - {} (categorical): count={} missing={} unique={}",
                        stats_opt.name,
                        stats_opt.total - stats_opt.missing,
                        stats_opt.missing,
                        stats_opt.uniques.len()
                    );
                }
            }
        }
    }

    if show_diagnose {
        println!();
        println!("Data Quality Report");
        println!("--------------------");
        println!();

        for stats_opt in column_stats.iter().flatten() {
            println!("{} ({:?})", stats_opt.name, stats_opt.kind);
            diagnose_column(stats_opt, row_count);
            println!();
        }
    }
}

fn diagnose_column(stats: &ColumnStats, total_rows: usize) {
    let mut warnings = Vec::new();

    let missing_ratio = stats.missing as f64 / total_rows as f64;

    // missing severity
    if missing_ratio > 0.05 {
        warnings.push(format!(
            "! missing values: {}%",
            (missing_ratio * 100.0).round() as usize
        ));
    }

    match stats.kind {
        ColumnType::Categorical => {
            let non_missing = total_rows - stats.missing;
            if non_missing > 0 {
                let unique_ratio = stats.uniques.len() as f64 / non_missing as f64;
                if unique_ratio > 0.95 {
                    warnings.push(format!(
                        "! high cardinality: {:.1}% unique (likely identifier)",
                        unique_ratio * 100.0
                    ));
                }
            }
        }

        ColumnType::Numeric => {
            // near-constant numeric
            if let (Some(min), Some(max)) = (stats.min, stats.max) {
                if (max - min).abs() < 1e-12 {
                    warnings.push("! near-constant numeric column".to_string());
                }
            }

            // mixed-type numeric
            if stats.numeric_parse_failures > 0 {
                warnings.push("! mixed numeric and non-numeric values".to_string());
            }
            
            // outliers 
            if stats.outlier_count > 0 {
                warnings.push(format!(
                        "! extreme outliers detected: {} values >= 5σ",
                        stats.outlier_count
                ));
            }
        }
    }

    // out
    if warnings.is_empty() {
        println!("  ok");
    } else {
        for w in warnings {
            println!("  {}", w);
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

