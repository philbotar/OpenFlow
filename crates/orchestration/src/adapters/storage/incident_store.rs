use crate::adapters::storage::json_file_store::{atomic_write, OPENFLOW_DATA_DIR_SLUG};
use crate::incident::{IncidentListOptions, IncidentRecord, IncidentStore};
use std::collections::HashSet;
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

    fn prune_to_max(&self, max: u32) -> io::Result<usize> {
        let mut records = self.read_all()?;
        let max = max as usize;
        if records.len() <= max {
            return Ok(0);
        }
        let to_remove = records.len() - max;
        let mut remove_ids = Vec::with_capacity(to_remove);

        let mut resolved_indices: Vec<usize> = records
            .iter()
            .enumerate()
            .filter(|(_, record)| record.resolved)
            .map(|(index, _)| index)
            .collect();
        resolved_indices.sort_by_key(|&index| records[index].created_at_ms);
        for index in resolved_indices {
            if remove_ids.len() >= to_remove {
                break;
            }
            remove_ids.push(records[index].id.clone());
        }

        if remove_ids.len() < to_remove {
            let mut unresolved_indices: Vec<usize> = records
                .iter()
                .enumerate()
                .filter(|(_, record)| !record.resolved)
                .map(|(index, _)| index)
                .collect();
            unresolved_indices.sort_by_key(|&index| records[index].created_at_ms);
            for index in unresolved_indices {
                if remove_ids.len() >= to_remove {
                    break;
                }
                remove_ids.push(records[index].id.clone());
            }
        }

        let remove_set: HashSet<_> = remove_ids.iter().cloned().collect();
        records.retain(|record| !remove_set.contains(&record.id));
        self.write_all(&records)?;
        Ok(to_remove)
    }
}
