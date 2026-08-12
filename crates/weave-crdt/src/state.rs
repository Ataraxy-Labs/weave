use std::path::{Path, PathBuf};

use automerge::{
    transaction::Transactable, AutoCommit, AutomergeError, ObjId, ObjType, ReadDoc, ScalarValue,
    Value, ROOT,
};

use crate::error::{Result, WeaveError};

const SCHEMA_VERSION: u64 = 4;

/// Wraps an Automerge document for entity state persistence.
///
/// Document structure (v4):
/// ```text
/// root {
///   schema_version: u64,
///   entities: Map<entity_id → {
///     name, type, file_path, content_hash,
///     claimed_by, claimed_at,                    -- advisory, off the merge path
///     last_modified_by, version,                 -- advisory, derived from the VV
///     version_vector: Map<agent_id → counter>,
///     base_content: String,
///     anchor_after: u64, anchor_tier: u64,       -- written only by `anchor::place`
///     writes: Map<agent_id → { content, deleted, hash, vv: Map<agent → counter> }>,
///   }>,
///   agents: Map<agent_id → { name, status, branch, last_seen, working_on }>,
///   operations: List<{agent, entity_id, op, timestamp}>,
///   file_interstitials: Map<position_key → String>,
///   key_index: Map<parser_key → entity_id>,
/// }
/// ```
///
/// v4 separates *identity* from *spelling*. The keys of `entities`
/// used to be the parser's own key, `file::type::name` — so an entity's name was
/// stored twice, once as data and once inside its identity, and a rename made
/// the two disagree. `EntityOp::Renamed` updated the `name` field while the map
/// key still spelled the old name, so the next `sync_from_files` looked up the
/// new key, missed, and created a SECOND entity with its own writes register and
/// anchor — the renamed entity's history orphaned on a key nobody would ask for
/// again.
///
/// `key_index` is the fix: parser keys are now *aliases* that resolve to an
/// entity id, and every door resolves through it. An entity's id is the parser
/// key it was **first seen under**, which is deliberately not random — two
/// replicas meeting the same new entity independently mint the same id and
/// converge, which is the property the old scheme had and a UUID would have
/// destroyed. After a rename the id keeps its original spelling; that spelling
/// is a historical artifact and nothing may read meaning out of it.
///
/// v3 removed the merge-relevant scalars — `content`,
/// `merge_state`, `conflict_*`. Those were a last-writer-wins register and a
/// flag beside it: two replicas writing the same entity concurrently kept one
/// body and silently dropped the other, which is the one thing a CRDT may not
/// do. Content now lives in `writes`, one entry per agent, and the entity's
/// value is *derived* by joining the causally-maximal entries through
/// `weave_core::entity_merge`. There is no slot left for a writer to win.
///
/// `file_entity_order` is gone too. It was a per-file list replaced
/// wholesale on every sync — a last-writer-wins list holding a fact that
/// `anchor_after`/`anchor_tier` already hold convergently, kept only as a
/// fallback for documents written before anchors existed. Every door now places
/// every entity it creates, so there is nothing left for it to fall back to.
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

    /// Create a new in-memory document. A test door: no product path opens a
    /// document that is never written to disk, so it is gated behind
    /// `test-helpers` the same way `set_entity_conflict` is.
    #[cfg(any(test, feature = "test-helpers"))]
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
        doc.put_object(ROOT, "file_interstitials", ObjType::Map)?;
        doc.put_object(ROOT, "key_index", ObjType::Map)?;
        Ok(doc)
    }

    /// Bring an older document up to the v3 shape.
    ///
    /// Structural only, and deliberately so. The previous version
    /// carried a v1→v2 clock reconstruction *and* a v2→v3 content transfer that
    /// invented a `writes` entry from the dead scalar `content` field —
    /// ~100 lines of one-shot code translating a schema that existed for a day
    /// and whose whole point was that its scalar `content` was the wrong answer
    /// under concurrency. Reconstructing a version vector from a scalar counter,
    /// in particular, fabricates causality that never happened. What survives is
    /// the part that is a *fact about the shape*: the maps a reader needs must
    /// exist, and the merge-relevant scalars must be gone, because leaving them
    /// lets a reader take the last-writer-wins answer by accident.
    fn migrate_if_needed(&mut self) -> Result<()> {
        if self.get_schema_version() >= SCHEMA_VERSION {
            return Ok(());
        }
        self.doc
            .put(ROOT, "schema_version", SCHEMA_VERSION as i64)?;
        if self.doc.get(ROOT, "file_interstitials")?.is_none() {
            self.doc
                .put_object(ROOT, "file_interstitials", ObjType::Map)?;
        }
        // v3 → v4. Structural, and the entities map is NOT rekeyed:
        // a v3 document's keys already *are* its entities' first-seen parser
        // keys, which is exactly what v4 calls an id. The index starts empty and
        // `entity_id_for` falls back to the identity mapping, so a v3 document reads
        // as itself — and two replicas migrating independently produce the same
        // document, which a rekeying migration could not promise.
        if self.doc.get(ROOT, "key_index")?.is_none() {
            self.doc.put_object(ROOT, "key_index", ObjType::Map)?;
        }

        let entities = self.entities_id()?;
        let entity_keys: Vec<String> = self.doc.keys(&entities).collect();
        for key in &entity_keys {
            let Some((_, entity_obj)) = self.doc.get(&entities, key.as_str())? else {
                continue;
            };
            if self.doc.get(&entity_obj, "version_vector")?.is_none() {
                self.doc
                    .put_object(&entity_obj, "version_vector", ObjType::Map)?;
            }
            if self.doc.get(&entity_obj, "writes")?.is_none() {
                self.doc.put_object(&entity_obj, "writes", ObjType::Map)?;
            }
            if self.doc.get(&entity_obj, "base_content")?.is_none() {
                self.doc.put(&entity_obj, "base_content", "")?;
            }
            for dead in [
                "content",
                "merge_state",
                "conflict_ours",
                "conflict_theirs",
                "conflict_base",
                "conflict_ours_agent",
                "conflict_theirs_agent",
            ] {
                if self.doc.get(&entity_obj, dead)?.is_some() {
                    self.doc.delete(&entity_obj, dead)?;
                }
            }
        }
        Ok(())
    }

    fn get_schema_version(&self) -> u64 {
        match self.doc.get(ROOT, "schema_version") {
            Ok(Some((Value::Scalar(v), _))) => match v.as_ref() {
                ScalarValue::Uint(n) => *n,
                ScalarValue::Int(n) => *n as u64,
                _ => 0,
            },
            _ => 0,
        }
    }

    /// Fork this replica: an independent document that shares the history so
    /// far. What a second agent, or a second machine, starts from.
    pub fn fork(&mut self) -> Result<Self> {
        let bytes = self.doc.save();
        Ok(Self {
            doc: AutoCommit::load(&bytes)?,
            path: PathBuf::new(),
        })
    }

    /// Fold another replica's history into this one. This is the *transport*,
    /// not the merge: automerge reconciles the document structure, and the
    /// entity values are joined on read by `join::value_of`.
    pub fn merge_replica(&mut self, other: &mut Self) -> Result<()> {
        self.doc.merge(&mut other.doc)?;
        Ok(())
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

    /// The ExId of a top-level root object, or a `WeaveError` naming what's
    /// missing. The five accessors below are this one lookup with a field
    /// name and an error message — kept as one helper rather than five copies
    /// so a schema field can't drift from its own "missing" message.
    fn root_object_id(&self, field: &str, missing: &str) -> Result<ObjId> {
        self.doc
            .get(ROOT, field)?
            .map(|(_, id)| id)
            .ok_or_else(|| WeaveError::Automerge(AutomergeError::InvalidObjId(missing.into())))
    }

    /// Get the ExId of the "entities" map.
    pub(crate) fn entities_id(&self) -> Result<ObjId> {
        self.root_object_id("entities", "entities map missing")
    }

    /// Get the ExId of the `key_index` map: parser key → entity id.
    pub(crate) fn key_index_id(&self) -> Result<ObjId> {
        self.root_object_id("key_index", "key_index map missing")
    }

    /// Get the ExId of the "agents" map.
    pub(crate) fn agents_id(&self) -> Result<ObjId> {
        self.root_object_id("agents", "agents map missing")
    }

    /// Get the ExId of the "operations" list.
    pub(crate) fn operations_id(&self) -> Result<ObjId> {
        self.root_object_id("operations", "operations list missing")
    }

    /// Get the ExId of the "file_interstitials" map.
    pub(crate) fn file_interstitials_id(&self) -> Result<ObjId> {
        self.root_object_id("file_interstitials", "file_interstitials map missing")
    }
}

/// Get current time in milliseconds since epoch.
pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document written before v4: entities keyed by their parser key, and no
    /// `key_index` at all.
    fn v3_document() -> EntityStateDoc {
        let mut state = EntityStateDoc::new_memory().unwrap();
        let entities = state.entities_id().unwrap();
        let e = state
            .doc
            .put_object(&entities, "app.py::function::alpha", ObjType::Map)
            .unwrap();
        state.doc.put(&e, "name", "alpha").unwrap();
        state.doc.put(&e, "base_content", "def alpha():\n").unwrap();
        // Wind the document back to what v3 actually looked like.
        state.doc.delete(ROOT, "key_index").unwrap();
        state.doc.put(ROOT, "schema_version", 3_i64).unwrap();
        state
    }

    /// v3 → v4 is structural, and the entities map is deliberately NOT rekeyed:
    /// a v3 key already *is* its entity's first-seen parser key, which is what
    /// v4 calls an id. So the migration adds an empty index and changes nothing
    /// else — and two replicas migrating independently produce the same
    /// document, which a rekeying migration could not promise.
    #[test]
    fn a_v3_document_migrates_to_v4_and_still_reads_as_itself() {
        let mut state = v3_document();
        assert_eq!(state.get_schema_version(), 3);

        state.migrate_if_needed().unwrap();

        assert_eq!(state.get_schema_version(), SCHEMA_VERSION);
        assert!(
            state.key_index_id().is_ok(),
            "v4 must have a key_index to resolve through"
        );
        // The entity is still there, under the same key…
        let entities = state.entities_id().unwrap();
        assert!(state
            .doc
            .get(&entities, "app.py::function::alpha")
            .unwrap()
            .is_some());
        // …and that key resolves to it, by the identity fallback rather than by
        // a recorded alias, which is what makes the empty index correct.
        assert_eq!(
            crate::identity::entity_id_for(&state, "app.py::function::alpha"),
            "app.py::function::alpha"
        );
    }

    /// Migration is idempotent: running it twice is running it once. A door that
    /// opens the same document repeatedly must not accumulate anything.
    #[test]
    fn migrating_twice_is_migrating_once() {
        let mut state = v3_document();
        state.migrate_if_needed().unwrap();
        let once = state.doc.save();
        state.migrate_if_needed().unwrap();
        assert_eq!(
            once,
            state.doc.save(),
            "the second migration wrote something"
        );
    }
}
