use crate::adapters::storage::json_file_store::{atomic_write, OPENFLOW_DATA_DIR_SLUG};
use crate::incident::{IncidentListOptions, IncidentRecord, IncidentStore};
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileIncidentStore {
    path: PathBuf,
}

impl FileIncidentStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(OPENFLOW_DATA_DIR_SLUG)
            .join("incidents.jsonl")
    }

    fn read_all(&self) -> io::Result<Vec<IncidentRecord>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = fs::File::open(&self.path)?;
        let reader = io::BufReader::new(file);
        let mut records = Vec::new();
        for (index, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let record: IncidentRecord = serde_json::from_str(&line).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("incidents.jsonl line {}: {error}", index + 1),
                )
            })?;
            records.push(record);
        }
        Ok(records)
    }

    fn write_all(&self, records: &[IncidentRecord]) -> io::Result<()> {
        let mut body = String::new();
        for record in records {
            let line = serde_json::to_string(record).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("incident serialization failed: {error}"),
                )
            })?;
            body.push_str(&line);
            body.push('\n');
        }
        atomic_write(&self.path, &body)
    }
}

impl IncidentStore for FileIncidentStore {
    fn append(&self, record: &IncidentRecord) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let line = serde_json::to_string(record).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("incident serialization failed: {error}"),
            )
        })?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{line}")?;
        file.sync_all()?;
        Ok(())
    }

    fn list(&self, options: Option<IncidentListOptions>) -> io::Result<Vec<IncidentRecord>> {
        let options = options.unwrap_or_default();
        let mut records = self.read_all()?;
        if !options.include_resolved {
            records.retain(|record| !record.resolved);
        }
        if let Some(limit) = options.limit {
            if records.len() > limit {
                let start = records.len() - limit;
                records = records.split_off(start);
            }
        }
        Ok(records)
    }

    fn dismiss(&self, id: &str) -> io::Result<()> {
        let mut records = self.read_all()?;
        let mut found = false;
        for record in &mut records {
            if record.id == id {
                record.resolved = true;
                found = true;
                break;
            }
        }
        if !found {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("incident {id} not found"),
            ));
        }
        self.write_all(&records)
    }

    fn clear_resolved(&self) -> io::Result<usize> {
        let records = self.read_all()?;
        let before = records.len();
        let kept: Vec<_> = records.into_iter().filter(|r| !r.resolved).collect();
        let removed = before - kept.len();
        self.write_all(&kept)?;
        Ok(removed)
    }
}
