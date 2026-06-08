use gemed_core::{WorkflowError, WorkflowFile};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const DEFAULT_AUTOSAVE_SLOT: &str = "autosave";
pub const PROJECT_SCHEMA_VERSION: u8 = 1;
pub const PROJECT_MANIFEST_FILE: &str = "gemed-project.json";
pub const PROJECT_WORKFLOW_FILE: &str = "workflow.json";
pub const PROJECT_MEDIA_DIR: &str = "media";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowProjectManifest {
    pub version: u8,
    pub name: String,
    pub workflow_file: String,
    pub media_dir: String,
}

impl WorkflowProjectManifest {
    pub fn from_workflow(workflow: &WorkflowFile) -> Self {
        Self {
            version: PROJECT_SCHEMA_VERSION,
            name: workflow.name.clone(),
            workflow_file: PROJECT_WORKFLOW_FILE.to_string(),
            media_dir: PROJECT_MEDIA_DIR.to_string(),
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

#[derive(Debug, Clone, Default)]
pub struct MemoryWorkflowStorage {
    snapshots: BTreeMap<String, WorkflowSnapshot>,
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

#[cfg(feature = "desktop")]
pub mod desktop {
    use super::{
        DEFAULT_AUTOSAVE_SLOT, PROJECT_MANIFEST_FILE, PROJECT_MEDIA_DIR, PROJECT_WORKFLOW_FILE,
        Result, StorageError, WorkflowProjectManifest, WorkflowSnapshot, WorkflowStorage,
        normalize_slot,
    };
    use directories::ProjectDirs;
    use gemed_core::WorkflowFile;
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

        fn path_for_slot(&self, slot: &str) -> Result<PathBuf> {
            Ok(self.root.join(format!("{}.json", normalize_slot(slot)?)))
        }

        fn ensure_root(&self) -> Result<()> {
            std::fs::create_dir_all(&self.root).map_err(|source| StorageError::Io {
                path: self.root.clone(),
                source,
            })
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

            let manifest = WorkflowProjectManifest::from_workflow(workflow);
            let workflow_json = workflow.to_pretty_json()?;
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
                }
            };

            let workflow_path = safe_project_child(&self.root, &manifest.workflow_file)?;
            let json =
                std::fs::read_to_string(&workflow_path).map_err(|source| StorageError::Io {
                    path: workflow_path.clone(),
                    source,
                })?;
            let workflow = WorkflowFile::from_json_str(&json)?;
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
    use super::{Result, StorageError, WorkflowSnapshot, WorkflowStorage, normalize_slot};
    use gemed_core::WorkflowFile;

    const STORAGE_PREFIX: &str = "gemed.workflow.";

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
