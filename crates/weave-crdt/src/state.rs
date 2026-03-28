use std::path::{Path, PathBuf};

use automerge::{AutoCommit, ObjType, ReadDoc, Value, ROOT, transaction::Transactable};

use crate::error::{Result, WeaveError};

const SCHEMA_VERSION: u64 = 2;

/// Wraps an Automerge document for entity state persistence.
///
/// Document structure (v2):
/// ```text
/// root {
///   schema_version: u64,
///   entities: Map<entity_id → {
///     name, type, file_path, content_hash,
///     claimed_by, claimed_at,
///     last_modified_by, last_modified_at,
///     version,
///     version_vector: Map<agent_id → counter>,
///     content: String,
///     base_content: String,
///     merge_state: String,
///     conflict_ours: String,
///     conflict_theirs: String,
///     conflict_base: String,
///     conflict_ours_agent: String,
///     conflict_theirs_agent: String,
///   }>,
///   agents: Map<agent_id → {
///     name, status, branch, last_seen,
///     working_on: List<entity_id>
///   }>,
///   operations: List<{agent, entity_id, op, timestamp}>,
///   file_entity_order: Map<file_path → List<entity_id>>,
///   file_interstitials: Map<position_key → String>,
/// }
/// ```
pub struct EntityStateDoc {
    pub(crate) doc: AutoCommit,
    pub(crate) path: PathBuf,
}

impl EntityStateDoc {
    /// Load from disk or create a new document.
    pub fn open(path: &Path) -> Result<Self> {
        let doc = if path.exists() {
            let data = std::fs::read(path)?;
            AutoCommit::load(&data)?
        } else {
            Self::create_new_doc()?
        };
        let mut state = Self {
            doc,
            path: path.to_path_buf(),
        };
        state.migrate_if_needed()?;
        Ok(state)
    }

    /// Create a new in-memory document (for testing).
    pub fn new_memory() -> Result<Self> {
        let doc = Self::create_new_doc()?;
        let mut state = Self {
            doc,
            path: PathBuf::new(),
        };
        state.migrate_if_needed()?;
        Ok(state)
    }

    fn create_new_doc() -> Result<AutoCommit> {
        let mut doc = AutoCommit::new();
        doc.put(ROOT, "schema_version", SCHEMA_VERSION as i64)?;
        doc.put_object(ROOT, "entities", ObjType::Map)?;
        doc.put_object(ROOT, "agents", ObjType::Map)?;
        doc.put_object(ROOT, "operations", ObjType::List)?;
        doc.put_object(ROOT, "file_entity_order", ObjType::Map)?;
        doc.put_object(ROOT, "file_interstitials", ObjType::Map)?;
        Ok(doc)
    }

    /// Migrate v1 documents to v2 schema.
    ///
    /// - Adds schema_version field
    /// - Initializes version_vector from existing version + last_modified_by
    /// - Adds empty content/base_content/merge_state fields
    /// - Creates file_entity_order and file_interstitials maps
    fn migrate_if_needed(&mut self) -> Result<()> {
        let current_version = self.get_schema_version();
        if current_version >= SCHEMA_VERSION {
            return Ok(());
        }

        // Set schema version
        self.doc.put(ROOT, "schema_version", SCHEMA_VERSION as i64)?;

        // Ensure file_entity_order exists
        if self.doc.get(ROOT, "file_entity_order")?.is_none() {
            self.doc.put_object(ROOT, "file_entity_order", ObjType::Map)?;
        }

        // Ensure file_interstitials exists
        if self.doc.get(ROOT, "file_interstitials")?.is_none() {
            self.doc.put_object(ROOT, "file_interstitials", ObjType::Map)?;
        }

        // Migrate entities: add version_vector and content fields
        let entities = self.entities_id()?;
        let entity_keys: Vec<String> = self.doc.keys(&entities).collect();

        for key in &entity_keys {
            let entity_obj = match self.doc.get(&entities, key.as_str())? {
                Some((_, id)) => id,
                None => continue,
            };

            // Create version_vector from existing version + last_modified_by
            if self.doc.get(&entity_obj, "version_vector")?.is_none() {
                let vv_obj = self.doc.put_object(&entity_obj, "version_vector", ObjType::Map)?;

                let version = match self.doc.get(&entity_obj, "version")? {
                    Some((Value::Scalar(v), _)) => match v.as_ref() {
                        automerge::ScalarValue::Uint(n) => *n,
                        automerge::ScalarValue::Int(n) => *n as u64,
                        _ => 0,
                    },
                    _ => 0,
                };

                if version > 0 {
                    let agent = match self.doc.get(&entity_obj, "last_modified_by")? {
                        Some((Value::Scalar(v), _)) => {
                            if let automerge::ScalarValue::Str(s) = v.as_ref() {
                                Some(s.to_string())
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };

                    if let Some(agent_id) = agent {
                        self.doc.put(&vv_obj, agent_id.as_str(), version as i64)?;
                    }
                }
            }

            // Add empty content fields if missing
            if self.doc.get(&entity_obj, "content")?.is_none() {
                self.doc.put(&entity_obj, "content", "")?;
            }
            if self.doc.get(&entity_obj, "base_content")?.is_none() {
                self.doc.put(&entity_obj, "base_content", "")?;
            }
            if self.doc.get(&entity_obj, "merge_state")?.is_none() {
                self.doc.put(&entity_obj, "merge_state", "clean")?;
            }
        }

        Ok(())
    }

    fn get_schema_version(&self) -> u64 {
        match self.doc.get(ROOT, "schema_version") {
            Ok(Some((Value::Scalar(v), _))) => match v.as_ref() {
                automerge::ScalarValue::Uint(n) => *n,
                automerge::ScalarValue::Int(n) => *n as u64,
                _ => 0,
            },
            _ => 0,
        }
    }

    /// Save the document to disk.
    pub fn save(&mut self) -> Result<()> {
        if self.path.as_os_str().is_empty() {
            return Ok(()); // In-memory mode, no-op
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = self.doc.save();
        std::fs::write(&self.path, data)?;
        Ok(())
    }

    /// Get the ExId of the "entities" map.
    pub(crate) fn entities_id(&self) -> Result<automerge::ObjId> {
        match self.doc.get(ROOT, "entities")? {
            Some((_, id)) => Ok(id),
            None => Err(WeaveError::Automerge(
                automerge::AutomergeError::InvalidObjId("entities map missing".into()),
            )),
        }
    }

    /// Get the ExId of the "agents" map.
    pub(crate) fn agents_id(&self) -> Result<automerge::ObjId> {
        match self.doc.get(ROOT, "agents")? {
            Some((_, id)) => Ok(id),
            None => Err(WeaveError::Automerge(
                automerge::AutomergeError::InvalidObjId("agents map missing".into()),
            )),
        }
    }

    /// Get the ExId of the "operations" list.
    pub(crate) fn operations_id(&self) -> Result<automerge::ObjId> {
        match self.doc.get(ROOT, "operations")? {
            Some((_, id)) => Ok(id),
            None => Err(WeaveError::Automerge(
                automerge::AutomergeError::InvalidObjId("operations list missing".into()),
            )),
        }
    }

    /// Get the ExId of the "file_entity_order" map.
    pub(crate) fn file_entity_order_id(&self) -> Result<automerge::ObjId> {
        match self.doc.get(ROOT, "file_entity_order")? {
            Some((_, id)) => Ok(id),
            None => Err(WeaveError::Automerge(
                automerge::AutomergeError::InvalidObjId("file_entity_order map missing".into()),
            )),
        }
    }

    /// Get the ExId of the "file_interstitials" map.
    pub(crate) fn file_interstitials_id(&self) -> Result<automerge::ObjId> {
        match self.doc.get(ROOT, "file_interstitials")? {
            Some((_, id)) => Ok(id),
            None => Err(WeaveError::Automerge(
                automerge::AutomergeError::InvalidObjId("file_interstitials map missing".into()),
            )),
        }
    }
}

/// Get current time in milliseconds since epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
