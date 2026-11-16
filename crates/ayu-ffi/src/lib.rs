use slopogram_core::{ChatId, Message};
use std::{path::PathBuf, sync::Arc};
uniffi::setup_scaffolding!();
#[derive(uniffi::Record)]
pub struct AyuMessage {
    pub chat_id: i64,
    pub id: i32,
    pub text: String,
    pub sender: String,
    pub outgoing: bool,
    pub sent_at: i64,
    pub edited_at: Option<i64>,
    pub global_id_eligible: bool,
    pub author_peer_id: Option<i64>,
}
#[derive(Debug, uniffi::Error)]
pub enum EngineError {
    Failed { message: String },
}
impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed { message } => f.write_str(message),
        }
    }
}
impl std::error::Error for EngineError {}
fn error(message: impl ToString) -> EngineError {
    EngineError::Failed {
        message: message.to_string(),
    }
}
#[derive(uniffi::Record)]
pub struct FeaturePolicy {
    pub suppress_read_receipts: bool,
    pub suppress_typing: bool,
    pub suppress_online_status: bool,
    pub archive_observed_messages: bool,
}
#[derive(uniffi::Record)]
pub struct LocalCosmetics {
    pub local_premium: bool,
    pub emoji_status: Option<String>,
    pub profile_color: Option<u32>,
    pub chat_emoji: Vec<LocalChatEmoji>,
}
#[derive(uniffi::Record)]
pub struct LocalChatEmoji {
    pub trigger: String,
    pub file_id: i64,
}
#[derive(uniffi::Record)]
pub struct ArchivedVersion {
    pub text: String,
    pub sender: String,
    pub author_peer_id: Option<i64>,
    pub outgoing: bool,
    pub sent_at: i64,
    pub edited_at: Option<i64>,
}
#[derive(uniffi::Record)]
pub struct ArchivedMessage {
    pub chat_id: i64,
    pub message_id: i32,
    pub deleted: bool,
    pub versions: Vec<ArchivedVersion>,
}

#[derive(uniffi::Object)]
pub struct AyuFeatures {
    directory: PathBuf,
    journal: Arc<std::sync::Mutex<Option<slopogram_ayu::Journal>>>,
}
impl AyuFeatures {
    async fn run<T, F>(&self, action: F) -> Result<T, EngineError>
    where
        T: Send + 'static,
        F: FnOnce(&mut slopogram_ayu::Journal) -> Result<T, String> + Send + 'static,
    {
        let directory = self.directory.clone();
        let journal = self.journal.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let result = (|| {
                let mut guard = journal
                    .lock()
                    .map_err(|_| "Local history unavailable".to_string())?;
                if guard.is_none() {
                    *guard = Some(slopogram_ayu::Journal::open(directory)?);
                }
                let result = action(guard.as_mut().ok_or("Local history unavailable")?);
                if result.is_err() {
                    *guard = None;
                }
                result
            })();
            let _ = tx.send(result);
        });
        rx.await.map_err(error)?.map_err(error)
    }
}
impl AyuFeatures {
    async fn project_history(
        &self,
        chat_id: i64,
        deleted_only: bool,
    ) -> Result<Vec<ArchivedMessage>, EngineError> {
        self.run(move |j| {
            Ok((if deleted_only {
                j.deleted_entries(chat_id)
            } else {
                j.entries(chat_id)
            })
            .into_iter()
            .map(|e| ArchivedMessage {
                chat_id: e.chat_id,
                message_id: e.message_id,
                deleted: e.deleted,
                versions: e
                    .versions
                    .into_iter()
                    .map(|v| ArchivedVersion {
                        text: v.text,
                        sender: v.sender,
                        author_peer_id: v.author_peer_id,
                        outgoing: v.outgoing,
                        sent_at: v.sent_at,
                        edited_at: v.edited_at,
                    })
                    .collect(),
            })
            .collect())
        })
        .await
    }
}

#[uniffi::export]
impl AyuFeatures {
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Arc<Self> {
        Arc::new(Self {
            directory: PathBuf::from(data_dir),
            journal: Arc::new(std::sync::Mutex::new(None)),
        })
    }
    pub async fn policy(&self) -> Result<FeaturePolicy, EngineError> {
        self.run(|j| {
            let p = j.policy();
            Ok(FeaturePolicy {
                suppress_read_receipts: p.suppress_read_receipts,
                suppress_typing: p.suppress_typing,
                suppress_online_status: p.suppress_online_status,
                archive_observed_messages: p.archive_observed_messages,
            })
        })
        .await
    }
    pub async fn set_policy(&self, policy: FeaturePolicy) -> Result<(), EngineError> {
        self.run(move |j| {
            j.set_policy(slopogram_ayu::Policy {
                suppress_read_receipts: policy.suppress_read_receipts,
                suppress_typing: policy.suppress_typing,
                suppress_online_status: policy.suppress_online_status,
                archive_observed_messages: policy.archive_observed_messages,
            });
            j.save()
        })
        .await
    }
    pub async fn local_cosmetics(&self) -> Result<LocalCosmetics, EngineError> {
        self.run(|j| {
            let value = j.local_cosmetics();
            Ok(LocalCosmetics {
                local_premium: value.local_premium,
                emoji_status: value.emoji_status,
                profile_color: value.profile_color,
                chat_emoji: value
                    .chat_emoji
                    .into_iter()
                    .map(|item| LocalChatEmoji {
                        trigger: item.trigger,
                        file_id: item.file_id,
                    })
                    .collect(),
            })
        })
        .await
    }
    pub async fn set_local_cosmetics(&self, value: LocalCosmetics) -> Result<(), EngineError> {
        self.run(move |j| {
            j.set_local_cosmetics(slopogram_ayu::LocalCosmetics {
                local_premium: value.local_premium,
                emoji_status: value.emoji_status,
                profile_color: value.profile_color,
                chat_emoji: value
                    .chat_emoji
                    .into_iter()
                    .map(|item| slopogram_ayu::LocalChatEmoji {
                        trigger: item.trigger,
                        file_id: item.file_id,
                    })
                    .collect(),
            })?;
            j.save()
        })
        .await
    }
    pub async fn observe(&self, message: AyuMessage) -> Result<(), EngineError> {
        self.observe_batch(vec![message]).await
    }
    pub async fn observe_batch(&self, messages: Vec<AyuMessage>) -> Result<(), EngineError> {
        self.run(move |j| {
            let mut changed = false;
            for message in messages {
                changed |= j.observe(
                    &Message {
                        chat: ChatId(message.chat_id),
                        id: message.id,
                        text: message.text,
                        sender: message.sender,
                        outgoing: message.outgoing,
                        sent_at: message.sent_at,
                        edited_at: message.edited_at,
                    },
                    message.global_id_eligible,
                    message.author_peer_id,
                );
            }
            if changed {
                j.save()?;
            }
            Ok(())
        })
        .await
    }
    pub async fn record_deletion(&self, chat_id: i64, ids: Vec<i32>) -> Result<(), EngineError> {
        self.run(move |j| {
            if j.delete(ChatId(chat_id), &ids) {
                j.save()?;
            }
            Ok(())
        })
        .await
    }
    pub async fn record_global_deletion(&self, ids: Vec<i32>) -> Result<(), EngineError> {
        self.run(move |j| {
            if j.delete_global(&ids) {
                j.save()?;
            }
            Ok(())
        })
        .await
    }
    pub async fn record_remote_deletion(
        &self,
        chat_id: i64,
        ids: Vec<i32>,
    ) -> Result<(), EngineError> {
        self.run(move |j| {
            if j.remote_delete(ChatId(chat_id), &ids) {
                j.save()?;
            }
            Ok(())
        })
        .await
    }
    pub async fn record_remote_global_deletion(&self, ids: Vec<i32>) -> Result<(), EngineError> {
        self.run(move |j| {
            if j.remote_delete_global(&ids) {
                j.save()?;
            }
            Ok(())
        })
        .await
    }
    pub async fn record_local_deletion(
        &self,
        chat_id: i64,
        ids: Vec<i32>,
    ) -> Result<(), EngineError> {
        self.run(move |j| {
            if j.local_delete(ChatId(chat_id), &ids) {
                j.save()?;
            }
            Ok(())
        })
        .await
    }
    pub async fn record_local_global_deletion(&self, ids: Vec<i32>) -> Result<(), EngineError> {
        self.run(move |j| {
            if j.local_delete_global(&ids) {
                j.save()?;
            }
            Ok(())
        })
        .await
    }
    pub async fn history(&self, chat_id: i64) -> Result<Vec<ArchivedMessage>, EngineError> {
        self.project_history(chat_id, false).await
    }
    pub async fn deleted_history(&self, chat_id: i64) -> Result<Vec<ArchivedMessage>, EngineError> {
        self.project_history(chat_id, true).await
    }

    pub async fn clear_history(&self) -> Result<(), EngineError> {
        self.run(|j| {
            j.clear();
            j.save()
        })
        .await
    }
}

#[cfg(test)]
mod ayu_tests {
    use super::*;
    #[tokio::test]
    async fn archive_only_records_when_enabled_and_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("private").to_str().unwrap().to_owned();
        let features = AyuFeatures::new(path.clone());
        let make = || AyuMessage {
            chat_id: 1,
            id: 3,
            text: "hello".into(),
            sender: "A".into(),
            outgoing: false,
            sent_at: 1,
            edited_at: None,
            global_id_eligible: true,
            author_peer_id: Some(42),
        };
        let initial = features.policy().await.unwrap();
        assert!(
            initial.archive_observed_messages
                && !initial.suppress_read_receipts
                && !initial.suppress_typing
                && !initial.suppress_online_status
        );
        features.observe(make()).await.unwrap();
        assert_eq!(features.history(1).await.unwrap().len(), 1);
        features
            .set_policy(FeaturePolicy {
                archive_observed_messages: true,
                suppress_read_receipts: true,
                suppress_typing: false,
                suppress_online_status: false,
            })
            .await
            .unwrap();
        features.observe(make()).await.unwrap();
        features.record_remote_deletion(1, vec![3]).await.unwrap();
        drop(features);
        let reopened = AyuFeatures::new(path);
        assert!(reopened.policy().await.unwrap().suppress_read_receipts);
        let history = reopened.history(1).await.unwrap();
        assert_eq!(history.len(), 1);
        assert!(history[0].deleted);
        assert_eq!(history[0].versions[0].text, "hello");
        assert_eq!(history[0].versions[0].author_peer_id, Some(42));
        assert_eq!(reopened.deleted_history(1).await.unwrap().len(), 1);
        assert!(reopened.deleted_history(2).await.unwrap().is_empty());
        reopened
            .set_policy(FeaturePolicy {
                archive_observed_messages: false,
                suppress_read_receipts: false,
                suppress_typing: false,
                suppress_online_status: false,
            })
            .await
            .unwrap();
        reopened.record_local_deletion(1, vec![3]).await.unwrap();
        assert!(reopened.deleted_history(1).await.unwrap().is_empty());
        reopened.clear_history().await.unwrap();
        assert!(reopened.history(1).await.unwrap().is_empty());
    }
    #[tokio::test]
    async fn failed_policy_save_is_not_kept_in_memory() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("private");
        let features = AyuFeatures::new(dir.to_str().unwrap().into());
        assert!(!features.policy().await.unwrap().suppress_typing);
        std::fs::remove_dir_all(&dir).unwrap();
        let result = features
            .set_policy(FeaturePolicy {
                suppress_read_receipts: false,
                suppress_typing: true,
                suppress_online_status: false,
                archive_observed_messages: false,
            })
            .await;
        assert!(result.is_err());
        assert!(!features.policy().await.unwrap().suppress_typing);
    }
}

#[derive(uniffi::Record)]
pub struct DesktopTheme {
    pub name: String,
    pub palette: String,
    pub is_dark: bool,
    pub wallpaper: Option<Vec<u8>>,
    pub wallpaper_extension: Option<String>,
    pub wallpaper_tiled: bool,
    pub video: Option<Vec<u8>>,
    pub video_warning: Option<String>,
}
#[uniffi::export]
pub async fn decode_desktop_theme(path: String) -> Result<DesktopTheme, EngineError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result =
            slopogram_ayu::theme::decode(std::path::Path::new(&path)).map(|theme| DesktopTheme {
                name: theme.name,
                palette: theme.palette,
                is_dark: theme.is_dark,
                wallpaper: theme.wallpaper,
                wallpaper_extension: theme.wallpaper_extension,
                wallpaper_tiled: theme.wallpaper_tiled,
                video: theme.video,
                video_warning: theme.video_warning,
            });
        let _ = tx.send(result);
    });
    rx.await.map_err(error)?.map_err(error)
}
