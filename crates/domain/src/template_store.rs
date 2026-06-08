#![allow(
    clippy::missing_errors_doc,
    clippy::needless_pass_by_value,
    clippy::redundant_clone,
    clippy::significant_drop_tightening,
    clippy::uninlined_format_args
)]

use crate::model::AgentNodeConfig;
use crate::template::{default_templates, Template};
use anyhow::{Context, Result};
use log::{debug, info, warn};
use parking_lot::RwLock;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const CURRENT_DATA_DIR_SLUG: &str = "openflow";
const LEGACY_DATA_DIR_SLUG: &str = "step-through-agentic-workflow";
const TEMPLATES_FILE_NAME: &str = "templates.json";

pub trait TemplateStore {
    fn list(&self) -> Vec<Template>;

    fn add(&self, template: Template) -> Result<()>;

    fn remove(&self, id: &str) -> Result<()>;

    fn update(&self, template: Template) -> Result<()>;
}

pub struct FileTemplateStore {
    templates: RwLock<Vec<Template>>,
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
            let defaults = default_templates();
            Self::write_templates(&path, &defaults)?;
            defaults
        };

        Ok(Self {
            templates: RwLock::new(templates),
            path,
        })
    }

    fn load_existing_at(source_path: &Path, write_path: &Path) -> Result<Vec<Template>> {
        debug!("loading templates from {}", source_path.display());
        let data = std::fs::read_to_string(source_path)
            .with_context(|| format!("cannot read templates file: {}", source_path.display()))?;
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

    fn write_templates(path: &Path, templates: &[Template]) -> Result<()> {
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
}

impl TemplateStore for FileTemplateStore {
    fn list(&self) -> Vec<Template> {
        self.templates.read().clone()
    }

    fn add(&self, template: Template) -> Result<()> {
        self.templates.write().push(template);
        self.save()
    }

    fn remove(&self, id: &str) -> Result<()> {
        self.templates.write().retain(|t| t.id != id);
        self.save()
    }

    fn update(&self, template: Template) -> Result<()> {
        let mut data = self.templates.write();
        if let Some(existing) = data.iter_mut().find(|t| t.id == template.id) {
            *existing = template;
            drop(data);
            return self.save();
        }
        Ok(())
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

    fn template(id: &str) -> Template {
        default_templates()
            .into_iter()
            .find(|template| template.id == id)
            .unwrap()
    }

    fn legacy_template_json(id: &str, name: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "name": name,
            "description": "Legacy template",
            "config": AgentNodeConfig::default()
        })
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
        let openflow_path = storage_path(dir.path(), CURRENT_DATA_DIR_SLUG);
        let legacy_path = storage_path(dir.path(), LEGACY_DATA_DIR_SLUG);

        let store = FileTemplateStore::new_in(dir.path().to_path_buf()).unwrap();

        assert_eq!(store.list(), default_templates());
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

        let legacy_templates = vec![legacy_template_json(
            "builtin.simple-agent",
            "Legacy Writer",
        )];
        let mut openflow_templates = vec![template("builtin.simple-agent")];
        openflow_templates[0].display_name = "Openflow Writer".to_string();
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

        let legacy_templates = vec![legacy_template_json("builtin.writer", "Legacy Writer")];
        std::fs::write(
            &legacy_path,
            serde_json::to_string_pretty(&legacy_templates).unwrap(),
        )
        .unwrap();

        let store = FileTemplateStore::new_in(dir.path().to_path_buf()).unwrap();
        assert_eq!(store.list()[0].id, "builtin.writer");
        assert_eq!(store.list()[0].display_name, "Legacy Writer");
        assert!(store.list()[0].locked_fields.is_empty());
        assert!(!openflow_path.exists());

        let classifier = template("builtin.classifier");
        store.add(classifier).unwrap();

        let saved_templates: Vec<Template> =
            serde_json::from_str(&std::fs::read_to_string(&openflow_path).unwrap()).unwrap();
        assert_eq!(saved_templates, store.list());

        let legacy_saved: Vec<serde_json::Value> =
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
        assert_eq!(store.list(), default_templates());

        let bak_path = legacy_path.with_extension("json.bak");
        assert!(bak_path.exists());
        assert_eq!(
            std::fs::read_to_string(&bak_path).unwrap(),
            "this is not valid json"
        );

        let saved_templates: Vec<Template> =
            serde_json::from_str(&std::fs::read_to_string(&openflow_path).unwrap()).unwrap();
        assert_eq!(saved_templates, default_templates());
        assert!(!legacy_path.exists());
    }

    #[test]
    fn add_returns_err_when_path_unwritable() {
        let dir = tempfile::tempdir().unwrap();
        let file_parent = dir.path().join("not-a-directory");
        std::fs::write(&file_parent, "file, not directory").unwrap();
        let store = FileTemplateStore {
            templates: RwLock::new(Vec::new()),
            path: file_parent.join("templates.json"),
        };

        let err = store.add(template("builtin.simple-agent")).unwrap_err();

        assert!(err.to_string().contains("cannot write templates"));
        assert_eq!(store.list().len(), 1);
    }
}
