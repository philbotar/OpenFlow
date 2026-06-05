#![allow(
    clippy::missing_errors_doc,
    clippy::needless_pass_by_value,
    clippy::redundant_clone,
    clippy::significant_drop_tightening,
    clippy::uninlined_format_args
)]

use crate::model::NodeTemplate;
use anyhow::{Context, Result};
use log::{debug, info, warn};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};

const CURRENT_DATA_DIR_SLUG: &str = "openflow";
const LEGACY_DATA_DIR_SLUG: &str = "step-through-agentic-workflow";
const TEMPLATES_FILE_NAME: &str = "templates.json";

pub trait TemplateStore {
    fn list(&self) -> Vec<NodeTemplate>;

    fn add(&self, template: NodeTemplate);

    fn remove(&self, id: &str);

    fn update(&self, template: NodeTemplate);
}

pub struct FileTemplateStore {
    templates: RwLock<Vec<NodeTemplate>>,
    path: PathBuf,
}

impl FileTemplateStore {
    pub fn new() -> Result<Self> {
        let data_dir = dirs::data_local_dir().context("cannot determine local data directory")?;
        Self::new_in(data_dir)
    }

    fn new_in(data_dir: PathBuf) -> Result<Self> {
        let dir = data_dir.join(CURRENT_DATA_DIR_SLUG);
        let path = dir.join(TEMPLATES_FILE_NAME);
        if path.exists() {
            return Self::new_at(path);
        }

        let legacy_path = data_dir
            .join(LEGACY_DATA_DIR_SLUG)
            .join(TEMPLATES_FILE_NAME);
        if legacy_path.exists() {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("cannot create directory: {}", dir.display()))?;
            let templates = Self::load_existing_at(&legacy_path, &path)?;
            return Ok(Self {
                templates: RwLock::new(templates),
                path,
            });
        }

        std::fs::create_dir_all(&dir)
            .with_context(|| format!("cannot create directory: {}", dir.display()))?;
        Self::new_at(path)
    }

    pub(crate) fn new_at(path: PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create directory: {}", parent.display()))?;
        }

        let templates = if path.exists() {
            Self::load_existing_at(&path, &path)?
        } else {
            info!("no templates file found, bootstrapping with builtin defaults");
            let defaults = NodeTemplate::builtin_defaults();
            Self::write_templates(&path, &defaults)?;
            defaults
        };

        Ok(Self {
            templates: RwLock::new(templates),
            path,
        })
    }

    fn load_existing_at(source_path: &Path, write_path: &Path) -> Result<Vec<NodeTemplate>> {
        debug!("loading templates from {}", source_path.display());
        let data = std::fs::read_to_string(source_path)
            .with_context(|| format!("cannot read templates file: {}", source_path.display()))?;
        match serde_json::from_str::<Vec<NodeTemplate>>(&data) {
            Ok(loaded) => {
                info!("loaded {} templates from disk", loaded.len());
                Ok(loaded)
            }
            Err(e) => {
                warn!("corrupt templates file, renaming to .bak: {:#}", e);
                let bak_path = source_path.with_extension("json.bak");
                if let Err(bak_err) = std::fs::rename(source_path, &bak_path) {
                    warn!("failed to rename corrupt templates file: {:#}", bak_err);
                }
                let defaults = NodeTemplate::builtin_defaults();
                Self::write_templates(write_path, &defaults)?;
                Ok(defaults)
            }
        }
    }

    fn write_templates(path: &Path, templates: &[NodeTemplate]) -> Result<()> {
        let json = serde_json::to_string_pretty(templates)
            .context("cannot serialize default templates")?;
        std::fs::write(path, json)
            .with_context(|| format!("cannot write templates to: {}", path.display()))?;
        Ok(())
    }

    fn save(&self) -> Result<()> {
        let data = self.templates.read();
        Self::write_templates(&self.path, &data)?;
        debug!("saved {} templates to disk", data.len());
        Ok(())
    }

    fn try_save(&self) {
        if let Err(e) = self.save() {
            warn!("failed to save templates: {:#}", e);
        }
    }
}

impl TemplateStore for FileTemplateStore {
    fn list(&self) -> Vec<NodeTemplate> {
        self.templates.read().clone()
    }

    fn add(&self, template: NodeTemplate) {
        self.templates.write().push(template);
        self.try_save();
    }

    fn remove(&self, id: &str) {
        self.templates.write().retain(|t| t.id != id);
        self.try_save();
    }

    fn update(&self, template: NodeTemplate) {
        let mut data = self.templates.write();
        if let Some(existing) = data.iter_mut().find(|t| t.id == template.id) {
            *existing = template;
            drop(data);
            self.try_save();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    fn empty_template_store() -> (FileTemplateStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("templates.json");

        let store = FileTemplateStore {
            templates: RwLock::new(Vec::new()),
            path,
        };
        (store, dir)
    }

    fn storage_path(root: &std::path::Path, slug: &str) -> PathBuf {
        root.join(slug).join(TEMPLATES_FILE_NAME)
    }

    #[test]
    fn add_and_list_templates() {
        let (store, _dir) = empty_template_store();

        let template = NodeTemplate::builtin_defaults()
            .into_iter()
            .find(|t| t.id == "builtin.writer")
            .unwrap();

        store.add(template.clone());
        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "builtin.writer");
    }

    #[test]
    fn remove_template() {
        let (store, _dir) = empty_template_store();

        let templates = NodeTemplate::builtin_defaults();
        for t in templates {
            store.add(t);
        }

        store.remove("builtin.analyst");
        let list = store.list();
        assert!(!list.iter().any(|t| t.id == "builtin.analyst"));
        assert!(list.iter().any(|t| t.id == "builtin.writer"));
    }

    #[test]
    fn update_template() {
        let (store, _dir) = empty_template_store();

        let mut template = NodeTemplate::builtin_defaults()
            .into_iter()
            .find(|t| t.id == "builtin.task-runner")
            .unwrap();

        store.add(template.clone());

        template.name = "Updated Runner".to_string();
        store.update(template.clone());

        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Updated Runner");
    }

    #[test]
    fn builtin_defaults_has_six_templates() {
        let templates = NodeTemplate::builtin_defaults();
        assert_eq!(templates.len(), 6);
    }

    #[test]
    fn corrupt_file_recovers_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("templates.json");

        std::fs::write(&path, b"this is not valid json").unwrap();

        let store = FileTemplateStore::new_at(path.clone()).unwrap();
        let list = store.list();
        assert_eq!(list.len(), 6, "should recover with builtin defaults");

        let bak_path = path.with_extension("json.bak");
        assert!(bak_path.exists(), "corrupt file should be renamed to .bak");
        assert_eq!(
            std::fs::read_to_string(&bak_path).unwrap(),
            "this is not valid json"
        );

        let fresh_data = std::fs::read_to_string(&path).unwrap();
        assert!(serde_json::from_str::<Vec<NodeTemplate>>(&fresh_data).is_ok());

        assert_eq!(list, NodeTemplate::builtin_defaults());
    }

    #[test]
    fn new_store_bootstraps_templates_in_openflow_path() {
        let dir = tempfile::tempdir().unwrap();
        let openflow_path = storage_path(dir.path(), CURRENT_DATA_DIR_SLUG);
        let legacy_path = storage_path(dir.path(), LEGACY_DATA_DIR_SLUG);

        let store = FileTemplateStore::new_in(dir.path().to_path_buf()).unwrap();

        assert_eq!(store.list(), NodeTemplate::builtin_defaults());
        assert!(openflow_path.exists());
        assert!(!legacy_path.exists());
    }

    #[test]
    fn openflow_templates_take_precedence_over_legacy_templates() {
        let dir = tempfile::tempdir().unwrap();
        let legacy_path = storage_path(dir.path(), LEGACY_DATA_DIR_SLUG);
        let openflow_path = storage_path(dir.path(), CURRENT_DATA_DIR_SLUG);
        std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(openflow_path.parent().unwrap()).unwrap();

        let mut legacy_templates = vec![NodeTemplate::builtin_defaults()
            .into_iter()
            .find(|t| t.id == "builtin.writer")
            .unwrap()];
        legacy_templates[0].name = "Legacy Writer".to_string();
        let mut openflow_templates = vec![NodeTemplate::builtin_defaults()
            .into_iter()
            .find(|t| t.id == "builtin.writer")
            .unwrap()];
        openflow_templates[0].name = "Openflow Writer".to_string();
        std::fs::write(
            &legacy_path,
            serde_json::to_string_pretty(&legacy_templates).unwrap(),
        )
        .unwrap();
        std::fs::write(
            &openflow_path,
            serde_json::to_string_pretty(&openflow_templates).unwrap(),
        )
        .unwrap();

        let store = FileTemplateStore::new_in(dir.path().to_path_buf()).unwrap();

        assert_eq!(store.list(), openflow_templates);
    }

    #[test]
    fn loads_legacy_templates_when_openflow_file_is_missing_and_saves_to_openflow() {
        let dir = tempfile::tempdir().unwrap();
        let legacy_path = storage_path(dir.path(), LEGACY_DATA_DIR_SLUG);
        let openflow_path = storage_path(dir.path(), CURRENT_DATA_DIR_SLUG);
        std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();

        let mut legacy_templates = vec![NodeTemplate::builtin_defaults()
            .into_iter()
            .find(|t| t.id == "builtin.writer")
            .unwrap()];
        legacy_templates[0].name = "Legacy Writer".to_string();
        std::fs::write(
            &legacy_path,
            serde_json::to_string_pretty(&legacy_templates).unwrap(),
        )
        .unwrap();

        let store = FileTemplateStore::new_in(dir.path().to_path_buf()).unwrap();
        assert_eq!(store.list(), legacy_templates);
        assert!(!openflow_path.exists());

        let analyst = NodeTemplate::builtin_defaults()
            .into_iter()
            .find(|t| t.id == "builtin.analyst")
            .unwrap();
        store.add(analyst);

        let saved_templates: Vec<NodeTemplate> =
            serde_json::from_str(&std::fs::read_to_string(&openflow_path).unwrap()).unwrap();
        assert_eq!(saved_templates, store.list());

        let legacy_saved: Vec<NodeTemplate> =
            serde_json::from_str(&std::fs::read_to_string(&legacy_path).unwrap()).unwrap();
        assert_eq!(legacy_saved, legacy_templates);
    }

    #[test]
    fn corrupt_legacy_file_recovers_defaults_into_openflow_path() {
        let dir = tempfile::tempdir().unwrap();
        let legacy_path = storage_path(dir.path(), LEGACY_DATA_DIR_SLUG);
        let openflow_path = storage_path(dir.path(), CURRENT_DATA_DIR_SLUG);
        std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_path, b"this is not valid json").unwrap();

        let store = FileTemplateStore::new_in(dir.path().to_path_buf()).unwrap();
        assert_eq!(store.list(), NodeTemplate::builtin_defaults());

        let bak_path = legacy_path.with_extension("json.bak");
        assert!(bak_path.exists());
        assert_eq!(
            std::fs::read_to_string(&bak_path).unwrap(),
            "this is not valid json"
        );

        let saved_templates: Vec<NodeTemplate> =
            serde_json::from_str(&std::fs::read_to_string(&openflow_path).unwrap()).unwrap();
        assert_eq!(saved_templates, NodeTemplate::builtin_defaults());
        assert!(!legacy_path.exists());
    }
}
