use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, LoglensError>;

#[derive(Debug, thiserror::Error)]
pub enum LoglensError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("JSON error at line {line}: {source}")]
    JsonLine {
        line: usize,
        #[source]
        source: serde_json::Error,
    },

    #[error("regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("invalid timestamp '{value}'")]
    InvalidTimestamp { value: String },

    #[error("invalid field filter '{value}', expected key=value")]
    InvalidFieldFilter { value: String },

    #[error("--field-name is required when --group-by field is used")]
    MissingFieldName,

    #[error("strict parsing failed in '{file}' at line {line}: {reason}")]
    StrictParseIssue {
        file: String,
        line: usize,
        reason: String,
    },

    #[error("unsupported input format for path '{path}'")]
    UnsupportedFormat { path: PathBuf },

    #[error("path does not contain readable files: '{path}'")]
    NoInputFiles { path: PathBuf },

    #[error("failed to write report to '{path}': {source}")]
    ReportWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
