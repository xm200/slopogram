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

type JournalAction = Box<dyn FnOnce(&PathBuf) + Send>;

#[derive(uniffi::Object)]
pub struct AyuFeatures {
    journal: Arc<std::sync::Mutex<Option<slopogram_ayu::Journal>>>,
    sender: Option<tokio::sync::mpsc::Sender<JournalAction>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Drop for AyuFeatures {
    fn drop(&mut self) {
        drop(self.sender.take());
        if let Some(worker) = self.worker.take()
            && worker.thread().id() != std::thread::current().id()
        {
            let _ = worker.join();
        }
    }
}
impl AyuFeatures {
    async fn run<T, F>(&self, action: F) -> Result<T, EngineError>
    where
        T: Send + 'static,
        F: FnOnce(&mut slopogram_ayu::Journal) -> Result<T, String> + Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let journal = self.journal.clone();
        let job: JournalAction = Box::new(move |directory| {
            let result = (|| {
                let mut guard = journal
                    .lock()
                    .map_err(|_| "Local history unavailable".to_string())?;
                let journal = &mut *guard;
                if journal.is_none() {
                    *journal = Some(slopogram_ayu::Journal::open(directory.clone())?);
                }
                let result = action(journal.as_mut().ok_or("Local history unavailable")?);
                if result.is_err() {
                    *journal = None;
                }
                result
            })();
            let _ = tx.send(result);
        });
        self.sender
            .as_ref()
            .ok_or_else(|| error("Local history unavailable"))?
            .send(job)
            .await
            .map_err(|_| error("Local history worker unavailable"))?;
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
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<JournalAction>(256);
        let worker = std::thread::Builder::new()
            .name("ayu-journal".into())
            .spawn(move || {
                let directory = PathBuf::from(data_dir);
                while let Some(action) = receiver.blocking_recv() {
                    action(&directory);
                }
            })
            .ok();
        Arc::new(Self {
            journal: Arc::new(std::sync::Mutex::new(None)),
            sender: worker.as_ref().map(|_| sender),
            worker,
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
    async fn cancelled_admitted_action_finishes_before_drop_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("private").to_str().unwrap().to_owned();
        let features = AyuFeatures::new(path.clone());
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        let task_features = features.clone();
        let task = tokio::spawn(async move {
            task_features
                .run(move |_| {
                    let _ = entered_tx.send(());
                    release_rx.recv().unwrap();
                    let _ = finished_tx.send(());
                    Ok(())
                })
                .await
        });
        entered_rx.await.unwrap();
        task.abort();
        let _ = task.await;
        let dropping = tokio::task::spawn_blocking(move || drop(features));
        release_tx.send(()).unwrap();
        dropping.await.unwrap();
        finished_rx.await.unwrap();
        AyuFeatures::new(path).policy().await.unwrap();
    }

    #[tokio::test]
    async fn worker_reuses_named_thread_and_releases_journal_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("private").to_str().unwrap().to_owned();
        let features = AyuFeatures::new(path.clone());
        let identify = || {
            (
                std::thread::current().id(),
                std::thread::current().name().map(str::to_owned),
            )
        };
        let first = features.run(move |_| Ok(identify())).await.unwrap();
        let second = features.run(move |_| Ok(identify())).await.unwrap();
        assert_eq!(first, second);
        assert_eq!(first.1.as_deref(), Some("ayu-journal"));
        drop(features);
        let reopened = AyuFeatures::new(path);
        reopened.policy().await.unwrap();
    }

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
        let mut original = features.policy().await.unwrap();
        original.suppress_online_status = true;
        features.set_policy(original).await.unwrap();
        let connection = rusqlite::Connection::open(dir.join("observed-history.sqlite")).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_event BEFORE INSERT ON events
                 BEGIN SELECT RAISE(ABORT, 'test failure'); END;",
            )
            .unwrap();
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
        connection
            .execute_batch("DROP TRIGGER reject_event;")
            .unwrap();
        drop(features);
        let reopened = AyuFeatures::new(dir.to_str().unwrap().into());
        let persisted = reopened.policy().await.unwrap();
        assert!(!persisted.suppress_typing);
        assert!(persisted.suppress_online_status);
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

#[derive(uniffi::Record)]
pub struct DesktopThemeColor {
    pub key: String,
    pub value: String,
}

#[uniffi::export]
pub async fn desktop_theme_colors(palette: String) -> Result<Vec<DesktopThemeColor>, EngineError> {
    slopogram_ayu::theme::desktop_colors(&palette)
        .map(|colors| {
            colors
                .into_iter()
                .map(|(key, value)| DesktopThemeColor { key, value })
                .collect()
        })
        .map_err(error)
}

#[uniffi::export]
pub async fn encode_desktop_theme(
    colors: Vec<DesktopThemeColor>,
    wallpaper: Option<Vec<u8>>,
    wallpaper_extension: Option<String>,
    wallpaper_tiled: bool,
    video: Option<Vec<u8>>,
) -> Result<Vec<u8>, EngineError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let colors = colors
            .into_iter()
            .map(|color| (color.key, color.value))
            .collect();
        let _ = tx.send(slopogram_ayu::theme::encode(
            colors,
            wallpaper,
            wallpaper_extension,
            wallpaper_tiled,
            video,
        ));
    });
    rx.await.map_err(error)?.map_err(error)
}
