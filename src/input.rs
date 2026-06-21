use crate::error::{LoglensError, Result};
use crate::model::{InputFormat, LogDataset};
use crate::parser::{detect_format, parse_records};
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub fn load_dataset(
    path: &Path,
    requested_format: InputFormat,
    strict: bool,
) -> Result<LogDataset> {
    let files = collect_files(path)?;
    let mut dataset = LogDataset::empty();

    for file in files {
        let format = detect_format(&file, requested_format)?;
        let source = file.to_string_lossy().into_owned();
        let reader = BufReader::new(fs::File::open(&file)?);
        let parsed = parse_records(reader, &source, format)?;
        if strict && let Some(issue) = parsed.issues.first() {
            return Err(LoglensError::StrictParseIssue {
                file: issue.source.clone(),
                line: issue.line,
                reason: issue.reason.clone(),
            });
        }
        dataset.extend(parsed);
    }

    Ok(dataset)
}

fn collect_files(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    if !path.is_dir() {
        return Err(LoglensError::NoInputFiles {
            path: path.to_path_buf(),
        });
    }

    let mut files = Vec::new();
    visit_dir(path, &mut files)?;
    files.sort();

    if files.is_empty() {
        return Err(LoglensError::NoInputFiles {
            path: path.to_path_buf(),
        });
    }

    Ok(files)
}

fn visit_dir(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit_dir(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn non_strict_mode_keeps_good_jsonl_records_and_issues() {
        let path = temp_file("mixed.jsonl");
        fs::write(
            &path,
            "{\"level\":\"info\",\"message\":\"ok\"}\n{bad json}\n{\"level\":\"error\",\"message\":\"bad\"}",
        )
        .expect("temp file should be writable");

        let dataset =
            load_dataset(&path, InputFormat::Jsonl, false).expect("non-strict load should work");

        assert_eq!(dataset.records.len(), 2);
        assert_eq!(dataset.issues.len(), 1);

        cleanup_temp_file(&path);
    }

    #[test]
    fn strict_mode_fails_on_first_parse_issue() {
        let path = temp_file("strict.jsonl");
        fs::write(&path, "{\"level\":\"info\",\"message\":\"ok\"}\n{bad json}")
            .expect("temp file should be writable");

        let err = load_dataset(&path, InputFormat::Jsonl, true)
            .expect_err("strict mode should reject bad lines");

        assert!(err.to_string().contains("strict parsing failed"));

        cleanup_temp_file(&path);
    }

    #[test]
    fn directory_input_collects_files_recursively() {
        let root = temp_dir("loglens-dir");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).expect("temp dir should be writable");
        fs::write(root.join("one.log"), "INFO root\n").expect("root file should be writable");
        fs::write(nested.join("two.log"), "ERROR nested\n")
            .expect("nested file should be writable");

        let dataset =
            load_dataset(&root, InputFormat::PlainLog, false).expect("directory should load");

        assert_eq!(dataset.records.len(), 2);
        assert!(
            dataset
                .records
                .iter()
                .any(|record| record.source.contains("one.log"))
        );
        assert!(
            dataset
                .records
                .iter()
                .any(|record| record.source.contains("two.log"))
        );

        let _ = fs::remove_dir_all(root);
    }

    fn temp_file(name: &str) -> PathBuf {
        temp_dir("loglens-file").join(name)
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("temp dir should be creatable");
        path
    }

    fn cleanup_temp_file(path: &Path) {
        let _ = fs::remove_file(path);
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}
