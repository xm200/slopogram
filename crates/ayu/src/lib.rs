pub mod theme;
mod theme_mappings;

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use slopogram_core::{ChatId, Message};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    path::PathBuf,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Version {
    pub text: String,
    pub sender: String,
    #[serde(default)]
    pub author_peer_id: Option<i64>,
    pub outgoing: bool,
    pub sent_at: i64,
    pub edited_at: Option<i64>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Entry {
    pub chat_id: i64,
    pub message_id: i32,
    pub deleted: bool,
    #[serde(default)]
    pub global_id_eligible: bool,
    pub versions: Vec<Version>,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Policy {
    pub suppress_read_receipts: bool,
    pub suppress_typing: bool,
    pub suppress_online_status: bool,
    pub archive_observed_messages: bool,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            suppress_read_receipts: false,
            suppress_typing: false,
            suppress_online_status: false,
            archive_observed_messages: true,
        }
    }
}
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalCosmetics {
    pub local_premium: bool,
    pub emoji_status: Option<String>,
    pub profile_color: Option<u32>,
    pub chat_emoji: Vec<LocalChatEmoji>,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalChatEmoji {
    pub trigger: String,
    pub file_id: i64,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Data {
    entries: BTreeMap<String, Entry>,
    exact_tombstones: BTreeSet<String>,
    global_tombstones: BTreeSet<i32>,
    remote_exact: BTreeSet<String>,
    remote_global: BTreeSet<i32>,
    local_exact: BTreeSet<String>,
    local_global: BTreeSet<i32>,
    policy: Policy,
    cosmetics: LocalCosmetics,
}

const MAX_EVENTS: usize = 4096;

#[derive(Serialize)]
enum BorrowedSnapshot<'a> {
    Snapshot(&'a Data),
}

#[derive(Serialize, Deserialize)]
enum Operation {
    Snapshot(Data),
    Observe {
        chat: i64,
        id: i32,
        version: Version,
        eligible: bool,
    },
    Delete {
        chat: i64,
        ids: Vec<i32>,
    },
    DeleteGlobal(Vec<i32>),
    Remote {
        chat: i64,
        ids: Vec<i32>,
    },
    RemoteGlobal(Vec<i32>),
    Local {
        chat: i64,
        ids: Vec<i32>,
    },
    LocalGlobal(Vec<i32>),
    Policy(Policy),
    Cosmetics(LocalCosmetics),
}

pub struct Journal {
    connection: rusqlite::Connection,
    data: Data,
    pending: Vec<Operation>,
    replaying: bool,
    replace: bool,
    persisted_events: usize,
    _lock: fs::File,
}
impl Journal {
    pub fn open(directory: PathBuf) -> Result<Self, String> {
        if directory.is_symlink() {
            return Err("Journal directory cannot be a symbolic link".into());
        }
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(&directory)
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err("Journal directory must be private".into());
            }
        }
        let lock_path = directory.join("observed-history.lock");
        if lock_path.is_symlink() {
            return Err("Journal lock cannot be a symbolic link".into());
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(lock_path).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            lock.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
        }
        lock.try_lock_exclusive()
            .map_err(|_| "Local history is already open".to_string())?;
        let legacy = directory.join("observed-history.json");
        let path = directory.join("observed-history.sqlite");
        for candidate in [
            &legacy,
            &path,
            &directory.join("observed-history.sqlite-wal"),
            &directory.join("observed-history.sqlite-shm"),
        ] {
            if candidate.is_symlink() {
                return Err("Journal file cannot be a symbolic link".into());
            }
        }
        let exists = path.exists();
        let legacy_data: Option<Data> = if !exists {
            match fs::read(&legacy) {
                Ok(bytes) => Some(serde_json::from_slice(&bytes).map_err(|_| {
                    "Cannot read local history; existing file preserved".to_string()
                })?),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.to_string()),
            }
        } else {
            None
        };
        if !exists {
            let temporary =
                tempfile::NamedTempFile::new_in(&directory).map_err(|e| e.to_string())?;
            {
                let mut initial =
                    rusqlite::Connection::open(temporary.path()).map_err(|e| e.to_string())?;
                let tx = initial.transaction().map_err(|e| e.to_string())?;
                tx.execute_batch("CREATE TABLE events (sequence INTEGER PRIMARY KEY, payload TEXT NOT NULL); CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL); PRAGMA user_version=1;").map_err(|e| e.to_string())?;
                if let Some(mut data) = legacy_data {
                    data.policy.archive_observed_messages = true;
                    let payload = serde_json::to_string(&Operation::Snapshot(data))
                        .map_err(|e| e.to_string())?;
                    tx.execute("INSERT INTO events(payload) VALUES (?1)", [&payload])
                        .map_err(|e| e.to_string())?;
                    tx.execute(
                        "INSERT INTO metadata(key,value) VALUES ('legacy_migrated','1')",
                        [],
                    )
                    .map_err(|e| e.to_string())?;
                }
                tx.commit().map_err(|e| e.to_string())?;
            }
            temporary.as_file().sync_all().map_err(|e| e.to_string())?;
            temporary
                .persist_noclobber(&path)
                .map_err(|e| e.error.to_string())?;
            #[cfg(unix)]
            fs::File::open(&directory)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(&path)
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err("Journal file must be private".into());
            }
        }
        let connection = rusqlite::Connection::open(&path).map_err(|e| e.to_string())?;
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|e| e.to_string())?;
        if version != 1 {
            return Err("Cannot open local history; existing database preserved".into());
        }
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA secure_delete=ON;",
            )
            .map_err(|e| e.to_string())?;
        let migrated: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM metadata WHERE key='legacy_migrated')",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if legacy.exists() && !migrated {
            return Err("Both local history files exist; neither was removed".into());
        }
        let operations: Vec<Operation> = {
            let mut statement = connection
                .prepare("SELECT payload FROM events ORDER BY sequence")
                .map_err(|e| e.to_string())?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            let mut operations = Vec::new();
            for row in rows {
                operations.push(
                    serde_json::from_str(&row.map_err(|e| e.to_string())?).map_err(|_| {
                        "Cannot read local history; existing database preserved".to_string()
                    })?,
                );
            }
            operations
        };
        let persisted_events = operations.len();
        let mut journal = Self {
            connection,
            data: Data::default(),
            pending: Vec::new(),
            replaying: true,
            replace: false,
            persisted_events,
            _lock: lock,
        };
        for operation in operations {
            journal.apply(operation)?;
        }
        Self::validate_local_cosmetics(&journal.data.cosmetics)?;
        journal.replaying = false;
        journal.data.policy.archive_observed_messages = true;
        if legacy.exists() {
            fs::remove_file(&legacy)
                .map_err(|e| format!("Cannot complete local history migration: {e}"))?;
            #[cfg(unix)]
            fs::File::open(&directory)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
        }
        if migrated {
            journal
                .connection
                .execute("DELETE FROM metadata WHERE key='legacy_migrated'", [])
                .map_err(|e| e.to_string())?;
        }
        if journal.persisted_events > MAX_EVENTS {
            journal.save()?;
        }
        Ok(journal)
    }
    fn append(&mut self, operation: Operation) {
        if !self.replaying {
            self.pending.push(operation);
        }
    }
    fn apply(&mut self, operation: Operation) -> Result<(), String> {
        match operation {
            Operation::Snapshot(data) => self.data = data,
            Operation::Observe {
                chat,
                id,
                version,
                eligible,
            } => {
                self.observe(
                    &Message {
                        chat: ChatId(chat),
                        id,
                        text: version.text,
                        sender: version.sender,
                        outgoing: version.outgoing,
                        sent_at: version.sent_at,
                        edited_at: version.edited_at,
                    },
                    eligible,
                    version.author_peer_id,
                );
            }
            Operation::Delete { chat, ids } => {
                self.delete(ChatId(chat), &ids);
            }
            Operation::DeleteGlobal(ids) => {
                self.delete_global(&ids);
            }
            Operation::Remote { chat, ids } => {
                self.remote_delete(ChatId(chat), &ids);
            }
            Operation::RemoteGlobal(ids) => {
                self.remote_delete_global(&ids);
            }
            Operation::Local { chat, ids } => {
                self.local_delete(ChatId(chat), &ids);
            }
            Operation::LocalGlobal(ids) => {
                self.local_delete_global(&ids);
            }
            Operation::Policy(policy) => self.set_policy(policy),
            Operation::Cosmetics(value) => self.set_local_cosmetics(value)?,
        }
        Ok(())
    }
    fn key(chat: i64, id: i32) -> String {
        format!("{chat}:{id}")
    }
    pub fn observe(
        &mut self,
        message: &Message,
        global_id_eligible: bool,
        author_peer_id: Option<i64>,
    ) -> bool {
        let key = Self::key(message.chat.0, message.id);
        let deleted = self.data.exact_tombstones.contains(&key)
            || (global_id_eligible && self.data.global_tombstones.contains(&message.id))
            || self.data.local_exact.contains(&key)
            || (global_id_eligible && self.data.local_global.contains(&message.id));
        let entry = self.data.entries.entry(key).or_insert_with(|| Entry {
            chat_id: message.chat.0,
            message_id: message.id,
            deleted,
            global_id_eligible,
            versions: Vec::new(),
        });
        let eligibility_changed = global_id_eligible && !entry.global_id_eligible;
        entry.global_id_eligible |= global_id_eligible;
        if global_id_eligible
            && (self.data.global_tombstones.contains(&message.id)
                || self.data.local_global.contains(&message.id))
        {
            entry.deleted = true;
        }
        let version = Version {
            text: message.text.clone(),
            sender: message.sender.clone(),
            author_peer_id,
            outgoing: message.outgoing,
            sent_at: message.sent_at,
            edited_at: message.edited_at,
        };
        if let Some(existing) = entry.versions.iter_mut().find(|v| {
            v.text == version.text
                && v.sender == version.sender
                && v.outgoing == version.outgoing
                && v.sent_at == version.sent_at
                && v.edited_at == version.edited_at
        }) {
            if existing.author_peer_id.is_none() && version.author_peer_id.is_some() {
                existing.author_peer_id = version.author_peer_id;
                self.append(Operation::Observe {
                    chat: message.chat.0,
                    id: message.id,
                    version,
                    eligible: global_id_eligible,
                });
                return true;
            }
        } else {
            entry.versions.push(version.clone());
            entry
                .versions
                .sort_by_key(|v| v.edited_at.unwrap_or(v.sent_at));
            self.append(Operation::Observe {
                chat: message.chat.0,
                id: message.id,
                version,
                eligible: global_id_eligible,
            });
            return true;
        }
        if eligibility_changed {
            self.append(Operation::Observe {
                chat: message.chat.0,
                id: message.id,
                version,
                eligible: global_id_eligible,
            });
        }
        eligibility_changed
    }
    pub fn delete_global(&mut self, ids: &[i32]) -> bool {
        let mut changed = false;
        for id in ids {
            changed |= self.data.global_tombstones.insert(*id);
        }
        for entry in self.data.entries.values_mut() {
            if entry.global_id_eligible && ids.contains(&entry.message_id) {
                changed |= !entry.deleted;
                entry.deleted = true;
            }
        }
        if changed {
            self.append(Operation::DeleteGlobal(ids.to_vec()));
        }
        changed
    }
    pub fn delete(&mut self, chat: ChatId, ids: &[i32]) -> bool {
        let mut changed = false;
        for id in ids {
            changed |= self.data.exact_tombstones.insert(Self::key(chat.0, *id));
        }
        for entry in self.data.entries.values_mut() {
            if ids.contains(&entry.message_id) && entry.chat_id == chat.0 {
                changed |= !entry.deleted;
                entry.deleted = true;
            }
        }
        if changed {
            self.append(Operation::Delete {
                chat: chat.0,
                ids: ids.to_vec(),
            });
        }
        changed
    }
    pub fn remote_delete(&mut self, chat: ChatId, ids: &[i32]) -> bool {
        let mut changed = self.delete(chat, ids);
        for id in ids {
            changed |= self.data.remote_exact.insert(Self::key(chat.0, *id));
        }
        if changed {
            self.append(Operation::Remote {
                chat: chat.0,
                ids: ids.to_vec(),
            });
        }
        changed
    }
    pub fn remote_delete_global(&mut self, ids: &[i32]) -> bool {
        let mut changed = self.delete_global(ids);
        for id in ids {
            changed |= self.data.remote_global.insert(*id);
        }
        if changed {
            self.append(Operation::RemoteGlobal(ids.to_vec()));
        }
        changed
    }
    pub fn local_delete(&mut self, chat: ChatId, ids: &[i32]) -> bool {
        let mut changed = false;
        for id in ids {
            let key = Self::key(chat.0, *id);
            {
                changed |= self.data.local_exact.insert(key.clone());
                if let Some(entry) = self.data.entries.get_mut(&key) {
                    changed |= !entry.deleted;
                    entry.deleted = true;
                }
            }
        }
        if changed {
            self.append(Operation::Local {
                chat: chat.0,
                ids: ids.to_vec(),
            });
        }
        changed
    }
    pub fn local_delete_global(&mut self, ids: &[i32]) -> bool {
        let mut changed = false;
        for id in ids {
            changed |= self.data.local_global.insert(*id);
            for entry in self.data.entries.values_mut() {
                if entry.message_id == *id && entry.global_id_eligible {
                    changed |= !entry.deleted;
                    entry.deleted = true;
                }
            }
        }
        if changed {
            self.append(Operation::LocalGlobal(ids.to_vec()));
        }
        changed
    }
    pub fn policy(&self) -> Policy {
        self.data.policy
    }
    pub fn set_policy(&mut self, policy: Policy) {
        let effective = Policy {
            archive_observed_messages: true,
            ..policy
        };
        if self.data.policy != effective {
            self.data.policy = effective;
            self.append(Operation::Policy(effective));
        }
    }
    pub fn local_cosmetics(&self) -> LocalCosmetics {
        self.data.cosmetics.clone()
    }
    fn validate_local_cosmetics(value: &LocalCosmetics) -> Result<(), String> {
        if value.profile_color.is_some_and(|color| color > 0x00ff_ffff) {
            return Err("Profile color must be an RGB value".into());
        }
        if value.chat_emoji.len() > 32 {
            return Err("Too many local emoji mappings".into());
        }
        let mut seen = BTreeSet::new();
        for mapping in &value.chat_emoji {
            use unicode_segmentation::UnicodeSegmentation;
            if mapping.file_id <= 0
                || mapping.trigger.len() > 64
                || mapping.trigger.trim().is_empty()
                || mapping.trigger.chars().any(char::is_control)
                || mapping.trigger.graphemes(true).count() != 1
                || !seen.insert(&mapping.trigger)
            {
                return Err("Invalid local emoji mapping".into());
            }
        }
        if let Some(status) = &value.emoji_status
            && (status.is_empty()
                || status.len() > 64
                || status.chars().count() > 16
                || status.chars().any(char::is_control)
                || status.trim().is_empty())
        {
            return Err("Emoji status must be a short, nonempty Unicode string".into());
        }
        Ok(())
    }
    pub fn set_local_cosmetics(&mut self, value: LocalCosmetics) -> Result<(), String> {
        Self::validate_local_cosmetics(&value)?;
        if self.data.cosmetics != value {
            self.data.cosmetics = value.clone();
            self.append(Operation::Cosmetics(value));
        }
        Ok(())
    }
    pub fn entries(&self, chat: i64) -> Vec<Entry> {
        self.select_entries(chat, false)
    }
    fn select_entries(&self, chat: i64, deleted_only: bool) -> Vec<Entry> {
        let prefix = format!("{chat}:");
        let mut entries: Vec<_> = self
            .data
            .entries
            .range(prefix.clone()..)
            .take_while(|(key, _)| key.starts_with(&prefix))
            .map(|(_, entry)| entry)
            .filter(|entry| !deleted_only || (entry.deleted && !entry.versions.is_empty()))
            .cloned()
            .collect();
        for entry in &mut entries {
            entry
                .versions
                .sort_by_key(|version| version.edited_at.unwrap_or(version.sent_at));
        }
        entries.sort_by_key(|entry| {
            (
                entry
                    .versions
                    .first()
                    .map_or(i64::MIN, |version| version.sent_at),
                entry.message_id,
            )
        });
        entries
    }
    pub fn deleted_entries(&self, chat: i64) -> Vec<Entry> {
        self.select_entries(chat, true)
            .into_iter()
            .filter(|entry| {
                let key = Self::key(entry.chat_id, entry.message_id);
                let remote = self.data.remote_exact.contains(&key)
                    || (entry.global_id_eligible
                        && self.data.remote_global.contains(&entry.message_id));
                let local = self.data.local_exact.contains(&key)
                    || (entry.global_id_eligible
                        && self.data.local_global.contains(&entry.message_id));
                remote && !local
            })
            .collect()
    }
    pub fn clear(&mut self) {
        self.data.entries.clear();
        self.data.exact_tombstones.clear();
        self.data.global_tombstones.clear();
        self.data.remote_exact.clear();
        self.data.remote_global.clear();
        self.data.local_exact.clear();
        self.data.local_global.clear();
        self.pending.clear();
        self.replace = true;
    }
    pub fn save(&mut self) -> Result<(), String> {
        let snapshot =
            self.replace || self.persisted_events.saturating_add(self.pending.len()) > MAX_EVENTS;
        if self.pending.is_empty() && !snapshot {
            return Ok(());
        }
        let tx = self.connection.transaction().map_err(|e| e.to_string())?;
        if snapshot {
            let payload = serde_json::to_string(&BorrowedSnapshot::Snapshot(&self.data))
                .map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM events", [])
                .map_err(|e| e.to_string())?;
            tx.execute("INSERT INTO events(payload) VALUES (?1)", [&payload])
                .map_err(|e| e.to_string())?;
        } else {
            for operation in &self.pending {
                let payload = serde_json::to_string(operation).map_err(|e| e.to_string())?;
                tx.execute("INSERT INTO events(payload) VALUES (?1)", [&payload])
                    .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
        self.persisted_events = if snapshot {
            1
        } else {
            self.persisted_events + self.pending.len()
        };
        self.pending.clear();
        let checkpoint = self.replace;
        self.replace = false;
        if checkpoint {
            self.connection
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn msg(chat: i64, id: i32, text: &str, edited_at: Option<i64>) -> Message {
        Message {
            chat: ChatId(chat),
            id,
            text: text.into(),
            sender: "A".into(),
            outgoing: false,
            sent_at: 1,
            edited_at,
        }
    }
    #[test]
    fn preserves_observed_versions_and_deletions_across_reopen() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("private");
        let mut j = Journal::open(dir.clone()).unwrap();
        assert!(j.policy().archive_observed_messages);
        j.set_policy(Policy {
            archive_observed_messages: true,
            suppress_read_receipts: true,
            ..Policy::default()
        });
        j.observe(&msg(1, 7, "first", None), false, None);
        j.observe(&msg(1, 7, "edit", Some(3)), false, None);
        j.observe(&msg(1, 7, "stale", Some(2)), false, None);
        j.observe(&msg(2, 7, "other", None), false, None);
        j.delete(ChatId(1), &[7]);
        j.observe(&msg(1, 7, "resurrect", Some(4)), false, None);
        j.save().unwrap();
        assert!(Journal::open(dir.clone()).is_err());
        drop(j);
        let mut j = Journal::open(dir.clone()).unwrap();
        let one = j.entries(1);
        assert_eq!(one.len(), 1);
        assert!(one[0].deleted);
        assert_eq!(
            one[0]
                .versions
                .iter()
                .map(|v| v.text.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "stale", "edit", "resurrect"]
        );
        assert!(!j.entries(2)[0].deleted);
        assert!(j.policy().suppress_read_receipts);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(dir.join("observed-history.sqlite"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        j.clear();
        j.save().unwrap();
        drop(j);
        assert!(Journal::open(dir).unwrap().entries(1).is_empty());
    }
    #[test]
    fn event_log_compacts_and_reopens() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("journal");
        let mut journal = Journal::open(dir.clone()).unwrap();
        journal.set_policy(Policy {
            suppress_typing: true,
            ..Policy::default()
        });
        journal.observe(&msg(1, 1, "first", None), false, None);
        journal.observe(&msg(1, 1, "edited", Some(2)), false, None);
        journal.delete(ChatId(1), &[1]);
        journal.save().unwrap();
        for id in 2..=4093 {
            journal.observe(&msg(1, id, "retained", None), false, None);
        }
        journal.save().unwrap();
        assert_eq!(journal.persisted_events, MAX_EVENTS);
        journal.observe(&msg(1, 5000, "last", None), false, None);
        journal
            .connection
            .execute_batch(
                "CREATE TRIGGER reject_compaction BEFORE INSERT ON events
             BEGIN SELECT RAISE(ABORT, 'injected'); END;",
            )
            .unwrap();
        assert!(journal.save().is_err());
        let count = |j: &Journal| {
            j.connection
                .query_row("SELECT COUNT(*) FROM events", [], |row| {
                    row.get::<_, usize>(0)
                })
                .unwrap()
        };
        assert_eq!(count(&journal), MAX_EVENTS);
        journal
            .connection
            .execute_batch("DROP TRIGGER reject_compaction")
            .unwrap();
        journal.save().unwrap();
        assert_eq!(count(&journal), 1);
        drop(journal);
        let journal = Journal::open(dir).unwrap();
        assert_eq!(count(&journal), 1);
        assert!(journal.policy().suppress_typing);
        let entry = journal
            .entries(1)
            .into_iter()
            .find(|entry| entry.message_id == 1)
            .unwrap();
        assert!(entry.deleted);
        assert_eq!(entry.versions.len(), 2);
        assert!(
            journal
                .entries(1)
                .iter()
                .any(|entry| entry.message_id == 5000)
        );
    }

    #[test]
    fn oversized_existing_log_compacts_on_open() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("journal");
        let mut journal = Journal::open(dir.clone()).unwrap();
        journal.observe(&msg(1, 1, "retained", None), false, None);
        journal.save().unwrap();
        journal
            .connection
            .execute_batch(
                "WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x < 4096)
             INSERT INTO events(payload) SELECT (SELECT payload FROM events LIMIT 1) FROM n;",
            )
            .unwrap();
        drop(journal);
        let journal = Journal::open(dir).unwrap();
        let count: usize = journal
            .connection
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(journal.entries(1)[0].versions[0].text, "retained");
    }
    #[test]
    fn corrupt_journal_is_not_reset() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("observed-history.json");
        fs::write(&path, b"corrupt").unwrap();
        assert!(Journal::open(root.path().to_owned()).is_err());
        assert_eq!(fs::read(path).unwrap(), b"corrupt");
    }
    #[test]
    fn default_capture_duplicate_and_delete_scope() {
        let root = tempfile::tempdir().unwrap();
        let mut j = Journal::open(root.path().join("private")).unwrap();
        assert!(j.observe(&msg(1, 9, "off", None), false, None));
        j.set_policy(Policy {
            archive_observed_messages: true,
            ..Policy::default()
        });
        assert!(j.observe(&msg(1, 9, "one", None), false, None));
        assert!(!j.observe(&msg(1, 9, "one", None), false, None));
        assert!(j.observe(&msg(-1_000_000_000_001, 9, "channel", None), false, None));
        assert!(j.delete(ChatId(1), &[9]));
        assert!(!j.delete(ChatId(1), &[9]));
        assert!(j.entries(1)[0].deleted);
        assert!(!j.entries(-1_000_000_000_001)[0].deleted);
    }
    #[test]
    fn global_deletion_respects_explicit_eligibility_and_legacy_defaults() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("private");
        let mut journal = Journal::open(dir.clone()).unwrap();
        journal.set_policy(Policy {
            archive_observed_messages: true,
            ..Policy::default()
        });
        journal.observe(&msg(4_294_967_297, 9, "user", None), true, None);
        journal.observe(&msg(8_589_934_593, 9, "channel", None), false, None);
        journal.save().unwrap();
        drop(journal);
        let mut journal = Journal::open(dir.clone()).unwrap();
        assert!(journal.delete_global(&[9]));
        assert!(journal.entries(4_294_967_297)[0].deleted);
        assert!(!journal.entries(8_589_934_593)[0].deleted);
        let mut json = serde_json::to_value(&journal.data).unwrap();
        json["global_tombstones"] = serde_json::json!([]);
        for item in json["entries"].as_object_mut().unwrap().values_mut() {
            item.as_object_mut().unwrap().remove("global_id_eligible");
            item["deleted"] = false.into();
        }
        drop(journal);
        let legacy_dir = root.path().join("legacy");
        fs::create_dir(&legacy_dir).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&legacy_dir, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::write(
            legacy_dir.join("observed-history.json"),
            serde_json::to_vec(&json).unwrap(),
        )
        .unwrap();
        let mut old = Journal::open(legacy_dir).unwrap();
        assert!(old.delete_global(&[9]));
        assert!(!old.entries(4_294_967_297)[0].deleted);
    }
    #[test]
    fn deletions_before_observation_survive_reopen_and_clear() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("private");
        let mut j = Journal::open(dir.clone()).unwrap();
        assert!(j.delete(ChatId(5), &[20]));
        assert!(j.delete_global(&[21]));
        j.set_policy(Policy {
            archive_observed_messages: true,
            ..Policy::default()
        });
        assert!(!j.delete(ChatId(5), &[20]));
        assert!(!j.delete_global(&[21]));
        j.save().unwrap();
        drop(j);
        let mut j = Journal::open(dir.clone()).unwrap();
        assert!(j.observe(&msg(5, 20, "late", None), true, None));
        assert!(j.entries(5)[0].deleted);
        assert!(j.observe(&msg(6, 21, "eligible", None), true, None));
        assert!(j.entries(6)[0].deleted);
        assert!(j.observe(&msg(8_589_934_593, 21, "channel", None), false, None));
        assert!(!j.entries(8_589_934_593)[0].deleted);
        assert!(j.observe(&msg(7, 20, "other", None), true, None));
        assert!(!j.entries(7)[0].deleted);
        j.clear();
        j.save().unwrap();
        drop(j);
        let mut j = Journal::open(dir).unwrap();
        assert!(j.observe(&msg(5, 20, "after clear", None), true, None));
        assert!(!j.entries(5)[0].deleted);
    }
    #[test]
    fn history_orders_by_time_then_numeric_message_id() {
        let root = tempfile::tempdir().unwrap();
        let mut j = Journal::open(root.path().join("private")).unwrap();
        j.set_policy(Policy {
            archive_observed_messages: true,
            ..Policy::default()
        });
        j.observe(&msg(1, 10, "ten", None), true, None);
        j.observe(&msg(1, 2, "two", None), true, None);
        let mut earlier = msg(1, 11, "earlier", None);
        earlier.sent_at = 0;
        j.observe(&earlier, true, None);
        assert_eq!(
            j.entries(1)
                .iter()
                .map(|entry| entry.message_id)
                .collect::<Vec<_>>(),
            vec![11, 2, 10]
        );
    }
    #[test]
    fn late_observed_versions_remain_deleted_in_any_delivery_order() {
        let root = tempfile::tempdir().unwrap();
        for (index, order) in [[0, 1, 2], [2, 1, 0], [1, 2, 0]].iter().enumerate() {
            let dir = root.path().join(index.to_string());
            let mut j = Journal::open(dir.clone()).unwrap();
            j.set_policy(Policy {
                archive_observed_messages: true,
                ..Policy::default()
            });
            for action in order {
                match action {
                    0 => {
                        j.observe(&msg(1, 7, "original", None), true, None);
                    }
                    1 => {
                        j.observe(&msg(1, 7, "edited", Some(3)), true, None);
                    }
                    _ => {
                        j.delete(ChatId(1), &[7]);
                    }
                }
            }
            j.observe(&msg(1, 7, "original", None), true, None);
            j.save().unwrap();
            drop(j);
            let j = Journal::open(dir).unwrap();
            let entries = j.entries(1);
            assert!(entries[0].deleted);
            assert_eq!(
                entries[0]
                    .versions
                    .iter()
                    .map(|v| v.text.as_str())
                    .collect::<Vec<_>>(),
                vec!["original", "edited"]
            );
        }
    }
    #[test]
    fn deleted_projection_retains_author_versions_and_isolates_collisions() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("private");
        let mut j = Journal::open(dir.clone()).unwrap();
        j.set_policy(Policy {
            archive_observed_messages: true,
            ..Policy::default()
        });
        j.observe(&msg(1, 7, "before", None), true, None);
        j.observe(&msg(1, 7, "before", None), true, Some(42));
        j.observe(&msg(1, 7, "edit", Some(3)), true, Some(42));
        j.observe(&msg(2, 7, "other", None), false, Some(99));
        assert!(j.deleted_entries(1).is_empty());
        j.remote_delete_global(&[7]);
        assert!(j.deleted_entries(2).is_empty());
        j.save().unwrap();
        drop(j);
        let j = Journal::open(dir).unwrap();
        let entries = j.deleted_entries(1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].versions.len(), 2);
        assert_eq!(entries[0].versions[0].author_peer_id, Some(42));
        assert_eq!(entries[0].versions[1].text, "edit");
    }
    #[test]
    fn legacy_versions_without_author_remain_readable() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("private");
        let mut j = Journal::open(dir.clone()).unwrap();
        j.set_policy(Policy {
            archive_observed_messages: true,
            ..Policy::default()
        });
        j.observe(&msg(1, 7, "old", None), true, None);
        j.delete(ChatId(1), &[7]);
        let mut json = serde_json::to_value(&j.data).unwrap();
        json["entries"]["1:7"]["versions"][0]
            .as_object_mut()
            .unwrap()
            .remove("author_peer_id");
        json["policy"]["archive_observed_messages"] = false.into();
        drop(j);
        let legacy_dir = root.path().join("legacy");
        fs::create_dir(&legacy_dir).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&legacy_dir, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::write(
            legacy_dir.join("observed-history.json"),
            serde_json::to_vec(&json).unwrap(),
        )
        .unwrap();
        let legacy = Journal::open(legacy_dir.clone()).unwrap();
        assert!(legacy.policy().archive_observed_messages);
        assert!(!legacy_dir.join("observed-history.json").exists());
        assert!(legacy.deleted_entries(1).is_empty());
        assert_eq!(legacy.entries(1)[0].versions[0].author_peer_id, None);
    }
    #[test]
    fn local_intent_wins_over_remote_echo_in_all_orders_and_after_restart() {
        let root = tempfile::tempdir().unwrap();
        for (i, order) in [[0, 1, 2], [1, 0, 2], [2, 0, 1], [2, 1, 0]]
            .iter()
            .enumerate()
        {
            let dir = root.path().join(i.to_string());
            let mut j = Journal::open(dir.clone()).unwrap();
            j.set_policy(Policy {
                archive_observed_messages: true,
                ..Policy::default()
            });
            for action in order {
                match action {
                    0 => {
                        j.local_delete(ChatId(5), &[7]);
                    }
                    1 => {
                        j.remote_delete(ChatId(5), &[7]);
                    }
                    _ => {
                        j.observe(&msg(5, 7, "text", None), true, Some(42));
                    }
                }
            }
            assert!(j.deleted_entries(5).is_empty());
            assert!(j.entries(5)[0].deleted);
            j.save().unwrap();
            drop(j);
            assert!(Journal::open(dir).unwrap().deleted_entries(5).is_empty());
        }
    }
    #[test]
    fn remote_projection_requires_proven_origin_and_respects_disabled_local_intent() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("private");
        let mut j = Journal::open(dir.clone()).unwrap();
        j.set_policy(Policy {
            archive_observed_messages: true,
            ..Policy::default()
        });
        j.observe(&msg(4_294_967_297, 7, "user", None), true, Some(1));
        j.observe(&msg(8_589_934_593, 7, "channel", None), false, Some(2));
        j.delete(ChatId(4_294_967_297), &[7]);
        assert!(j.deleted_entries(4_294_967_297).is_empty());
        j.remote_delete_global(&[7]);
        assert_eq!(j.deleted_entries(4_294_967_297).len(), 1);
        assert!(j.deleted_entries(8_589_934_593).is_empty());
        j.set_policy(Policy::default());
        assert!(j.local_delete_global(&[7]));
        j.save().unwrap();
        drop(j);
        let mut j = Journal::open(dir).unwrap();
        j.set_policy(Policy {
            archive_observed_messages: true,
            ..Policy::default()
        });
        j.remote_delete_global(&[7]);
        assert!(j.deleted_entries(4_294_967_297).is_empty());
        j.remote_delete(ChatId(8_589_934_593), &[7]);
        assert_eq!(j.deleted_entries(8_589_934_593).len(), 1);
    }
    #[test]
    fn migrates_legacy_false_and_retains_origin_and_local_hidden() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let mut old = Journal::open(source).unwrap();
        old.set_policy(Policy {
            suppress_typing: true,
            ..Policy::default()
        });
        old.observe(&msg(1, 7, "old", None), true, Some(42));
        old.remote_delete(ChatId(1), &[7]);
        old.local_delete(ChatId(1), &[7]);
        old.data.policy.archive_observed_messages = false;
        let bytes = serde_json::to_vec(&old.data).unwrap();
        drop(old);
        let dir = root.path().join("legacy");
        fs::create_dir(&dir).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::write(dir.join("observed-history.json"), bytes).unwrap();
        let mut journal = Journal::open(dir.clone()).unwrap();
        assert!(journal.policy().archive_observed_messages && journal.policy().suppress_typing);
        assert_eq!(journal.entries(1)[0].versions[0].author_peer_id, Some(42));
        assert!(journal.deleted_entries(1).is_empty());
        assert!(!dir.join("observed-history.json").exists());
        journal.observe(&msg(2, 8, "new", None), true, None);
        journal.save().unwrap();
        drop(journal);
        let j = Journal::open(dir).unwrap();
        assert_eq!(j.entries(2)[0].versions[0].text, "new");
        assert!(j.deleted_entries(1).is_empty());
    }
    #[test]
    fn duplicate_is_no_write_and_clear_keeps_capture_active() {
        let root = tempfile::tempdir().unwrap();
        let mut j = Journal::open(root.path().join("private")).unwrap();
        assert!(j.observe(&msg(1, 1, "one", None), true, None));
        assert!(j.observe(&msg(1, 2, "two", None), true, None));
        j.save().unwrap();
        let count: i64 = j
            .connection
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);
        assert!(!j.observe(&msg(1, 1, "one", None), true, None));
        j.save().unwrap();
        assert_eq!(
            j.connection
                .query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            count
        );
        j.clear();
        j.save().unwrap();
        assert!(j.policy().archive_observed_messages);
        assert!(j.observe(&msg(1, 3, "after", None), true, None));
        j.save().unwrap();
        assert_eq!(j.entries(1)[0].versions[0].text, "after");
    }
    #[test]
    fn conflicting_legacy_and_database_are_both_preserved() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("private");
        drop(Journal::open(dir.clone()).unwrap());
        let legacy = dir.join("observed-history.json");
        fs::write(&legacy, b"unrelated archive").unwrap();
        assert!(Journal::open(dir.clone()).is_err());
        assert_eq!(fs::read(legacy).unwrap(), b"unrelated archive");
        assert!(dir.join("observed-history.sqlite").exists());
    }
    #[test]
    fn corrupt_database_is_not_reinitialized() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("private");
        fs::create_dir(&dir).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let path = dir.join("observed-history.sqlite");
        fs::write(&path, b"corrupt").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert!(Journal::open(dir).is_err());
        assert_eq!(fs::read(path).unwrap(), b"corrupt");
    }
    #[test]
    fn conversation_range_isolates_prefixes_and_preserves_order() {
        let root = tempfile::tempdir().unwrap();
        let mut j = Journal::open(root.path().join("private")).unwrap();
        for chat in [1, 10, -1, -10, -1_000_000_000_001] {
            j.observe(&msg(chat, 10, "ten", None), chat > 0, None);
            j.observe(&msg(chat, 2, "two", None), chat > 0, None);
            j.remote_delete(ChatId(chat), &[2]);
        }
        for chat in [1, 10, -1, -10, -1_000_000_000_001] {
            let entries = j.entries(chat);
            assert_eq!(
                entries
                    .iter()
                    .map(|entry| entry.message_id)
                    .collect::<Vec<_>>(),
                vec![2, 10]
            );
            assert!(entries.iter().all(|entry| entry.chat_id == chat));
            let deleted = j.deleted_entries(chat);
            assert_eq!(deleted.len(), 1);
            assert_eq!((deleted[0].chat_id, deleted[0].message_id), (chat, 2));
        }
    }
    #[test]
    fn local_cosmetics_are_isolated_persistent_and_survive_history_clear() {
        let root = tempfile::tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        let mut a = Journal::open(first.clone()).unwrap();
        assert!(a.local_cosmetics() == LocalCosmetics::default());
        let value = LocalCosmetics {
            local_premium: true,
            emoji_status: Some("👩‍💻".into()),
            profile_color: Some(0x123456),
            chat_emoji: vec![
                LocalChatEmoji {
                    trigger: "⭐".into(),
                    file_id: 111,
                },
                LocalChatEmoji {
                    trigger: "👩‍💻".into(),
                    file_id: 222,
                },
            ],
        };
        a.set_local_cosmetics(value.clone()).unwrap();
        a.save().unwrap();
        a.clear();
        a.save().unwrap();
        drop(a);
        let a = Journal::open(first).unwrap();
        assert!(a.local_cosmetics() == value);
        let b = Journal::open(second).unwrap();
        assert!(b.local_cosmetics() == LocalCosmetics::default());
    }
    #[test]
    fn invalid_cosmetics_do_not_replace_saved_state() {
        let root = tempfile::tempdir().unwrap();
        let mut j = Journal::open(root.path().join("private")).unwrap();
        let value = LocalCosmetics {
            local_premium: true,
            emoji_status: Some("⭐".into()),
            profile_color: None,
            chat_emoji: Vec::new(),
        };
        j.set_local_cosmetics(value.clone()).unwrap();
        j.save().unwrap();
        for status in [
            "".to_string(),
            " ".to_string(),
            "x\ny".to_string(),
            "x".repeat(65),
        ] {
            assert!(
                j.set_local_cosmetics(LocalCosmetics {
                    local_premium: false,
                    emoji_status: Some(status),
                    profile_color: None,
                    chat_emoji: Vec::new(),
                })
                .is_err()
            );
        }
        assert!(j.local_cosmetics() == value);
    }
    #[test]
    fn old_cosmetics_events_default_new_fields_when_replayed() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("private");
        let j = Journal::open(dir.clone()).unwrap();
        let old = r#"{"Cosmetics":{"local_premium":true,"emoji_status":"⭐"}}"#;
        j.connection
            .execute("INSERT INTO events(payload) VALUES (?1)", [old])
            .unwrap();
        drop(j);
        let j = Journal::open(dir).unwrap();
        let value = j.local_cosmetics();
        assert!(value.local_premium);
        assert_eq!(value.emoji_status.as_deref(), Some("⭐"));
        assert_eq!(value.profile_color, None);
        assert!(value.chat_emoji.is_empty());
    }
    #[test]
    fn invalid_rgb_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let mut j = Journal::open(root.path().join("private")).unwrap();
        assert!(
            j.set_local_cosmetics(LocalCosmetics {
                profile_color: Some(0x0100_0000),
                ..LocalCosmetics::default()
            })
            .is_err()
        );
        assert!(j.local_cosmetics() == LocalCosmetics::default());
    }
    #[test]
    fn chat_emoji_mappings_validate_persist_and_remove_independently() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("private");
        let mut journal = Journal::open(dir.clone()).unwrap();
        let star = LocalChatEmoji {
            trigger: "⭐".into(),
            file_id: 1,
        };
        let coder = LocalChatEmoji {
            trigger: "👩‍💻".into(),
            file_id: 2,
        };
        let mut value = LocalCosmetics {
            chat_emoji: vec![star.clone(), coder.clone()],
            ..LocalCosmetics::default()
        };
        journal.set_local_cosmetics(value.clone()).unwrap();
        journal.save().unwrap();
        drop(journal);
        let mut journal = Journal::open(dir.clone()).unwrap();
        assert!(journal.local_cosmetics() == value);
        value.chat_emoji.remove(0);
        journal.set_local_cosmetics(value.clone()).unwrap();
        journal.save().unwrap();
        let invalid = [
            vec![coder.clone(), coder],
            vec![LocalChatEmoji {
                trigger: "ab".into(),
                file_id: 1,
            }],
            vec![LocalChatEmoji {
                trigger: "⭐".into(),
                file_id: 0,
            }],
            (0..33)
                .map(|i| LocalChatEmoji {
                    trigger: format!("{i}"),
                    file_id: 1,
                })
                .collect(),
        ];
        for mappings in invalid {
            assert!(
                journal
                    .set_local_cosmetics(LocalCosmetics {
                        chat_emoji: mappings,
                        ..LocalCosmetics::default()
                    })
                    .is_err()
            );
        }
        drop(journal);
        assert!(Journal::open(dir).unwrap().local_cosmetics() == value);
    }
}
