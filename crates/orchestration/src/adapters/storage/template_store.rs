use crate::adapters::storage::json_file_store::{atomic_write, OPENFLOW_DATA_DIR_SLUG};
use engine::{default_templates, AgentNodeConfig, Template, TemplateStore, TemplateStoreError};
use log::{debug, info, warn};
use parking_lot::RwLock;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const TEMPLATES_FILE_NAME: &str = "templates.json";

pub struct FileTemplateStore {
    templates: RwLock<Vec<Template>>,
    path: PathBuf,
}

impl FileTemplateStore {
    /// # Errors
    /// Returns an error when the data directory or templates file cannot be opened.
    pub fn new() -> Result<Self, TemplateStoreError> {
        let data_dir = dirs::data_local_dir().ok_or(TemplateStoreError::DataDirUnavailable)?;
        Self::new_in(data_dir)
    }

    fn new_in(data_dir: PathBuf) -> Result<Self, TemplateStoreError> {
        let path = data_dir
            .join(OPENFLOW_DATA_DIR_SLUG)
            .join(TEMPLATES_FILE_NAME);
        Self::new_at(path)
    }

    pub(crate) fn new_at(path: PathBuf) -> Result<Self, TemplateStoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| {
                TemplateStoreError::CannotCreateDir {
                    path: parent.display().to_string(),
                    source,
                }
            })?;
        }

        let templates = if path.exists() {
            Self::load_existing_at(&path, &path)?
        } else {
            info!("no templates file found, bootstrapping with builtin defaults");
            let defaults = default_templates();
            Self::write_templates(&path, &defaults)?;
            defaults
        };

        Ok(Self {
            templates: RwLock::new(templates),
            path,
        })
    }

    fn load_existing_at(
        source_path: &Path,
        write_path: &Path,
    ) -> Result<Vec<Template>, TemplateStoreError> {
        debug!("loading templates from {}", source_path.display());
        let data = std::fs::read_to_string(source_path).map_err(|source| {
            TemplateStoreError::CannotRead {
                path: source_path.display().to_string(),
                source,
            }
        })?;
        match serde_json::from_str::<Vec<Template>>(&data) {
            Ok(loaded) => {
                info!("loaded {} templates from disk", loaded.len());
                Ok(loaded)
            }
            Err(e) => {
                if let Ok(legacy) = serde_json::from_str::<Vec<LegacyNodeTemplate>>(&data) {
                    let loaded = legacy
                        .into_iter()
                        .map(LegacyNodeTemplate::into_template)
                        .collect::<Vec<_>>();
                    info!("migrated {} legacy templates from disk", loaded.len());
                    if source_path == write_path {
                        Self::write_templates(write_path, &loaded)?;
                    }
                    return Ok(loaded);
                }
                warn!("corrupt templates file, renaming to .bak: {:#}", e);
                let bak_path = source_path.with_extension("json.bak");
                if let Err(bak_err) = std::fs::rename(source_path, &bak_path) {
                    warn!("failed to rename corrupt templates file: {:#}", bak_err);
                }
                let defaults = default_templates();
                Self::write_templates(write_path, &defaults)?;
                Ok(defaults)
            }
        }
    }

    fn write_templates(path: &Path, templates: &[Template]) -> Result<(), TemplateStoreError> {
        let json = serde_json::to_string_pretty(templates)?;
        atomic_write(path, &json).map_err(|source| TemplateStoreError::CannotWrite {
            path: path.display().to_string(),
            source,
        })?;
        Ok(())
    }

    fn persist(&self, templates: &[Template]) -> Result<(), TemplateStoreError> {
        Self::write_templates(&self.path, templates)?;
        debug!("saved {} templates to disk", templates.len());
        Ok(())
    }

    fn mutate<F>(&self, mutate: F) -> Result<(), TemplateStoreError>
    where
        F: FnOnce(&mut Vec<Template>) -> Result<(), TemplateStoreError>,
    {
        let mut data = self.templates.write();
        let snapshot = data.clone();
        mutate(&mut data)?;
        if let Err(error) = self.persist(&data) {
            *data = snapshot;
            return Err(error);
        }
        Ok(())
    }
}

impl TemplateStore for FileTemplateStore {
    fn list(&self) -> Vec<Template> {
        self.templates.read().clone()
    }

    fn add(&self, template: Template) -> Result<(), TemplateStoreError> {
        self.mutate(|data| {
            data.push(template);
            Ok(())
        })
    }

    fn remove(&self, id: &str) -> Result<(), TemplateStoreError> {
        self.mutate(|data| {
            data.retain(|t| t.id != id);
            Ok(())
        })
    }

    fn update(&self, template: Template) -> Result<(), TemplateStoreError> {
        self.mutate(|data| {
            let Some(existing) = data.iter_mut().find(|t| t.id == template.id) else {
                return Err(TemplateStoreError::NotFound {
                    id: template.id.clone(),
                });
            };
            *existing = template;
            Ok(())
        })
    }
}

#[derive(Debug, Deserialize)]
struct LegacyNodeTemplate {
    id: String,
    name: String,
    description: String,
    config: AgentNodeConfig,
}

impl LegacyNodeTemplate {
    fn into_template(self) -> Template {
        Template {
            id: self.id,
            display_name: self.name,
            description: self.description,
            default_config: self.config,
            locked_fields: HashSet::new(),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "storage tests use unwrap/expect for brevity"
)]
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

    fn template(id: &str) -> Template {
        default_templates()
            .into_iter()
            .find(|template| template.id == id)
            .unwrap()
    }

    #[test]
    fn add_and_list_templates() {
        let (store, _dir) = empty_template_store();

        let template = template("builtin.simple-agent");

        store.add(template.clone()).unwrap();
        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "builtin.simple-agent");
    }

    #[test]
    fn remove_template() {
        let (store, _dir) = empty_template_store();

        let templates = default_templates();
        for t in templates {
            store.add(t).unwrap();
        }

        store.remove("builtin.classifier").unwrap();
        let list = store.list();
        assert!(!list.iter().any(|t| t.id == "builtin.classifier"));
        assert!(list.iter().any(|t| t.id == "builtin.simple-agent"));
    }

    #[test]
    fn update_template() {
        let (store, _dir) = empty_template_store();

        let mut template = template("builtin.simple-agent");

        store.add(template.clone()).unwrap();

        template.display_name = "Updated Runner".to_string();
        store.update(template.clone()).unwrap();

        let list = store.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].display_name, "Updated Runner");
    }

    #[test]
    fn update_missing_template_returns_not_found() {
        let (store, _dir) = empty_template_store();

        let err = store.update(template("builtin.simple-agent")).unwrap_err();

        assert!(matches!(err, TemplateStoreError::NotFound { .. }));
        assert_eq!(store.list().len(), 0);
    }

    #[test]
    fn builtin_defaults_use_canonical_templates() {
        let templates = default_templates();
        assert_eq!(templates.len(), 5);
        assert!(templates
            .iter()
            .any(|template| template.id == "builtin.code-reviewer"));
    }

    #[test]
    fn corrupt_file_recovers_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("templates.json");

        std::fs::write(&path, b"this is not valid json").unwrap();

        let store = FileTemplateStore::new_at(path.clone()).unwrap();
        let list = store.list();
        assert_eq!(list.len(), 5, "should recover with builtin defaults");

        let bak_path = path.with_extension("json.bak");
        assert!(bak_path.exists(), "corrupt file should be renamed to .bak");
        assert_eq!(
            std::fs::read_to_string(&bak_path).unwrap(),
            "this is not valid json"
        );

        let fresh_data = std::fs::read_to_string(&path).unwrap();
        assert!(serde_json::from_str::<Vec<Template>>(&fresh_data).is_ok());

        assert_eq!(list, default_templates());
    }

    #[test]
    fn new_store_bootstraps_templates_in_openflow_path() {
        let dir = tempfile::tempdir().unwrap();
        let openflow_path = storage_path(dir.path(), OPENFLOW_DATA_DIR_SLUG);

        let store = FileTemplateStore::new_in(dir.path().to_path_buf()).unwrap();

        assert_eq!(store.list(), default_templates());
        assert!(openflow_path.exists());
    }

    #[test]
    fn add_rolls_back_memory_when_path_unwritable() {
        let dir = tempfile::tempdir().unwrap();
        let file_parent = dir.path().join("not-a-directory");
        std::fs::write(&file_parent, "file, not directory").unwrap();
        let store = FileTemplateStore {
            templates: RwLock::new(Vec::new()),
            path: file_parent.join("templates.json"),
        };

        let err = store.add(template("builtin.simple-agent")).unwrap_err();

        assert!(matches!(err, TemplateStoreError::CannotWrite { .. }));
        assert_eq!(store.list().len(), 0);
    }
}
