//! Append-only observation segments. Never delete or overwrite previous history.
use serde_json::Value;
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

static WRITER: Mutex<()> = Mutex::new(());
pub const SEGMENT_BYTES: u64 = 64 * 1024 * 1024;

pub fn append(base: &Path, record: &Value) -> io::Result<PathBuf> {
    append_with_limit(base, record, SEGMENT_BYTES)
}

fn append_with_limit(base: &Path, record: &Value, limit: u64) -> io::Result<PathBuf> {
    let _guard = WRITER
        .lock()
        .map_err(|_| io::Error::other("Observation writer poisoned"))?;
    let parent = base
        .parent()
        .ok_or_else(|| io::Error::other("Missing observation directory"))?;
    fs::create_dir_all(parent)?;
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| io::Error::other("Invalid log name"))?;
    let prefix = format!("{stem}.part-");
    let mut part = 0_u64;
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry.file_name();
        if let Some(n) = name
            .to_str()
            .and_then(|s| s.strip_prefix(&prefix))
            .and_then(|s| s.strip_suffix(".jsonl"))
            .and_then(|s| s.parse::<u64>().ok())
        {
            part = part.max(n);
        }
    }
    let segment = |n| {
        if n == 0 {
            base.to_owned()
        } else {
            parent.join(format!("{stem}.part-{n:06}.jsonl"))
        }
    };
    let mut path = segment(part);
    let mut bytes = serde_json::to_vec(record)?;
    bytes.push(b'\n');
    if let Ok(meta) = fs::metadata(&path) {
        let mut truncated = false;
        if meta.len() > 0 {
            let mut file = fs::File::open(&path)?;
            file.seek(SeekFrom::End(-1))?;
            let mut last = [0];
            file.read_exact(&mut last)?;
            truncated = last[0] != b'\n';
        }
        if truncated || (meta.len() > 0 && meta.len().saturating_add(bytes.len() as u64) > limit) {
            part = part
                .checked_add(1)
                .ok_or_else(|| io::Error::other("Segment number exhausted"))?;
            path = segment(part);
        }
    }
    let mut output = OpenOptions::new().create(true).append(true).open(&path)?;
    output.write_all(&bytes)?;
    output.sync_data()?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn resumes_past_full_legacy_file_without_deleting_history() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("smc-live-day.jsonl");
        fs::write(&base, b"{\"old\":true}\n").unwrap();
        let saved = fs::read(&base).unwrap();
        let next = append_with_limit(&base, &json!({"new":1}), 12).unwrap();
        assert_ne!(base, next);
        assert_eq!(fs::read(&base).unwrap(), saved);
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(next).unwrap()).unwrap()["new"],
            1
        );
    }
    #[test]
    fn restart_continues_latest_segment_and_all_records_remain_readable() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("frames.jsonl");
        for i in 0..20 {
            append_with_limit(&base, &json!({"id":i}), 32).unwrap();
        }
        let mut ids = Vec::new();
        for f in fs::read_dir(dir.path()).unwrap() {
            for line in fs::read_to_string(f.unwrap().path()).unwrap().lines() {
                ids.push(
                    serde_json::from_str::<Value>(line).unwrap()["id"]
                        .as_u64()
                        .unwrap(),
                );
            }
        }
        ids.sort();
        assert_eq!(ids, (0..20).collect::<Vec<_>>());
    }
    #[test]
    fn partial_tail_is_preserved_and_new_record_starts_separate_segment() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("frames.jsonl");
        fs::write(&base, b"{\"old\":").unwrap();
        let next = append_with_limit(&base, &json!({"id":2}), 1024).unwrap();
        assert_ne!(base, next);
        assert_eq!(fs::read(&base).unwrap(), b"{\"old\":");
        assert_eq!(
            serde_json::from_str::<Value>(&fs::read_to_string(next).unwrap()).unwrap()["id"],
            2
        );
    }
    #[test]
    fn concurrent_writers_do_not_interleave_or_lose_lines() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("frames.jsonl");
        std::thread::scope(|scope| {
            for i in 0..16 {
                let base = &base;
                scope.spawn(move || {
                    append_with_limit(base, &json!({"id":i}), 64).unwrap();
                });
            }
        });
        let mut ids = Vec::new();
        for f in fs::read_dir(dir.path()).unwrap() {
            for line in fs::read_to_string(f.unwrap().path()).unwrap().lines() {
                ids.push(
                    serde_json::from_str::<Value>(line).unwrap()["id"]
                        .as_u64()
                        .unwrap(),
                );
            }
        }
        ids.sort();
        assert_eq!(ids, (0..16).collect::<Vec<_>>());
    }
    #[test]
    fn write_failure_is_returned_not_silently_discarded() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        fs::write(&blocker, "occupied").unwrap();
        assert!(append(&blocker.join("frames.jsonl"), &json!({"id":1})).is_err());
    }
}
