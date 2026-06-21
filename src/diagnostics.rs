use crate::model::{LogRecord, ParseIssue};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Diagnostics {
    pub parse_issue_count: usize,
    pub issues_by_source: BTreeMap<String, usize>,
    pub issues_by_reason: BTreeMap<String, usize>,
    pub missing_timestamp_count: usize,
    pub missing_level_count: usize,
    pub quality_score: f64,
    pub field_catalog: Vec<FieldSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FieldSummary {
    pub name: String,
    pub present_count: usize,
    pub missing_count: usize,
    pub unique_value_count: usize,
    pub sample_values: Vec<String>,
}

pub fn analyze_dataset(
    records: &[LogRecord],
    issues: &[ParseIssue],
    field_limit: usize,
) -> Diagnostics {
    let missing_timestamp_count = records
        .iter()
        .filter(|record| record.timestamp.is_none())
        .count();
    let missing_level_count = records
        .iter()
        .filter(|record| record.level.is_none())
        .count();

    Diagnostics {
        parse_issue_count: issues.len(),
        issues_by_source: count_issue_sources(issues),
        issues_by_reason: count_issue_reasons(issues),
        missing_timestamp_count,
        missing_level_count,
        quality_score: quality_score(records, issues),
        field_catalog: field_catalog(records, field_limit),
    }
}

pub fn count_issue_sources(issues: &[ParseIssue]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for issue in issues {
        *counts.entry(issue.source.clone()).or_default() += 1;
    }
    counts
}

pub fn count_issue_reasons(issues: &[ParseIssue]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for issue in issues {
        let reason = normalize_reason(&issue.reason);
        *counts.entry(reason).or_default() += 1;
    }
    counts
}

pub fn field_catalog(records: &[LogRecord], limit: usize) -> Vec<FieldSummary> {
    let mut names = BTreeSet::new();
    for record in records {
        names.extend(record.fields.keys().cloned());
    }

    let mut summaries = names
        .into_iter()
        .map(|name| summarize_field(records, &name))
        .collect::<Vec<_>>();

    summaries.sort_by(|left, right| {
        right
            .present_count
            .cmp(&left.present_count)
            .then_with(|| left.name.cmp(&right.name))
    });
    summaries.truncate(limit);
    summaries
}

fn summarize_field(records: &[LogRecord], name: &str) -> FieldSummary {
    let mut present_count = 0;
    let mut values = BTreeSet::new();

    for record in records {
        if let Some(value) = record.fields.get(name) {
            present_count += 1;
            if !value.is_empty() {
                values.insert(value.clone());
            }
        }
    }

    FieldSummary {
        name: name.to_owned(),
        present_count,
        missing_count: records.len().saturating_sub(present_count),
        unique_value_count: values.len(),
        sample_values: values.into_iter().take(5).collect(),
    }
}

fn normalize_reason(reason: &str) -> String {
    reason
        .split(':')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown issue")
        .to_owned()
}

fn quality_score(records: &[LogRecord], issues: &[ParseIssue]) -> f64 {
    if records.is_empty() && issues.is_empty() {
        return 100.0;
    }

    let expected_record_fields = records.len() * 2;
    let present_timestamps = records
        .iter()
        .filter(|record| record.timestamp.is_some())
        .count();
    let present_levels = records
        .iter()
        .filter(|record| record.level.is_some())
        .count();
    let successful_checks = present_timestamps + present_levels;
    let total_checks = expected_record_fields + issues.len();

    if total_checks == 0 {
        100.0
    } else {
        (successful_checks as f64 / total_checks as f64 * 100.0 * 100.0).round() / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LogLevel, LogRecord, ParseIssue};

    #[test]
    fn builds_field_catalog_with_missing_counts() {
        let mut first = LogRecord::new("a.log", "one");
        first.fields.insert("user".to_owned(), "alice".to_owned());
        let mut second = LogRecord::new("b.log", "two");
        second.fields.insert("user".to_owned(), "bob".to_owned());
        second.fields.insert("service".to_owned(), "api".to_owned());

        let catalog = field_catalog(&[first, second], 10);

        let user = catalog
            .iter()
            .find(|field| field.name == "user")
            .expect("user field should be summarized");
        assert_eq!(user.present_count, 2);
        assert_eq!(user.missing_count, 0);
        assert_eq!(user.unique_value_count, 2);
    }

    #[test]
    fn summarizes_issue_reasons_by_prefix() {
        let issues = vec![
            ParseIssue::new("a.jsonl", 1, "invalid JSON: first", "{bad}"),
            ParseIssue::new("a.jsonl", 2, "invalid JSON: second", "{bad}"),
        ];

        let reasons = count_issue_reasons(&issues);

        assert_eq!(reasons["invalid JSON"], 2);
    }

    #[test]
    fn computes_quality_score_from_records_and_issues() {
        let mut first = LogRecord::new("a.log", "one");
        first.level = Some(LogLevel::Info);
        let second = LogRecord::new("a.log", "two");
        let issues = vec![ParseIssue::new("a.log", 3, "bad line", "???")];

        let diagnostics = analyze_dataset(&[first, second], &issues, 10);

        assert_eq!(diagnostics.missing_level_count, 1);
        assert_eq!(diagnostics.parse_issue_count, 1);
        assert!(diagnostics.quality_score < 100.0);
    }
}
