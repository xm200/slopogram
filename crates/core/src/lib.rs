//! Backend-independent conversation state. No protocol or GUI types belong here.

use std::collections::{BTreeMap, HashMap, HashSet};

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

#[derive(Debug, Default)]
struct Timeline {
    messages: BTreeMap<i32, Message>,
    deleted: HashSet<i32>,
}

#[derive(Debug, Default)]
pub struct Conversations {
    timelines: HashMap<ChatId, Timeline>,
}

impl Conversations {
    /// History is a potentially stale snapshot: never replace an equally recent
    /// live message, or resurrect a message deleted while the request was running.
    pub fn merge_history(&mut self, chat: ChatId, messages: Vec<Message>) {
        for message in messages.into_iter().filter(|message| message.chat == chat) {
            self.insert(message, false);
        }
    }

    pub fn upsert(&mut self, message: Message) {
        self.insert(message, true);
    }

    fn insert(&mut self, message: Message, live: bool) {
        let timeline = self.timelines.entry(message.chat).or_default();
        if timeline.deleted.contains(&message.id) {
            return;
        }
        let replace = timeline.messages.get(&message.id).is_none_or(|existing| {
            message.version() > existing.version()
                || (live && message.version() == existing.version())
        });
        if replace {
            timeline.messages.insert(message.id, message);
        }
    }

    /// Non-channel deletion IDs are unique within the account. Channel deletions
    /// carry an explicit conversation; never apply those to another channel.
    pub fn delete(&mut self, chat: Option<ChatId>, ids: &[i32]) {
        if let Some(chat) = chat {
            let timeline = self.timelines.entry(chat).or_default();
            for id in ids {
                timeline.messages.remove(id);
                timeline.deleted.insert(*id);
            }
        } else {
            for (chat, timeline) in &mut self.timelines {
                // Channel dialog IDs are <= -1_000_000_000_000.
                if chat.0 > -1_000_000_000_000 {
                    for id in ids {
                        timeline.messages.remove(id);
                        timeline.deleted.insert(*id);
                    }
                }
            }
        }
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
