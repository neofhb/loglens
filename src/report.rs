use crate::diagnostics::{Diagnostics, analyze_dataset};
use crate::error::{LoglensError, Result};
use crate::model::{LogRecord, ParseIssue};
use crate::stats::Summary;
use serde::Serialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize)]
struct JsonReport<'a> {
    summary: &'a Summary,
    diagnostics: Diagnostics,
    issues: &'a [ParseIssue],
    samples: Vec<&'a LogRecord>,
}

pub fn print_summary(summary: &Summary) {
    println!("Total records: {}", summary.total);
    println!();
    print_count_table("Level counts", &summary.level_counts);
    println!();
    print_count_table("Grouped counts", &summary.grouped_counts);
    println!();
    print_count_table("Source counts", &summary.source_counts);
    println!();
    print_pair_table("Top sources", &summary.top_sources);
    if let Some(field_name) = &summary.top_field_name {
        println!();
        print_pair_table(
            &format!("Top field values: {field_name}"),
            &summary.top_field_values,
        );
    }
}

pub fn print_records(records: &[LogRecord], limit: usize) {
    for record in records.iter().take(limit) {
        let timestamp = record
            .timestamp
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "-".to_owned());
        let level = record
            .level
            .map(|value| value.to_string())
            .unwrap_or_else(|| "UNKNOWN".to_owned());
        println!(
            "{timestamp} {level:<7} {} {}",
            record.source, record.message
        );
    }

    if records.len() > limit {
        println!("... {} more records", records.len() - limit);
    }
}

pub fn print_issues(issues: &[ParseIssue], limit: usize) {
    if issues.is_empty() {
        return;
    }

    println!("Parse issues: {}", issues.len());
    for issue in issues.iter().take(limit) {
        println!(
            "{}:{} {} {}",
            issue.source, issue.line, issue.reason, issue.raw
        );
    }
    if issues.len() > limit {
        println!("... {} more parse issues", issues.len() - limit);
    }
}

pub fn write_report(
    path: &Path,
    summary: &Summary,
    records: &[LogRecord],
    issues: &[ParseIssue],
    sample_limit: usize,
) -> Result<()> {
    let content = match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("json") => to_json(summary, records, issues, sample_limit)?,
        Some("csv") => to_csv(records)?,
        _ => to_markdown(summary, records, issues, sample_limit),
    };

    fs::write(path, content).map_err(|source| LoglensError::ReportWrite {
        path: path.to_path_buf(),
        source,
    })
}

pub fn to_markdown(
    summary: &Summary,
    records: &[LogRecord],
    issues: &[ParseIssue],
    sample_limit: usize,
) -> String {
    let diagnostics = analyze_dataset(records, issues, 10);
    let mut output = String::new();
    output.push_str("# LogLens Report\n\n");
    output.push_str(&format!("- Total records: {}\n", summary.total));
    output.push_str(&format!("- Parse issues: {}\n", issues.len()));
    output.push_str(&format!(
        "- Data quality score: {:.2}%\n",
        diagnostics.quality_score
    ));
    output.push_str("\n## Level Counts\n\n");
    append_markdown_table(&mut output, &summary.level_counts);
    output.push_str("\n## Grouped Counts\n\n");
    append_markdown_table(&mut output, &summary.grouped_counts);
    output.push_str("\n## Source Counts\n\n");
    append_markdown_table(&mut output, &summary.source_counts);
    output.push_str("\n## Top Sources\n\n");
    append_pair_table(&mut output, &summary.top_sources);
    if let Some(field_name) = &summary.top_field_name {
        output.push_str(&format!(
            "\n## Top Field Values: {}\n\n",
            escape_markdown(field_name)
        ));
        append_pair_table(&mut output, &summary.top_field_values);
    }
    output.push_str("\n## Data Quality\n\n");
    output.push_str("| Metric | Value |\n");
    output.push_str("| --- | ---: |\n");
    output.push_str(&format!(
        "| Missing timestamps | {} |\n",
        diagnostics.missing_timestamp_count
    ));
    output.push_str(&format!(
        "| Missing levels | {} |\n",
        diagnostics.missing_level_count
    ));
    output.push_str(&format!(
        "| Parse issues | {} |\n",
        diagnostics.parse_issue_count
    ));
    output.push_str(&format!(
        "| Quality score | {:.2}% |\n",
        diagnostics.quality_score
    ));
    output.push_str("\n## Field Catalog\n\n");
    if diagnostics.field_catalog.is_empty() {
        output.push_str("No structured fields found.\n");
    } else {
        output.push_str("| Field | Present | Missing | Unique Values | Samples |\n");
        output.push_str("| --- | ---: | ---: | ---: | --- |\n");
        for field in &diagnostics.field_catalog {
            output.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                escape_markdown(&field.name),
                field.present_count,
                field.missing_count,
                field.unique_value_count,
                escape_markdown(&field.sample_values.join(", "))
            ));
        }
    }
    output.push_str("\n## Parse Issue Summary\n\n");
    if diagnostics.issues_by_reason.is_empty() {
        output.push_str("No parse issue categories.\n");
    } else {
        append_markdown_table(&mut output, &diagnostics.issues_by_reason);
    }
    output.push_str("\n## Parse Diagnostics\n\n");
    if issues.is_empty() {
        output.push_str("No parse issues.\n");
    } else {
        output.push_str("| Source | Line | Reason | Raw |\n");
        output.push_str("| --- | ---: | --- | --- |\n");
        for issue in issues.iter().take(sample_limit) {
            output.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                escape_markdown(&issue.source),
                issue.line,
                escape_markdown(&issue.reason),
                escape_markdown(&issue.raw)
            ));
        }
    }
    output.push_str("\n## Sample Records\n\n");
    output.push_str("| Time | Level | Source | Message |\n");
    output.push_str("| --- | --- | --- | --- |\n");
    for record in records.iter().take(sample_limit) {
        let timestamp = record
            .timestamp
            .map(|value| value.to_rfc3339())
            .unwrap_or_else(|| "-".to_owned());
        let level = record
            .level
            .map(|value| value.to_string())
            .unwrap_or_else(|| "UNKNOWN".to_owned());
        output.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            escape_markdown(&timestamp),
            escape_markdown(&level),
            escape_markdown(&record.source),
            escape_markdown(&record.message)
        ));
    }
    output
}

pub fn to_json(
    summary: &Summary,
    records: &[LogRecord],
    issues: &[ParseIssue],
    sample_limit: usize,
) -> Result<String> {
    let report = JsonReport {
        summary,
        diagnostics: analyze_dataset(records, issues, 10),
        issues,
        samples: records.iter().take(sample_limit).collect(),
    };
    serde_json::to_string_pretty(&report)
        .map_err(|source| LoglensError::JsonLine { line: 0, source })
}

pub fn to_csv(records: &[LogRecord]) -> Result<String> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record(["timestamp", "level", "source", "message", "fields"])?;
    for record in records {
        let timestamp = record
            .timestamp
            .map(|value| value.to_rfc3339())
            .unwrap_or_default();
        let level = record
            .level
            .map(|value| value.to_string())
            .unwrap_or_default();
        let fields = serde_json::to_string(&record.fields)
            .map_err(|source| LoglensError::JsonLine { line: 0, source })?;
        writer.write_record([
            timestamp.as_str(),
            level.as_str(),
            record.source.as_str(),
            record.message.as_str(),
            fields.as_str(),
        ])?;
    }

    let bytes = writer.into_inner().map_err(|error| {
        let source = error.into_error();
        LoglensError::ReportWrite {
            path: Path::new("<memory>").to_path_buf(),
            source,
        }
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn print_count_table(title: &str, counts: &std::collections::BTreeMap<String, usize>) {
    println!("{title}");
    println!("{:<32} Count", "Key");
    println!("{:-<44}", "");
    for (key, count) in counts {
        println!("{key:<32} {count}");
    }
}

fn print_pair_table(title: &str, pairs: &[(String, usize)]) {
    println!("{title}");
    println!("{:<32} Count", "Value");
    println!("{:-<44}", "");
    for (value, count) in pairs {
        println!("{value:<32} {count}");
    }
}

fn append_markdown_table(output: &mut String, counts: &std::collections::BTreeMap<String, usize>) {
    output.push_str("| Key | Count |\n");
    output.push_str("| --- | ---: |\n");
    for (key, count) in counts {
        output.push_str(&format!("| {} | {} |\n", escape_markdown(key), count));
    }
}

fn append_pair_table(output: &mut String, pairs: &[(String, usize)]) {
    output.push_str("| Value | Count |\n");
    output.push_str("| --- | ---: |\n");
    for (value, count) in pairs {
        output.push_str(&format!("| {} | {} |\n", escape_markdown(value), count));
    }
}

fn escape_markdown(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GroupBy, LogRecord};
    use crate::stats::summarize;

    #[test]
    fn renders_markdown_report() {
        let records = vec![LogRecord::new("app.log", "hello")];
        let issues = vec![ParseIssue::new("app.log", 2, "bad line", "???")];
        let summary = summarize(&records, GroupBy::Level, None, 3, None);

        let report = to_markdown(&summary, &records, &issues, 20);

        assert!(report.contains("# LogLens Report"));
        assert!(report.contains("Data Quality"));
        assert!(report.contains("Field Catalog"));
        assert!(report.contains("Parse Diagnostics"));
        assert!(report.contains("hello"));
    }

    #[test]
    fn renders_json_report() {
        let records = vec![LogRecord::new("app.log", "hello")];
        let issues = vec![ParseIssue::new("app.log", 2, "bad line", "???")];
        let summary = summarize(&records, GroupBy::Level, None, 3, None);

        let report = to_json(&summary, &records, &issues, 20).expect("json report should render");

        assert!(report.contains("\"total\""));
        assert!(report.contains("\"diagnostics\""));
        assert!(report.contains("\"issues\""));
    }

    #[test]
    fn renders_csv_report() {
        let records = vec![LogRecord::new("app.log", "hello")];

        let report = to_csv(&records).expect("csv report should render");

        assert!(report.contains("timestamp,level,source,message,fields"));
        assert!(report.contains("app.log"));
    }
}
