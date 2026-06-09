use gemed_core::{WorkflowError, WorkflowFile};
use gemed_providers::ProviderConfigSet;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const DEFAULT_AUTOSAVE_SLOT: &str = "autosave";
pub const DEFAULT_PROVIDER_CONFIG_SLOT: &str = "providers";
pub const PROVIDER_CONFIG_SCHEMA_VERSION: u8 = 1;
pub const PROVIDER_CONFIG_DIR: &str = "provider-configs";
pub const PROJECT_SCHEMA_VERSION: u8 = 1;
pub const PROJECT_MANIFEST_FILE: &str = "gemed-project.json";
pub const PROJECT_WORKFLOW_FILE: &str = "workflow.json";
pub const PROJECT_MEDIA_DIR: &str = "media";
pub const PROJECT_MEDIA_URL_PREFIX: &str = "gemed-media://";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowProjectManifest {
    pub version: u8,
    pub name: String,
    pub workflow_file: String,
    pub media_dir: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media_files: Vec<String>,
}

impl WorkflowProjectManifest {
    pub fn from_workflow(workflow: &WorkflowFile) -> Self {
        Self {
            version: PROJECT_SCHEMA_VERSION,
            name: workflow.name.clone(),
            workflow_file: PROJECT_WORKFLOW_FILE.to_string(),
            media_dir: PROJECT_MEDIA_DIR.to_string(),
            media_files: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowSnapshot {
    pub slot: String,
    pub name: String,
    pub json: String,
}

impl WorkflowSnapshot {
    pub fn from_workflow(slot: impl Into<String>, workflow: &WorkflowFile) -> Result<Self> {
        let json = workflow.to_pretty_json()?;
        Ok(Self {
            slot: slot.into(),
            name: workflow.name.clone(),
            json,
        })
    }

    pub fn parse(&self) -> Result<WorkflowFile> {
        Ok(WorkflowFile::from_json_str(&self.json)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigFile {
    pub version: u8,
    pub provider_config: ProviderConfigSet,
}

impl ProviderConfigFile {
    pub fn new(provider_config: ProviderConfigSet) -> Self {
        Self {
            version: PROVIDER_CONFIG_SCHEMA_VERSION,
            provider_config,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfigSnapshot {
    pub slot: String,
    pub json: String,
}

impl ProviderConfigSnapshot {
    pub fn from_config(slot: impl Into<String>, config: &ProviderConfigSet) -> Result<Self> {
        let slot = normalize_slot(&slot.into())?;
        let payload = ProviderConfigFile::new(config.clone());
        let json = serde_json::to_string_pretty(&payload).map_err(|source| {
            StorageError::Backend(format!("provider config export failed: {source}"))
        })?;
        Ok(Self { slot, json })
    }

    pub fn parse(&self) -> Result<ProviderConfigSet> {
        let payload: ProviderConfigFile = serde_json::from_str(&self.json).map_err(|source| {
            StorageError::Backend(format!("provider config parse failed: {source}"))
        })?;
        if payload.version != PROVIDER_CONFIG_SCHEMA_VERSION {
            return Err(StorageError::Backend(format!(
                "provider config version `{}` is not supported",
                payload.version
            )));
        }
        Ok(payload.provider_config)
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error(transparent)]
    Workflow(#[from] WorkflowError),
    #[error("workflow slot `{0}` was not found")]
    NotFound(String),
    #[error("workflow slot must not be empty")]
    EmptySlot,
    #[error("storage backend failed: {0}")]
    Backend(String),
    #[cfg(feature = "desktop")]
    #[error("desktop app data directory is unavailable")]
    AppDataUnavailable,
    #[cfg(feature = "desktop")]
    #[error("filesystem storage failed at `{path}`: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
}

pub type Result<T> = std::result::Result<T, StorageError>;

pub trait WorkflowStorage {
    fn save_workflow(&mut self, slot: &str, workflow: &WorkflowFile) -> Result<WorkflowSnapshot>;
    fn load_workflow(&self, slot: &str) -> Result<WorkflowFile>;
    fn export_workflow(&self, workflow: &WorkflowFile) -> Result<String> {
        Ok(workflow.to_pretty_json()?)
    }
    fn list_workflows(&self) -> Result<Vec<WorkflowSnapshot>>;
    fn delete_workflow(&mut self, slot: &str) -> Result<()>;
}

pub trait ProviderConfigStorage {
    fn save_provider_config(
        &mut self,
        slot: &str,
        config: &ProviderConfigSet,
    ) -> Result<ProviderConfigSnapshot>;
    fn load_provider_config(&self, slot: &str) -> Result<ProviderConfigSet>;
    fn list_provider_configs(&self) -> Result<Vec<ProviderConfigSnapshot>>;
    fn delete_provider_config(&mut self, slot: &str) -> Result<()>;
}

#[derive(Debug, Clone, Default)]
pub struct MemoryWorkflowStorage {
    snapshots: BTreeMap<String, WorkflowSnapshot>,
    provider_configs: BTreeMap<String, ProviderConfigSnapshot>,
}

impl MemoryWorkflowStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_workflow(slot: &str, workflow: &WorkflowFile) -> Result<Self> {
        let mut storage = Self::new();
        storage.save_workflow(slot, workflow)?;
        Ok(storage)
    }
}

impl WorkflowStorage for MemoryWorkflowStorage {
    fn save_workflow(&mut self, slot: &str, workflow: &WorkflowFile) -> Result<WorkflowSnapshot> {
        let slot = normalize_slot(slot)?;
        let snapshot = WorkflowSnapshot::from_workflow(slot.clone(), workflow)?;
        self.snapshots.insert(slot, snapshot.clone());
        Ok(snapshot)
    }

    fn load_workflow(&self, slot: &str) -> Result<WorkflowFile> {
        let slot = normalize_slot(slot)?;
        self.snapshots
            .get(&slot)
            .ok_or_else(|| StorageError::NotFound(slot.clone()))?
            .parse()
    }

    fn list_workflows(&self) -> Result<Vec<WorkflowSnapshot>> {
        Ok(self.snapshots.values().cloned().collect())
    }

    fn delete_workflow(&mut self, slot: &str) -> Result<()> {
        let slot = normalize_slot(slot)?;
        self.snapshots
            .remove(&slot)
            .map(|_| ())
            .ok_or(StorageError::NotFound(slot))
    }
}

impl ProviderConfigStorage for MemoryWorkflowStorage {
    fn save_provider_config(
        &mut self,
        slot: &str,
        config: &ProviderConfigSet,
    ) -> Result<ProviderConfigSnapshot> {
        let snapshot = ProviderConfigSnapshot::from_config(slot, config)?;
        self.provider_configs
            .insert(snapshot.slot.clone(), snapshot.clone());
        Ok(snapshot)
    }

    fn load_provider_config(&self, slot: &str) -> Result<ProviderConfigSet> {
        let slot = normalize_slot(slot)?;
        self.provider_configs
            .get(&slot)
            .ok_or_else(|| StorageError::NotFound(slot.clone()))?
            .parse()
    }

    fn list_provider_configs(&self) -> Result<Vec<ProviderConfigSnapshot>> {
        Ok(self.provider_configs.values().cloned().collect())
    }

    fn delete_provider_config(&mut self, slot: &str) -> Result<()> {
        let slot = normalize_slot(slot)?;
        self.provider_configs
            .remove(&slot)
            .map(|_| ())
            .ok_or(StorageError::NotFound(slot))
    }
}

#[cfg(feature = "desktop")]
pub mod desktop {
    use super::{
        DEFAULT_AUTOSAVE_SLOT, DEFAULT_PROVIDER_CONFIG_SLOT, PROJECT_MANIFEST_FILE,
        PROJECT_MEDIA_DIR, PROJECT_MEDIA_URL_PREFIX, PROJECT_WORKFLOW_FILE, PROVIDER_CONFIG_DIR,
        ProviderConfigSnapshot, ProviderConfigStorage, Result, StorageError,
        WorkflowProjectManifest, WorkflowSnapshot, WorkflowStorage, normalize_slot,
    };
    use base64::{Engine as _, engine::general_purpose};
    use directories::ProjectDirs;
    use gemed_core::WorkflowFile;
    use gemed_providers::ProviderConfigSet;
    use serde_json::Value;
    use std::collections::HashSet;
    use std::path::{Component, Path, PathBuf};

    #[derive(Debug, Clone)]
    pub struct DesktopWorkflowStorage {
        root: PathBuf,
    }

    impl DesktopWorkflowStorage {
        pub fn new() -> Result<Self> {
            let dirs = ProjectDirs::from("io.github", "v1cc0", "GemEd")
                .ok_or(StorageError::AppDataUnavailable)?;
            Ok(Self::at_dir(dirs.data_local_dir().join("workflows")))
        }

        pub fn at_dir(root: impl Into<PathBuf>) -> Self {
            Self { root: root.into() }
        }

        pub fn root(&self) -> &Path {
            &self.root
        }

        pub fn autosave_path(&self) -> Result<PathBuf> {
            self.path_for_slot(DEFAULT_AUTOSAVE_SLOT)
        }

        pub fn default_provider_config_path(&self) -> Result<PathBuf> {
            self.path_for_provider_config_slot(DEFAULT_PROVIDER_CONFIG_SLOT)
        }

        fn path_for_slot(&self, slot: &str) -> Result<PathBuf> {
            Ok(self.root.join(format!("{}.json", normalize_slot(slot)?)))
        }

        fn provider_config_root(&self) -> PathBuf {
            self.root.join(PROVIDER_CONFIG_DIR)
        }

        fn path_for_provider_config_slot(&self, slot: &str) -> Result<PathBuf> {
            Ok(self
                .provider_config_root()
                .join(format!("{}.json", normalize_slot(slot)?)))
        }

        fn ensure_root(&self) -> Result<()> {
            std::fs::create_dir_all(&self.root).map_err(|source| StorageError::Io {
                path: self.root.clone(),
                source,
            })
        }

        fn ensure_provider_config_root(&self) -> Result<PathBuf> {
            let root = self.provider_config_root();
            std::fs::create_dir_all(&root).map_err(|source| StorageError::Io {
                path: root.clone(),
                source,
            })?;
            Ok(root)
        }
    }

    impl WorkflowStorage for DesktopWorkflowStorage {
        fn save_workflow(
            &mut self,
            slot: &str,
            workflow: &WorkflowFile,
        ) -> Result<WorkflowSnapshot> {
            self.ensure_root()?;
            let snapshot = WorkflowSnapshot::from_workflow(normalize_slot(slot)?, workflow)?;
            let path = self.path_for_slot(&snapshot.slot)?;
            std::fs::write(&path, snapshot.json.as_bytes())
                .map_err(|source| StorageError::Io { path, source })?;
            Ok(snapshot)
        }

        fn load_workflow(&self, slot: &str) -> Result<WorkflowFile> {
            let path = self.path_for_slot(slot)?;
            let json = std::fs::read_to_string(&path).map_err(|source| StorageError::Io {
                path: path.clone(),
                source,
            })?;
            Ok(WorkflowFile::from_json_str(&json)?)
        }

        fn list_workflows(&self) -> Result<Vec<WorkflowSnapshot>> {
            if !self.root.exists() {
                return Ok(Vec::new());
            }
            let entries = std::fs::read_dir(&self.root).map_err(|source| StorageError::Io {
                path: self.root.clone(),
                source,
            })?;
            let mut snapshots = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|source| StorageError::Io {
                    path: self.root.clone(),
                    source,
                })?;
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let json = std::fs::read_to_string(&path).map_err(|source| StorageError::Io {
                    path: path.clone(),
                    source,
                })?;
                let workflow = WorkflowFile::from_json_str(&json)?;
                let slot = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or(DEFAULT_AUTOSAVE_SLOT)
                    .to_string();
                snapshots.push(WorkflowSnapshot {
                    slot,
                    name: workflow.name,
                    json,
                });
            }
            snapshots.sort_by(|left, right| left.slot.cmp(&right.slot));
            Ok(snapshots)
        }

        fn delete_workflow(&mut self, slot: &str) -> Result<()> {
            let path = self.path_for_slot(slot)?;
            if !path.exists() {
                return Err(StorageError::NotFound(normalize_slot(slot)?));
            }
            std::fs::remove_file(&path).map_err(|source| StorageError::Io { path, source })
        }
    }

    impl ProviderConfigStorage for DesktopWorkflowStorage {
        fn save_provider_config(
            &mut self,
            slot: &str,
            config: &ProviderConfigSet,
        ) -> Result<ProviderConfigSnapshot> {
            self.ensure_provider_config_root()?;
            let snapshot = ProviderConfigSnapshot::from_config(slot, config)?;
            let path = self.path_for_provider_config_slot(&snapshot.slot)?;
            std::fs::write(&path, snapshot.json.as_bytes())
                .map_err(|source| StorageError::Io { path, source })?;
            Ok(snapshot)
        }

        fn load_provider_config(&self, slot: &str) -> Result<ProviderConfigSet> {
            let path = self.path_for_provider_config_slot(slot)?;
            let json = std::fs::read_to_string(&path).map_err(|source| StorageError::Io {
                path: path.clone(),
                source,
            })?;
            ProviderConfigSnapshot {
                slot: normalize_slot(slot)?,
                json,
            }
            .parse()
        }

        fn list_provider_configs(&self) -> Result<Vec<ProviderConfigSnapshot>> {
            let root = self.provider_config_root();
            if !root.exists() {
                return Ok(Vec::new());
            }
            let entries = std::fs::read_dir(&root).map_err(|source| StorageError::Io {
                path: root.clone(),
                source,
            })?;
            let mut snapshots = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|source| StorageError::Io {
                    path: root.clone(),
                    source,
                })?;
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let json = std::fs::read_to_string(&path).map_err(|source| StorageError::Io {
                    path: path.clone(),
                    source,
                })?;
                let slot = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or(DEFAULT_PROVIDER_CONFIG_SLOT)
                    .to_string();
                let snapshot = ProviderConfigSnapshot { slot, json };
                let _ = snapshot.parse()?;
                snapshots.push(snapshot);
            }
            snapshots.sort_by(|left, right| left.slot.cmp(&right.slot));
            Ok(snapshots)
        }

        fn delete_provider_config(&mut self, slot: &str) -> Result<()> {
            let path = self.path_for_provider_config_slot(slot)?;
            if !path.exists() {
                return Err(StorageError::NotFound(normalize_slot(slot)?));
            }
            std::fs::remove_file(&path).map_err(|source| StorageError::Io { path, source })
        }
    }

    #[derive(Debug, Clone)]
    pub struct WorkflowProjectSnapshot {
        pub root: PathBuf,
        pub manifest: WorkflowProjectManifest,
        pub workflow: WorkflowFile,
    }

    #[derive(Debug, Clone)]
    pub struct DesktopWorkflowProject {
        root: PathBuf,
    }

    impl DesktopWorkflowProject {
        pub fn at_dir(root: impl Into<PathBuf>) -> Self {
            Self { root: root.into() }
        }

        pub fn root(&self) -> &Path {
            &self.root
        }

        pub fn manifest_path(&self) -> PathBuf {
            self.root.join(PROJECT_MANIFEST_FILE)
        }

        pub fn default_workflow_path(&self) -> PathBuf {
            self.root.join(PROJECT_WORKFLOW_FILE)
        }

        pub fn default_media_dir(&self) -> PathBuf {
            self.root.join(PROJECT_MEDIA_DIR)
        }

        pub fn save(&self, workflow: &WorkflowFile) -> Result<WorkflowProjectSnapshot> {
            let previous_media_files = self.previous_media_files();
            std::fs::create_dir_all(&self.root).map_err(|source| StorageError::Io {
                path: self.root.clone(),
                source,
            })?;
            std::fs::create_dir_all(self.default_media_dir()).map_err(|source| {
                StorageError::Io {
                    path: self.default_media_dir(),
                    source,
                }
            })?;

            let media_dir = self.default_media_dir();
            let (workflow_json, media_files) =
                externalize_workflow_media_json(workflow, &media_dir)?;
            let mut manifest = WorkflowProjectManifest::from_workflow(workflow);
            manifest.media_files = media_files;
            remove_stale_media_files(&self.root, &previous_media_files, &manifest.media_files)?;
            std::fs::write(self.default_workflow_path(), workflow_json.as_bytes()).map_err(
                |source| StorageError::Io {
                    path: self.default_workflow_path(),
                    source,
                },
            )?;
            let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|source| {
                StorageError::Backend(format!("project manifest export failed: {source}"))
            })?;
            std::fs::write(self.manifest_path(), manifest_json.as_bytes()).map_err(|source| {
                StorageError::Io {
                    path: self.manifest_path(),
                    source,
                }
            })?;

            Ok(WorkflowProjectSnapshot {
                root: self.root.clone(),
                manifest,
                workflow: workflow.clone(),
            })
        }

        fn previous_media_files(&self) -> Vec<String> {
            let manifest_path = self.manifest_path();
            if !manifest_path.exists() {
                return Vec::new();
            }
            let Ok(json) = std::fs::read_to_string(manifest_path) else {
                return Vec::new();
            };
            serde_json::from_str::<WorkflowProjectManifest>(&json)
                .map(|manifest| manifest.media_files)
                .unwrap_or_default()
        }

        pub fn load(&self) -> Result<WorkflowProjectSnapshot> {
            let manifest_path = self.manifest_path();
            let manifest = if manifest_path.exists() {
                let json =
                    std::fs::read_to_string(&manifest_path).map_err(|source| StorageError::Io {
                        path: manifest_path.clone(),
                        source,
                    })?;
                serde_json::from_str::<WorkflowProjectManifest>(&json).map_err(|source| {
                    StorageError::Backend(format!(
                        "project manifest parse failed at `{}`: {source}",
                        manifest_path.display()
                    ))
                })?
            } else {
                WorkflowProjectManifest {
                    version: super::PROJECT_SCHEMA_VERSION,
                    name: String::new(),
                    workflow_file: PROJECT_WORKFLOW_FILE.to_string(),
                    media_dir: PROJECT_MEDIA_DIR.to_string(),
                    media_files: Vec::new(),
                }
            };

            let workflow_path = safe_project_child(&self.root, &manifest.workflow_file)?;
            let json =
                std::fs::read_to_string(&workflow_path).map_err(|source| StorageError::Io {
                    path: workflow_path.clone(),
                    source,
                })?;
            let hydrated_json = hydrate_workflow_media_json(&self.root, &json)?;
            let workflow = WorkflowFile::from_json_str(&hydrated_json)?;
            let manifest = if manifest.name.trim().is_empty() {
                WorkflowProjectManifest {
                    name: workflow.name.clone(),
                    ..manifest
                }
            } else {
                manifest
            };
            let _ = safe_project_child(&self.root, &manifest.media_dir)?;

            Ok(WorkflowProjectSnapshot {
                root: self.root.clone(),
                manifest,
                workflow,
            })
        }
    }

    fn externalize_workflow_media_json(
        workflow: &WorkflowFile,
        media_dir: &Path,
    ) -> Result<(String, Vec<String>)> {
        let mut value = serde_json::to_value(workflow).map_err(|source| {
            StorageError::Backend(format!("workflow project JSON export failed: {source}"))
        })?;
        let mut media_files = Vec::new();
        externalize_media_value(&mut value, media_dir, &mut media_files)?;
        media_files.sort();
        media_files.dedup();
        let json = serde_json::to_string_pretty(&value).map_err(|source| {
            StorageError::Backend(format!("workflow project JSON formatting failed: {source}"))
        })?;
        Ok((json, media_files))
    }

    fn remove_stale_media_files(
        root: &Path,
        previous_media_files: &[String],
        current_media_files: &[String],
    ) -> Result<()> {
        let current: HashSet<&str> = current_media_files.iter().map(String::as_str).collect();
        for previous in previous_media_files {
            if current.contains(previous.as_str()) || !is_project_media_file_ref(previous) {
                continue;
            }
            let Ok(path) = safe_project_child(root, previous) else {
                continue;
            };
            if !path.is_file() {
                continue;
            }
            std::fs::remove_file(&path).map_err(|source| StorageError::Io { path, source })?;
        }
        Ok(())
    }

    fn is_project_media_file_ref(value: &str) -> bool {
        let mut components = Path::new(value).components();
        matches!(
            components.next(),
            Some(Component::Normal(component)) if component == PROJECT_MEDIA_DIR
        ) && components.all(|component| matches!(component, Component::Normal(_)))
    }

    fn externalize_media_value(
        value: &mut Value,
        media_dir: &Path,
        media_files: &mut Vec<String>,
    ) -> Result<()> {
        match value {
            Value::String(text) => {
                let Some(data_url) = DataUrl::parse(text) else {
                    return Ok(());
                };
                let filename = write_data_url_media(media_dir, &data_url)?;
                let relative = format!("{PROJECT_MEDIA_DIR}/{filename}");
                *text = format!("{PROJECT_MEDIA_URL_PREFIX}{relative}");
                media_files.push(relative);
                Ok(())
            }
            Value::Array(items) => {
                for item in items {
                    externalize_media_value(item, media_dir, media_files)?;
                }
                Ok(())
            }
            Value::Object(map) => {
                externalize_known_media_fields(map, media_dir, media_files)?;
                for item in map.values_mut() {
                    externalize_media_value(item, media_dir, media_files)?;
                }
                Ok(())
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
        }
    }

    fn externalize_known_media_fields(
        map: &mut serde_json::Map<String, Value>,
        media_dir: &Path,
        media_files: &mut Vec<String>,
    ) -> Result<()> {
        for (inline_key, ref_key) in SINGLE_MEDIA_FIELD_REFS {
            let Some(data_url) = map
                .get(*inline_key)
                .and_then(Value::as_str)
                .and_then(DataUrl::parse)
            else {
                continue;
            };
            let reference = write_media_reference(media_dir, &data_url, media_files)?;
            map.insert((*inline_key).to_string(), Value::Null);
            map.insert((*ref_key).to_string(), Value::String(reference));
        }

        for (inline_key, ref_key) in ARRAY_MEDIA_FIELD_REFS {
            let mut refs = map
                .get(*ref_key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let Some(Value::Array(items)) = map.get_mut(*inline_key) else {
                continue;
            };
            if refs.len() < items.len() {
                refs.resize(items.len(), Value::Null);
            }
            let mut changed = false;
            for (index, item) in items.iter_mut().enumerate() {
                let Some(data_url) = item.as_str().and_then(DataUrl::parse) else {
                    continue;
                };
                let reference = write_media_reference(media_dir, &data_url, media_files)?;
                refs[index] = Value::String(reference);
                *item = Value::String(String::new());
                changed = true;
            }
            if changed {
                map.insert((*ref_key).to_string(), Value::Array(refs));
            }
        }

        Ok(())
    }

    fn write_media_reference(
        media_dir: &Path,
        data_url: &DataUrl,
        media_files: &mut Vec<String>,
    ) -> Result<String> {
        let filename = write_data_url_media(media_dir, data_url)?;
        let relative = format!("{PROJECT_MEDIA_DIR}/{filename}");
        media_files.push(relative.clone());
        Ok(format!("{PROJECT_MEDIA_URL_PREFIX}{relative}"))
    }

    fn hydrate_workflow_media_json(root: &Path, json: &str) -> Result<String> {
        let mut value: Value = serde_json::from_str(json).map_err(|source| {
            StorageError::Backend(format!("workflow JSON parse failed: {source}"))
        })?;
        hydrate_media_value(root, &mut value)?;
        serde_json::to_string_pretty(&value).map_err(|source| {
            StorageError::Backend(format!(
                "workflow JSON hydration formatting failed: {source}"
            ))
        })
    }

    fn hydrate_media_value(root: &Path, value: &mut Value) -> Result<()> {
        match value {
            Value::String(text) => {
                let Some(data_url) = media_reference_to_data_url(root, text)? else {
                    return Ok(());
                };
                *text = data_url;
                Ok(())
            }
            Value::Array(items) => {
                for item in items {
                    hydrate_media_value(root, item)?;
                }
                Ok(())
            }
            Value::Object(map) => {
                hydrate_known_media_fields(root, map)?;
                for item in map.values_mut() {
                    hydrate_media_value(root, item)?;
                }
                Ok(())
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => Ok(()),
        }
    }

    fn hydrate_known_media_fields(
        root: &Path,
        map: &mut serde_json::Map<String, Value>,
    ) -> Result<()> {
        for (inline_key, ref_key) in SINGLE_MEDIA_FIELD_REFS {
            if !is_empty_media_value(map.get(*inline_key)) {
                continue;
            }
            let Some(reference) = map.get(*ref_key).and_then(Value::as_str) else {
                continue;
            };
            let Some(data_url) = media_reference_to_data_url(root, reference)? else {
                continue;
            };
            map.insert((*inline_key).to_string(), Value::String(data_url));
        }

        for (inline_key, ref_key) in ARRAY_MEDIA_FIELD_REFS {
            let Some(refs) = map.get(*ref_key).and_then(Value::as_array).cloned() else {
                continue;
            };
            let mut items = map
                .get(*inline_key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if items.len() < refs.len() {
                items.resize(refs.len(), Value::String(String::new()));
            }
            let mut changed = false;
            for (index, reference) in refs.iter().enumerate() {
                if !is_empty_media_value(items.get(index)) {
                    continue;
                }
                let Some(reference) = reference.as_str() else {
                    continue;
                };
                let Some(data_url) = media_reference_to_data_url(root, reference)? else {
                    continue;
                };
                items[index] = Value::String(data_url);
                changed = true;
            }
            if changed {
                map.insert((*inline_key).to_string(), Value::Array(items));
            }
        }

        Ok(())
    }

    fn is_empty_media_value(value: Option<&Value>) -> bool {
        match value {
            None | Some(Value::Null) => true,
            Some(Value::String(text)) => text.is_empty(),
            _ => false,
        }
    }

    fn media_reference_to_data_url(root: &Path, reference: &str) -> Result<Option<String>> {
        let Some(relative) = reference.strip_prefix(PROJECT_MEDIA_URL_PREFIX) else {
            return Ok(None);
        };
        let path = safe_project_child(root, relative)?;
        let bytes = std::fs::read(&path).map_err(|source| StorageError::Io {
            path: path.clone(),
            source,
        })?;
        let mime = mime_from_path(&path);
        let encoded = general_purpose::STANDARD.encode(bytes);
        Ok(Some(format!("data:{mime};base64,{encoded}")))
    }

    const SINGLE_MEDIA_FIELD_REFS: &[(&str, &str)] = &[
        ("image", "imageRef"),
        ("audioFile", "audioFileRef"),
        ("video", "videoRef"),
        ("sourceImage", "sourceImageRef"),
        ("outputImage", "outputImageRef"),
        ("outputVideo", "outputVideoRef"),
        ("outputAudio", "outputAudioRef"),
        ("imageA", "imageARef"),
        ("imageB", "imageBRef"),
        ("capturedImage", "capturedImageRef"),
    ];

    const ARRAY_MEDIA_FIELD_REFS: &[(&str, &str)] = &[
        ("inputImages", "inputImageRefs"),
        ("images", "imageRefs"),
        ("videos", "videoRefs"),
    ];

    struct DataUrl {
        mime: String,
        bytes: Vec<u8>,
    }

    impl DataUrl {
        fn parse(value: &str) -> Option<Self> {
            let value = value.strip_prefix("data:")?;
            let (metadata, encoded) = value.split_once(',')?;
            if !metadata
                .split(';')
                .any(|part| part.eq_ignore_ascii_case("base64"))
            {
                return None;
            }
            let mime = metadata
                .split(';')
                .next()
                .filter(|mime| !mime.trim().is_empty())
                .unwrap_or("application/octet-stream")
                .to_ascii_lowercase();
            let bytes = general_purpose::STANDARD.decode(encoded).ok()?;
            Some(Self { mime, bytes })
        }
    }

    fn write_data_url_media(media_dir: &Path, data_url: &DataUrl) -> Result<String> {
        std::fs::create_dir_all(media_dir).map_err(|source| StorageError::Io {
            path: media_dir.to_path_buf(),
            source,
        })?;
        let extension = extension_for_mime(&data_url.mime);
        let hash = stable_hash(&data_url.bytes);
        for attempt in 0..=1024 {
            let filename = if attempt == 0 {
                format!("media-{hash:016x}.{extension}")
            } else {
                format!("media-{hash:016x}-{attempt}.{extension}")
            };
            let path = media_dir.join(&filename);
            if path.exists() {
                let existing = std::fs::read(&path).map_err(|source| StorageError::Io {
                    path: path.clone(),
                    source,
                })?;
                if existing == data_url.bytes {
                    return Ok(filename);
                }
                continue;
            }
            std::fs::write(&path, &data_url.bytes)
                .map_err(|source| StorageError::Io { path, source })?;
            return Ok(filename);
        }
        Err(StorageError::Backend(
            "could not allocate a unique media filename".to_string(),
        ))
    }

    fn stable_hash(bytes: &[u8]) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    fn extension_for_mime(mime: &str) -> &'static str {
        match mime {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "image/svg+xml" => "svg",
            "video/mp4" => "mp4",
            "video/webm" => "webm",
            "audio/mpeg" => "mp3",
            "audio/mp3" => "mp3",
            "audio/wav" | "audio/x-wav" => "wav",
            "audio/ogg" => "ogg",
            "model/gltf-binary" => "glb",
            "model/gltf+json" => "gltf",
            _ => "bin",
        }
    }

    fn mime_from_path(path: &Path) -> &'static str {
        match path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "ogg" => "audio/ogg",
            "glb" => "model/gltf-binary",
            "gltf" => "model/gltf+json",
            _ => "application/octet-stream",
        }
    }

    fn safe_project_child(root: &Path, value: &str) -> Result<PathBuf> {
        let relative = Path::new(value);
        if value.trim().is_empty() || relative.is_absolute() {
            return Err(StorageError::Backend(format!(
                "project path `{value}` must be relative"
            )));
        }

        let mut clean = PathBuf::new();
        for component in relative.components() {
            match component {
                Component::Normal(part) => clean.push(part),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(StorageError::Backend(format!(
                        "project path `{value}` must stay inside the project directory"
                    )));
                }
            }
        }
        if clean.as_os_str().is_empty() {
            return Err(StorageError::Backend(format!(
                "project path `{value}` must name a file or directory"
            )));
        }
        Ok(root.join(clean))
    }
}

#[cfg(feature = "web")]
pub mod web {
    use super::{
        ProviderConfigSnapshot, ProviderConfigStorage, Result, StorageError, WorkflowSnapshot,
        WorkflowStorage, normalize_slot,
    };
    use gemed_core::WorkflowFile;
    use gemed_providers::ProviderConfigSet;

    const STORAGE_PREFIX: &str = "gemed.workflow.";
    const PROVIDER_CONFIG_STORAGE_PREFIX: &str = "gemed.providerConfig.";

    #[derive(Debug, Clone, Default)]
    pub struct WebLocalStorage {
        namespace: String,
    }

    impl WebLocalStorage {
        pub fn new() -> Self {
            Self {
                namespace: STORAGE_PREFIX.to_string(),
            }
        }

        pub fn with_namespace(namespace: impl Into<String>) -> Self {
            Self {
                namespace: namespace.into(),
            }
        }

        fn key(&self, slot: &str) -> Result<String> {
            Ok(format!("{}{}", self.namespace, normalize_slot(slot)?))
        }

        fn provider_config_key(&self, slot: &str) -> Result<String> {
            Ok(format!(
                "{}{}",
                PROVIDER_CONFIG_STORAGE_PREFIX,
                normalize_slot(slot)?
            ))
        }

        fn storage(&self) -> Result<web_sys::Storage> {
            let window = web_sys::window().ok_or_else(|| {
                StorageError::Backend("browser window is unavailable".to_string())
            })?;
            window
                .local_storage()
                .map_err(|err| {
                    StorageError::Backend(format!("localStorage access failed: {err:?}"))
                })?
                .ok_or_else(|| StorageError::Backend("localStorage is unavailable".to_string()))
        }
    }

    impl WorkflowStorage for WebLocalStorage {
        fn save_workflow(
            &mut self,
            slot: &str,
            workflow: &WorkflowFile,
        ) -> Result<WorkflowSnapshot> {
            let snapshot = WorkflowSnapshot::from_workflow(normalize_slot(slot)?, workflow)?;
            let key = self.key(&snapshot.slot)?;
            self.storage()?
                .set_item(&key, &snapshot.json)
                .map_err(|err| {
                    StorageError::Backend(format!("localStorage save failed: {err:?}"))
                })?;
            Ok(snapshot)
        }

        fn load_workflow(&self, slot: &str) -> Result<WorkflowFile> {
            let key = self.key(slot)?;
            let json = self
                .storage()?
                .get_item(&key)
                .map_err(|err| StorageError::Backend(format!("localStorage load failed: {err:?}")))?
                .ok_or_else(|| {
                    StorageError::NotFound(
                        normalize_slot(slot).unwrap_or_else(|_| slot.to_string()),
                    )
                })?;
            Ok(WorkflowFile::from_json_str(&json)?)
        }

        fn list_workflows(&self) -> Result<Vec<WorkflowSnapshot>> {
            let storage = self.storage()?;
            let length = storage.length().map_err(|err| {
                StorageError::Backend(format!("localStorage length failed: {err:?}"))
            })?;
            let mut snapshots = Vec::new();
            for index in 0..length {
                let Some(key) = storage.key(index).map_err(|err| {
                    StorageError::Backend(format!("localStorage key failed: {err:?}"))
                })?
                else {
                    continue;
                };
                if !key.starts_with(&self.namespace) {
                    continue;
                }
                let Some(json) = storage.get_item(&key).map_err(|err| {
                    StorageError::Backend(format!("localStorage load failed: {err:?}"))
                })?
                else {
                    continue;
                };
                let workflow = WorkflowFile::from_json_str(&json)?;
                snapshots.push(WorkflowSnapshot {
                    slot: key[self.namespace.len()..].to_string(),
                    name: workflow.name,
                    json,
                });
            }
            snapshots.sort_by(|left, right| left.slot.cmp(&right.slot));
            Ok(snapshots)
        }

        fn delete_workflow(&mut self, slot: &str) -> Result<()> {
            let key = self.key(slot)?;
            self.storage()?.remove_item(&key).map_err(|err| {
                StorageError::Backend(format!("localStorage delete failed: {err:?}"))
            })
        }
    }

    impl ProviderConfigStorage for WebLocalStorage {
        fn save_provider_config(
            &mut self,
            slot: &str,
            config: &ProviderConfigSet,
        ) -> Result<ProviderConfigSnapshot> {
            let snapshot = ProviderConfigSnapshot::from_config(slot, config)?;
            let key = self.provider_config_key(&snapshot.slot)?;
            self.storage()?
                .set_item(&key, &snapshot.json)
                .map_err(|err| {
                    StorageError::Backend(format!(
                        "localStorage provider config save failed: {err:?}"
                    ))
                })?;
            Ok(snapshot)
        }

        fn load_provider_config(&self, slot: &str) -> Result<ProviderConfigSet> {
            let key = self.provider_config_key(slot)?;
            let json = self
                .storage()?
                .get_item(&key)
                .map_err(|err| {
                    StorageError::Backend(format!(
                        "localStorage provider config load failed: {err:?}"
                    ))
                })?
                .ok_or_else(|| {
                    StorageError::NotFound(
                        normalize_slot(slot).unwrap_or_else(|_| slot.to_string()),
                    )
                })?;
            ProviderConfigSnapshot {
                slot: normalize_slot(slot)?,
                json,
            }
            .parse()
        }

        fn list_provider_configs(&self) -> Result<Vec<ProviderConfigSnapshot>> {
            let storage = self.storage()?;
            let length = storage.length().map_err(|err| {
                StorageError::Backend(format!(
                    "localStorage provider config length failed: {err:?}"
                ))
            })?;
            let mut snapshots = Vec::new();
            for index in 0..length {
                let Some(key) = storage.key(index).map_err(|err| {
                    StorageError::Backend(format!(
                        "localStorage provider config key failed: {err:?}"
                    ))
                })?
                else {
                    continue;
                };
                if !key.starts_with(PROVIDER_CONFIG_STORAGE_PREFIX) {
                    continue;
                }
                let Some(json) = storage.get_item(&key).map_err(|err| {
                    StorageError::Backend(format!(
                        "localStorage provider config load failed: {err:?}"
                    ))
                })?
                else {
                    continue;
                };
                let snapshot = ProviderConfigSnapshot {
                    slot: key[PROVIDER_CONFIG_STORAGE_PREFIX.len()..].to_string(),
                    json,
                };
                let _ = snapshot.parse()?;
                snapshots.push(snapshot);
            }
            snapshots.sort_by(|left, right| left.slot.cmp(&right.slot));
            Ok(snapshots)
        }

        fn delete_provider_config(&mut self, slot: &str) -> Result<()> {
            let key = self.provider_config_key(slot)?;
            self.storage()?.remove_item(&key).map_err(|err| {
                StorageError::Backend(format!(
                    "localStorage provider config delete failed: {err:?}"
                ))
            })
        }
    }
}

fn normalize_slot(slot: &str) -> Result<String> {
    let slot = slot.trim();
    if slot.is_empty() {
        return Err(StorageError::EmptySlot);
    }

    let mut normalized = String::with_capacity(slot.len());
    for ch in slot.chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => normalized.push(ch),
            _ => normalized.push('-'),
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gemed_core::WorkflowFile;
    #[cfg(feature = "desktop")]
    use gemed_core::{NodeType, Position, WorkflowNode};
    #[cfg(feature = "desktop")]
    use serde_json::json;

    #[test]
    fn memory_storage_round_trips_workflow() {
        let workflow = WorkflowFile::example();
        let mut storage = MemoryWorkflowStorage::new();

        let snapshot = storage
            .save_workflow(DEFAULT_AUTOSAVE_SLOT, &workflow)
            .expect("save workflow");
        assert_eq!(snapshot.name, workflow.name);

        let loaded = storage
            .load_workflow(DEFAULT_AUTOSAVE_SLOT)
            .expect("load workflow");
        assert_eq!(loaded.name, workflow.name);
        assert_eq!(loaded.nodes.len(), workflow.nodes.len());
    }

    #[test]
    fn memory_storage_rejects_empty_slot() {
        let workflow = WorkflowFile::example();
        let mut storage = MemoryWorkflowStorage::new();

        let err = storage
            .save_workflow("  ", &workflow)
            .expect_err("empty slot rejected");
        assert!(matches!(err, StorageError::EmptySlot));
    }

    #[test]
    fn memory_storage_lists_by_slot_order() {
        let workflow = WorkflowFile::example();
        let mut storage = MemoryWorkflowStorage::new();
        storage.save_workflow("z-last", &workflow).unwrap();
        storage.save_workflow("a-first", &workflow).unwrap();

        let slots: Vec<String> = storage
            .list_workflows()
            .unwrap()
            .into_iter()
            .map(|snapshot| snapshot.slot)
            .collect();

        assert_eq!(slots, vec!["a-first", "z-last"]);
    }

    #[test]
    fn memory_provider_config_round_trips_without_secret_values() {
        let config = ProviderConfigSet::desktop_env_defaults();
        let mut storage = MemoryWorkflowStorage::new();

        let snapshot = storage
            .save_provider_config(DEFAULT_PROVIDER_CONFIG_SLOT, &config)
            .expect("save provider config");

        assert_eq!(snapshot.slot, DEFAULT_PROVIDER_CONFIG_SLOT);
        assert!(snapshot.json.contains("GEMINI_API_KEY"));
        assert!(!snapshot.json.contains("sk-live-secret"));
        assert_eq!(
            storage
                .load_provider_config(DEFAULT_PROVIDER_CONFIG_SLOT)
                .expect("load provider config"),
            config
        );
    }

    #[test]
    fn provider_config_snapshot_rejects_unsupported_version() {
        let snapshot = ProviderConfigSnapshot {
            slot: DEFAULT_PROVIDER_CONFIG_SLOT.to_string(),
            json: serde_json::json!({
                "version": 99,
                "providerConfig": ProviderConfigSet::mock_all()
            })
            .to_string(),
        };

        let err = snapshot.parse().expect_err("version rejected");
        assert!(matches!(err, StorageError::Backend(message) if message.contains("not supported")));
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_storage_round_trips_at_explicit_directory() {
        let workflow = WorkflowFile::example();
        let unique = format!(
            "gemed-storage-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::env::temp_dir().join(unique);
        let mut storage = desktop::DesktopWorkflowStorage::at_dir(&root);

        storage.save_workflow("test", &workflow).unwrap();
        let loaded = storage.load_workflow("test").unwrap();
        assert_eq!(loaded.name, workflow.name);
        assert!(storage.autosave_path().unwrap().ends_with("autosave.json"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_provider_config_storage_round_trips_at_explicit_directory() {
        let config = ProviderConfigSet::desktop_env_defaults();
        let root = unique_temp_dir("gemed-provider-config-test");
        let mut storage = desktop::DesktopWorkflowStorage::at_dir(&root);

        let snapshot = storage
            .save_provider_config(DEFAULT_PROVIDER_CONFIG_SLOT, &config)
            .expect("save provider config");

        assert_eq!(snapshot.slot, DEFAULT_PROVIDER_CONFIG_SLOT);
        assert_eq!(
            storage.default_provider_config_path().unwrap(),
            root.join(PROVIDER_CONFIG_DIR)
                .join(format!("{DEFAULT_PROVIDER_CONFIG_SLOT}.json"))
        );
        assert!(storage.default_provider_config_path().unwrap().is_file());
        assert_eq!(
            storage
                .load_provider_config(DEFAULT_PROVIDER_CONFIG_SLOT)
                .expect("load provider config"),
            config
        );
        assert_eq!(
            storage
                .list_provider_configs()
                .expect("list provider configs")
                .into_iter()
                .map(|snapshot| snapshot.slot)
                .collect::<Vec<_>>(),
            vec![DEFAULT_PROVIDER_CONFIG_SLOT]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_project_bundle_saves_manifest_workflow_and_media_dir() {
        let workflow = WorkflowFile::example();
        let root = unique_temp_dir("gemed-project-save-test");
        let project = desktop::DesktopWorkflowProject::at_dir(&root);

        let snapshot = project.save(&workflow).expect("save project");

        assert_eq!(snapshot.root, root);
        assert_eq!(snapshot.manifest.name, workflow.name);
        assert_eq!(snapshot.manifest.workflow_file, PROJECT_WORKFLOW_FILE);
        assert_eq!(snapshot.manifest.media_dir, PROJECT_MEDIA_DIR);
        assert!(root.join(PROJECT_MANIFEST_FILE).is_file());
        assert!(root.join(PROJECT_WORKFLOW_FILE).is_file());
        assert!(root.join(PROJECT_MEDIA_DIR).is_dir());

        let loaded = project.load().expect("load project");
        assert_eq!(loaded.workflow.name, workflow.name);
        assert_eq!(loaded.workflow.nodes.len(), workflow.nodes.len());

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_project_bundle_load_rejects_manifest_parent_paths() {
        let root = unique_temp_dir("gemed-project-path-test");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(PROJECT_MANIFEST_FILE),
            serde_json::to_string_pretty(&WorkflowProjectManifest {
                version: PROJECT_SCHEMA_VERSION,
                name: "bad".to_string(),
                workflow_file: "../workflow.json".to_string(),
                media_dir: PROJECT_MEDIA_DIR.to_string(),
                media_files: Vec::new(),
            })
            .unwrap(),
        )
        .unwrap();

        let err = desktop::DesktopWorkflowProject::at_dir(&root)
            .load()
            .expect_err("unsafe manifest path rejected");
        assert!(
            matches!(err, StorageError::Backend(message) if message.contains("inside the project directory"))
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_project_bundle_externalizes_and_hydrates_data_url_media() {
        let media = "data:image/png;base64,aGVsbG8=";
        let mut workflow = WorkflowFile::blank();
        workflow.name = "media bundle".to_string();
        workflow.nodes.push(WorkflowNode::new(
            "image",
            NodeType::ImageInput,
            Position { x: 0.0, y: 0.0 },
            json!({
                "image": media,
                "nested": {
                    "duplicate": media
                }
            }),
        ));
        let root = unique_temp_dir("gemed-project-media-test");
        let project = desktop::DesktopWorkflowProject::at_dir(&root);

        let snapshot = project.save(&workflow).expect("save project media");

        assert_eq!(snapshot.manifest.media_files.len(), 1);
        let media_file = root.join(&snapshot.manifest.media_files[0]);
        assert!(media_file.is_file());
        assert_eq!(std::fs::read(&media_file).unwrap(), b"hello");
        let saved_json = std::fs::read_to_string(root.join(PROJECT_WORKFLOW_FILE)).unwrap();
        assert!(!saved_json.contains("data:image/png"));
        assert!(saved_json.contains(PROJECT_MEDIA_URL_PREFIX));
        let saved_value: serde_json::Value = serde_json::from_str(&saved_json).unwrap();
        let saved_data = &saved_value["nodes"][0]["data"];
        assert_eq!(saved_data["image"], serde_json::Value::Null);
        assert!(
            saved_data["imageRef"]
                .as_str()
                .unwrap()
                .starts_with(PROJECT_MEDIA_URL_PREFIX)
        );
        assert!(
            saved_data["nested"]["duplicate"]
                .as_str()
                .unwrap()
                .starts_with(PROJECT_MEDIA_URL_PREFIX)
        );

        let loaded = project.load().expect("load project media");
        assert_eq!(loaded.workflow.nodes[0].data["image"], media);
        assert_eq!(loaded.workflow.nodes[0].data["nested"]["duplicate"], media);

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_project_bundle_removes_stale_manifest_media_files_on_save() {
        let mut workflow = WorkflowFile::blank();
        workflow.name = "media gc".to_string();
        workflow.nodes.push(WorkflowNode::new(
            "image",
            NodeType::ImageInput,
            Position { x: 0.0, y: 0.0 },
            json!({
                "image": "data:image/png;base64,aGVsbG8="
            }),
        ));
        let root = unique_temp_dir("gemed-project-media-gc-test");
        let project = desktop::DesktopWorkflowProject::at_dir(&root);

        let first = project.save(&workflow).expect("first project save");
        let stale_file = root.join(&first.manifest.media_files[0]);
        assert!(stale_file.is_file());
        let untracked_file = root.join(PROJECT_MEDIA_DIR).join("user-kept.bin");
        std::fs::write(&untracked_file, b"user data").unwrap();

        workflow.nodes[0].data["image"] = json!("data:image/png;base64,Ynll");
        let second = project.save(&workflow).expect("second project save");

        assert_ne!(first.manifest.media_files, second.manifest.media_files);
        assert!(!stale_file.exists());
        assert!(root.join(&second.manifest.media_files[0]).is_file());
        assert!(untracked_file.is_file());

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn desktop_project_bundle_externalizes_and_hydrates_known_media_arrays() {
        let media = "data:image/png;base64,aGVsbG8=";
        let mut workflow = WorkflowFile::blank();
        workflow.name = "media array bundle".to_string();
        workflow.nodes.push(WorkflowNode::new(
            "generate",
            NodeType::NanoBanana,
            Position { x: 0.0, y: 0.0 },
            json!({
                "inputImages": [media, "https://example.invalid/image.png"],
                "inputImageRefs": ["", ""]
            }),
        ));
        let root = unique_temp_dir("gemed-project-media-array-test");
        let project = desktop::DesktopWorkflowProject::at_dir(&root);

        let snapshot = project.save(&workflow).expect("save project media array");

        assert_eq!(snapshot.manifest.media_files.len(), 1);
        let saved_json = std::fs::read_to_string(root.join(PROJECT_WORKFLOW_FILE)).unwrap();
        let saved_value: serde_json::Value = serde_json::from_str(&saved_json).unwrap();
        let saved_data = &saved_value["nodes"][0]["data"];
        assert_eq!(saved_data["inputImages"][0], "");
        assert_eq!(
            saved_data["inputImages"][1],
            "https://example.invalid/image.png"
        );
        assert!(
            saved_data["inputImageRefs"][0]
                .as_str()
                .unwrap()
                .starts_with(PROJECT_MEDIA_URL_PREFIX)
        );

        let loaded = project.load().expect("load project media array");
        assert_eq!(loaded.workflow.nodes[0].data["inputImages"][0], media);
        assert_eq!(
            loaded.workflow.nodes[0].data["inputImages"][1],
            "https://example.invalid/image.png"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(feature = "desktop")]
    fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
        let unique = format!(
            "{}-{}",
            prefix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        std::env::temp_dir().join(unique)
    }
}
