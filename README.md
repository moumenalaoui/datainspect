### datainspect

`datainspect` is a terminal-native data inspection tool for quickly understanding
the structure, statistics, and data quality of CSV files.

It is designed for data scientists and engineers who want to navigate data files without opening notebooks or plotting libraries.

datainspect focuses on data risk more than anything. 

It helps answer questions like:
- Do I have missing values that will bias my analysis?
- Is this column an identifier that could cause leakage?
- Are there useless near-constant features?
- Are there mixed data types hiding in numeric columns?
- Are there extreme outliers that will break models?

All directly in the terminal.

#### Features 

##### Summary Statistics (`--summary`)
- Row and column counts
- Type inference
- Streaming numeric statistics (min, max, mean, stddev)
- Categorical cardinality

##### Data quality diagnostics (`--diagnose`)
Flags common, high-impact data issues:
- Missing value severity
- Identifier-like categorical columns
- Near-constant numeric columns
- Mixed numeric / non-numeric values
- Extreme numeric outliers (robust to outlier masking)

Diagnostics are deterministic, streaming, and opinionated by design.

#### Usage

```bash
datainspect data.csv --summary
datainspect data.csv --diagnose
```
You can combine modes:

```bash
datainspect data.csv --summary --diagnose
```
#### Example Output 

```text
Data Quality Report
-------------------

salary (Numeric)
  ! extreme outliers detected: 1 values ≥ 5σ

user_id (Categorical)
  ! high cardinality: 99.8% unique (likely identifier)

department (Categorical)
  ok

#### Design Notes 
- All statistics are computed in a single streaming pass
- Numeric statistics use Welford’s algorithm
- Outlier detection avoids outlier masking by using pre-contamination statistics
- The tool flags risks but does not prescribe fixes

#### Installation 
Clone the repository and install the binary locally using Cargo:

```bash
cargo install --path .
```
Make sure Cargo’s bin directory is on your PATH:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```
Once installed, the datainspect command will be available system-wide.
