//! Backend-independent conversation state. No protocol or GUI types belong here.

use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Telegram's signed dialog ID, which distinguishes users, groups, and channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChatId(pub i64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chat {
    pub id: ChatId,
    pub title: String,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub chat: ChatId,
    pub id: i32,
    pub text: String,
    pub sender: String,
    pub outgoing: bool,
    pub sent_at: i64,
    pub edited_at: Option<i64>,
}

impl Message {
    fn version(&self) -> i64 {
        self.edited_at.unwrap_or(self.sent_at)
    }
}

pub const CACHE_LIMIT: usize = 500;

#[derive(Debug, Default)]
struct Tombstones {
    ids: BTreeSet<i32>,
    forgotten_through: Option<i32>,
    generation: u64,
}

impl Tombstones {
    fn delete(&mut self, id: i32) {
        if self.forgotten_through.is_some_and(|floor| id <= floor) {
            self.generation = self.generation.wrapping_add(1);
            return;
        }
        self.ids.insert(id);
        if self.ids.len() > CACHE_LIMIT {
            let oldest = self.ids.pop_first().unwrap();
            self.forgotten_through = Some(oldest);
            self.generation = self.generation.wrapping_add(1);
        }
    }

    fn blocks(&self, id: i32, known: bool, authoritative: bool) -> bool {
        self.ids.contains(&id)
            || (!known && !authoritative && self.forgotten_through.is_some_and(|floor| id <= floor))
    }
}

#[derive(Debug, Default)]
struct Timeline {
    messages: BTreeMap<i32, Message>,
    deleted: Tombstones,
    floor: Option<i32>,
    history_generation: Option<(u64, u64)>,
}

impl Timeline {
    fn trim(&mut self) {
        if self.messages.len() > CACHE_LIMIT {
            let (oldest, _) = self
                .messages
                .pop_first()
                .expect("timeline exceeds its limit");
            self.floor = Some(self.floor.map_or(oldest, |floor| floor.max(oldest)));
        }
    }
}

#[derive(Debug, Default)]
pub struct Conversations {
    timelines: HashMap<ChatId, Timeline>,
    global_deleted: Tombstones,
}

impl Conversations {
    /// Capture deletion generations immediately before a server request. History
    /// starts and responses must be delivered in order, without overlap per chat.
    pub fn begin_history(&mut self, chat: ChatId) {
        let timeline = self.timelines.entry(chat).or_default();
        timeline.history_generation =
            Some((self.global_deleted.generation, timeline.deleted.generation));
    }

    pub fn end_history(&mut self, chat: ChatId) {
        if let Some(timeline) = self.timelines.get_mut(&chat) {
            timeline.history_generation = None;
        }
    }

    pub fn merge_history(&mut self, chat: ChatId, messages: Vec<Message>) {
        let timeline = self.timelines.entry(chat).or_default();
        // An eviction or a deletion below a watermark invalidates that scope.
        let token = timeline.history_generation.take();
        let global_authoritative =
            token.is_some_and(|(global, _)| global == self.global_deleted.generation);
        let local_authoritative =
            token.is_some_and(|(_, local)| local == timeline.deleted.generation);
        for message in messages.into_iter().filter(|message| message.chat == chat) {
            self.insert(message, false, global_authoritative, local_authoritative);
        }
    }

    pub fn upsert(&mut self, message: Message) {
        self.insert(message, true, false, false);
    }

    fn insert(
        &mut self,
        message: Message,
        live: bool,
        global_authoritative: bool,
        local_authoritative: bool,
    ) {
        let timeline = self.timelines.entry(message.chat).or_default();
        let existing = timeline.messages.get(&message.id);
        let known = existing.is_some();
        let channel = message.chat.0 <= -1_000_000_000_000;
        let fresh = local_authoritative && (channel || global_authoritative);
        if (!known && !fresh && timeline.floor.is_some_and(|floor| message.id <= floor))
            || timeline
                .deleted
                .blocks(message.id, known, local_authoritative)
            || (!channel
                && self
                    .global_deleted
                    .blocks(message.id, known, global_authoritative))
        {
            return;
        }
        let replace = existing.is_none_or(|existing| {
            message.version() > existing.version()
                || (live && message.version() == existing.version())
        });
        if replace {
            timeline.messages.insert(message.id, message);
            timeline.trim();
        }
    }

    /// Account-wide IDs do not apply to channels.
    pub fn delete(&mut self, chat: Option<ChatId>, ids: &[i32]) {
        if let Some(chat) = chat {
            let timeline = self.timelines.entry(chat).or_default();
            for &id in ids {
                timeline.messages.remove(&id);
                timeline.deleted.delete(id);
            }
        } else {
            for &id in ids {
                self.global_deleted.delete(id);
            }
            for (chat, timeline) in &mut self.timelines {
                if chat.0 > -1_000_000_000_000 {
                    for id in ids {
                        timeline.messages.remove(id);
                    }
                }
            }
        }
    }

    pub fn get(&self, chat: ChatId, id: i32) -> Option<&Message> {
        self.timelines.get(&chat)?.messages.get(&id)
    }

    pub fn messages(&self, chat: ChatId) -> impl DoubleEndedIterator<Item = &Message> {
        self.timelines
            .get(&chat)
            .into_iter()
            .flat_map(|timeline| timeline.messages.values())
    }

    /// Create a timeline before starting a request so concurrent deletions can
    /// leave tombstones even when the history has not arrived yet.
    pub fn prepare(&mut self, chat: ChatId) {
        self.timelines.entry(chat).or_default();
    }
}

pub fn validate_text(text: &str) -> Result<(), &'static str> {
    if text.trim().is_empty() {
        Err("Write a message first.")
    } else if text.chars().count() > 4096 {
        Err("Messages can contain up to 4,096 characters.")
    } else {
        Ok(())
    }
}

pub fn normalize_phone(phone: &str) -> Result<String, &'static str> {
    let phone: String = phone
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '-' | '(' | ')'))
        .collect();
    let digits = phone.strip_prefix('+').unwrap_or_default();
    if !(5..=15).contains(&digits.len()) || !digits.bytes().all(|c| c.is_ascii_digit()) {
        Err("Enter a phone number with its country code, starting with +.")
    } else {
        Ok(phone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(chat: i64, id: i32, text: &str) -> Message {
        Message {
            chat: ChatId(chat),
            id,
            text: text.into(),
            sender: "Alice".into(),
            outgoing: false,
            sent_at: 100,
            edited_at: None,
        }
    }

    #[test]
    fn send_update_and_history_deliveries_merge_once() {
        let mut state = Conversations::default();
        state.upsert(message(1, 20, "Hello"));
        state.upsert(message(1, 20, "Hello"));
        state.merge_history(
            ChatId(1),
            vec![message(1, 19, "Earlier"), message(1, 20, "Hello")],
        );
        assert_eq!(
            state.messages(ChatId(1)).map(|m| m.id).collect::<Vec<_>>(),
            [19, 20]
        );
    }

    #[test]
    fn live_message_survives_in_flight_history() {
        let mut state = Conversations::default();
        state.upsert(message(1, 21, "Arrived during request"));
        state.merge_history(ChatId(1), vec![message(1, 20, "Older")]);
        assert_eq!(state.messages(ChatId(1)).count(), 2);
    }

    #[test]
    fn stale_history_does_not_overwrite_edits() {
        let mut state = Conversations::default();
        let mut edited = message(1, 1, "Edited");
        edited.edited_at = Some(200);
        state.upsert(edited);
        state.merge_history(ChatId(1), vec![message(1, 1, "Original")]);
        assert_eq!(state.messages(ChatId(1)).next().unwrap().text, "Edited");
    }

    #[test]
    fn message_ids_are_scoped_to_conversations() {
        let mut state = Conversations::default();
        state.upsert(message(-1_000_000_000_001, 1, "Channel one"));
        state.upsert(message(-1_000_000_000_002, 1, "Channel two"));
        state.delete(Some(ChatId(-1_000_000_000_001)), &[1]);
        assert_eq!(state.messages(ChatId(-1_000_000_000_002)).count(), 1);
    }

    #[test]
    fn deletion_before_history_does_not_resurrect_message() {
        let mut state = Conversations::default();
        state.prepare(ChatId(1));
        state.delete(None, &[1]);
        state.merge_history(ChatId(1), vec![message(1, 1, "Deleted")]);
        assert_eq!(state.messages(ChatId(1)).count(), 0);
    }

    #[test]
    fn account_wide_deletion_does_not_delete_channel_message() {
        let mut state = Conversations::default();
        state.upsert(message(1, 1, "Private"));
        state.upsert(message(-1_000_000_000_001, 1, "Channel"));
        state.delete(None, &[1]);
        assert_eq!(state.messages(ChatId(1)).count(), 0);
        assert_eq!(state.messages(ChatId(-1_000_000_000_001)).count(), 1);
    }

    #[test]
    fn bounded_sparse_tombstones_reject_evicted_history_per_chat() {
        let mut state = Conversations::default();
        state.delete(Some(ChatId(1)), &[2]);
        for id in 1..=501 {
            state.upsert(message(1, id * 10, "recent"));
        }
        state.merge_history(
            ChatId(1),
            vec![message(1, 2, "deleted"), message(1, 10, "old")],
        );
        assert_eq!(state.messages(ChatId(1)).count(), 500);
        assert!(state.get(ChatId(1), 2).is_none());
        assert!(state.get(ChatId(1), 10).is_none());
        state.upsert(message(2, 2, "other chat"));
        assert!(state.get(ChatId(2), 2).is_some());
    }

    #[test]
    fn unrelated_global_deletions_do_not_evict_or_block_chat_history() {
        let mut state = Conversations::default();
        state.prepare(ChatId(2));
        state.upsert(message(2, 5, "kept"));
        let ids: Vec<_> = (1000..1601).collect();
        state.delete(None, &ids);
        assert_eq!(state.get(ChatId(2), 5).unwrap().text, "kept");
        state.begin_history(ChatId(2));
        state.merge_history(ChatId(2), vec![message(2, 6, "history")]);
        assert!(state.get(ChatId(2), 6).is_some());
        state.merge_history(ChatId(3), vec![message(3, 1000, "deleted before opening")]);
        assert!(state.get(ChatId(3), 1000).is_none());
        state.merge_history(
            ChatId(-1_000_000_000_001),
            vec![message(-1_000_000_000_001, 1000, "channel")],
        );
        assert!(state.get(ChatId(-1_000_000_000_001), 1000).is_some());
    }

    #[test]
    fn deletion_watermark_invalidates_stale_requests_and_bounds_storage() {
        let mut state = Conversations::default();
        state.begin_history(ChatId(1));
        state.delete(None, &(1000..1501).collect::<Vec<_>>());
        assert_eq!(state.global_deleted.ids.len(), CACHE_LIMIT);
        state.merge_history(ChatId(1), vec![message(1, 7, "stale")]);
        assert!(state.get(ChatId(1), 7).is_none());
        state.begin_history(ChatId(1));
        state.merge_history(ChatId(1), vec![message(1, 7, "fresh")]);
        assert!(state.get(ChatId(1), 7).is_some());
        state.begin_history(ChatId(2));
        state.delete(None, &[8]);
        state.merge_history(ChatId(2), vec![message(2, 8, "deleted")]);
        assert!(state.get(ChatId(2), 8).is_none());
        state.delete(Some(ChatId(3)), &(2000..2501).collect::<Vec<_>>());
        assert_eq!(state.timelines[&ChatId(3)].deleted.ids.len(), CACHE_LIMIT);
    }

    #[test]
    fn fresh_history_refills_evicted_rows_and_keeps_live_edits() {
        let mut state = Conversations::default();
        for id in 1..=501 {
            state.upsert(message(1, id, "original"));
        }
        state.delete(Some(ChatId(1)), &[501]);
        state.merge_history(ChatId(1), vec![message(1, 1, "stale")]);
        assert!(state.get(ChatId(1), 1).is_none());
        state.begin_history(ChatId(1));
        state.merge_history(ChatId(1), vec![message(1, 1, "fresh")]);
        assert_eq!(state.get(ChatId(1), 1).unwrap().text, "fresh");
        let mut edited = message(1, 1, "edited");
        edited.edited_at = Some(200);
        state.upsert(edited);
        assert_eq!(state.get(ChatId(1), 1).unwrap().text, "edited");
    }

    #[test]
    fn text_validation_counts_unicode_characters() {
        assert!(validate_text(" \n ").is_err());
        assert!(validate_text(&"🦀".repeat(4096)).is_ok());
        assert!(validate_text(&"a".repeat(4097)).is_err());
    }

    #[test]
    fn phone_validation_requires_international_format() {
        assert_eq!(
            normalize_phone("+1 (555) 123-4567"),
            Ok("+15551234567".into())
        );
        assert!(normalize_phone("5551234567").is_err());
        assert!(normalize_phone("+123abc").is_err());
    }
}
