//! Runtime-owning client facade for native frontends.

use slopogram_core::Conversations;
use slopogram_telegram::{Command, Config, Event};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc;

/// Events from the backend and confirmations whose message was deleted.
pub enum ClientEvent {
    Backend(Event),
    SendConfirmed { request: u64 },
}

/// Owns a backend worker on a dedicated runtime thread. Commands can be sent
/// from any thread, while events are delivered in backend order.
pub struct Client {
    commands: mpsc::Sender<Command>,
    stopped: AtomicBool,
    shutdown_lock: tokio::sync::Mutex<()>,
    events: Arc<tokio::sync::Mutex<mpsc::Receiver<Event>>>,
    conversations: Mutex<Conversations>,
    done: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    finished: tokio::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}

impl Client {
    pub fn start(config: Config) -> Result<Self, String> {
        Self::start_with(move || slopogram_telegram::start(config))
    }

    fn start_with<F>(start: F) -> Result<Self, String>
    where
        F: FnOnce() -> (mpsc::Sender<Command>, mpsc::Receiver<Event>) + Send + 'static,
    {
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("slopogram-client".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = ready_tx.send(Err(error.to_string()));
                        return;
                    }
                };
                runtime.block_on(async move {
                    let (commands, events) = start();
                    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
                    if ready_tx.send(Ok((commands, events, done_tx))).is_ok() {
                        let _ = done_rx.await;
                    }
                });
                drop(runtime);
                let _ = finished_tx.send(());
            })
            .map_err(|error| error.to_string())?;
        let (commands, events, done) = ready_rx.recv().map_err(|error| error.to_string())??;
        // Keep the runtime alive until the event stream ends or the client is dropped.
        let events = Arc::new(tokio::sync::Mutex::new(events));
        drop(thread);
        Ok(Self {
            commands,
            stopped: AtomicBool::new(false),
            shutdown_lock: tokio::sync::Mutex::new(()),
            events,
            conversations: Mutex::new(Conversations::default()),
            done: Arc::new(Mutex::new(Some(done))),
            finished: tokio::sync::Mutex::new(Some(finished_rx)),
        })
    }
    pub async fn send(&self, command: Command) -> Result<(), String> {
        if self.stopped.load(Ordering::Acquire) {
            return Err("Client stopped".into());
        }
        if let Command::OpenChat(chat) = &command {
            self.conversations.lock().unwrap().prepare(*chat);
        }
        self.commands
            .send(command)
            .await
            .map_err(|_| "Client stopped".into())
    }
    pub async fn shutdown(&self) -> Result<(), String> {
        let _guard = self.shutdown_lock.lock().await;
        if self.stopped.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        // A worker which has already stopped still needs its runtime released.
        let _ = self.commands.try_send(Command::Shutdown);
        let done = self.done.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            if let Some(done) = done.lock().unwrap().take() {
                let _ = done.send(());
            }
        });
        if let Some(finished) = self.finished.lock().await.take() {
            let _ = finished.await;
        }
        Ok(())
    }
    pub async fn next_event(&self) -> Option<ClientEvent> {
        loop {
            let mut events = self.events.lock().await;
            let event = events.recv().await?;
            let mut state = self.conversations.lock().unwrap();
            let event = match event {
                Event::History { chat, messages } => {
                    state.merge_history(chat, messages);
                    Event::History {
                        chat,
                        messages: state.messages(chat).cloned().collect(),
                    }
                }
                Event::Message(message) => {
                    let chat = message.chat;
                    let id = message.id;
                    state.upsert(message);
                    match state.messages(chat).find(|m| m.id == id).cloned() {
                        Some(message) => Event::Message(message),
                        None => continue,
                    }
                }
                Event::Sent { request, message } => {
                    let chat = message.chat;
                    let id = message.id;
                    state.upsert(message);
                    match state.messages(chat).find(|m| m.id == id).cloned() {
                        Some(message) => Event::Sent { request, message },
                        None => return Some(ClientEvent::SendConfirmed { request }),
                    }
                }
                Event::Deleted { chat, ids } => {
                    state.delete(chat, &ids);
                    Event::Deleted { chat, ids }
                }
                Event::Stopped => {
                    if let Some(done) = self.done.lock().unwrap().take() {
                        let _ = done.send(());
                    }
                    Event::Stopped
                }
                other => other,
            };
            return Some(ClientEvent::Backend(event));
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if let Ok(mut done) = self.done.lock()
            && let Some(done) = done.take()
        {
            let _ = done.send(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slopogram_core::{ChatId, Message};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    struct Guard(Arc<AtomicBool>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    #[tokio::test]
    async fn shutdown_wakes_poll_and_releases_runtime_resources() {
        let released = Arc::new(AtomicBool::new(false));
        let held = released.clone();
        let client = Arc::new(
            Client::start_with(move || {
                let (commands, mut incoming) = mpsc::channel(8);
                let (events, receiver) = mpsc::channel(8);
                tokio::spawn(async move {
                    let _guard = Guard(held);
                    let _events = events;
                    while incoming.recv().await.is_some() {
                        std::future::pending::<()>().await;
                    }
                });
                (commands, receiver)
            })
            .unwrap(),
        );
        let poller = {
            let client = client.clone();
            tokio::spawn(async move { client.next_event().await })
        };
        tokio::task::yield_now().await;
        tokio::time::timeout(Duration::from_secs(5), client.shutdown())
            .await
            .unwrap()
            .unwrap();
        assert!(released.load(Ordering::Acquire));
        assert!(
            tokio::time::timeout(Duration::from_secs(1), poller)
                .await
                .unwrap()
                .unwrap()
                .is_none()
        );
        assert!(client.send(Command::LoadChats).await.is_err());
        client.shutdown().await.unwrap();
    }

    fn message(id: i32, text: &str, edited_at: Option<i64>) -> Message {
        Message {
            chat: ChatId(1),
            id,
            text: text.into(),
            sender: "A".into(),
            outgoing: false,
            sent_at: 1,
            edited_at,
        }
    }

    #[tokio::test]
    async fn reconciles_history_deletes_edits_and_sent_echoes() {
        let (events, receiver) = mpsc::channel(16);
        let client = Client::start_with(move || {
            let (commands, mut incoming) = mpsc::channel(8);
            tokio::spawn(async move { while incoming.recv().await.is_some() {} });
            (commands, receiver)
        })
        .unwrap();
        client.send(Command::OpenChat(ChatId(1))).await.unwrap();
        events
            .send(Event::Message(message(1, "new", Some(3))))
            .await
            .unwrap();
        events
            .send(Event::Deleted {
                chat: Some(ChatId(1)),
                ids: vec![2],
            })
            .await
            .unwrap();
        events
            .send(Event::History {
                chat: ChatId(1),
                messages: vec![
                    message(1, "old", None),
                    message(2, "deleted", None),
                    message(3, "history", None),
                ],
            })
            .await
            .unwrap();
        events
            .send(Event::Sent {
                request: 10,
                message: message(2, "deleted", None),
            })
            .await
            .unwrap();
        events
            .send(Event::Sent {
                request: 9,
                message: message(1, "stale", None),
            })
            .await
            .unwrap();
        assert!(matches!(
            client.next_event().await,
            Some(ClientEvent::Backend(Event::Message(_)))
        ));
        assert!(matches!(
            client.next_event().await,
            Some(ClientEvent::Backend(Event::Deleted { .. }))
        ));
        match client.next_event().await.unwrap() {
            ClientEvent::Backend(Event::History { messages, .. }) => {
                assert_eq!(messages.len(), 2);
                assert_eq!(messages[0].text, "new");
                assert_eq!(messages[1].id, 3);
            }
            _ => panic!("expected history"),
        }
        assert!(matches!(
            client.next_event().await,
            Some(ClientEvent::SendConfirmed { request: 10 })
        ));
        match client.next_event().await.unwrap() {
            ClientEvent::Backend(Event::Sent { message, .. }) => assert_eq!(message.text, "new"),
            _ => panic!("expected sent"),
        }
    }
}
