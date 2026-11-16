//! Async Telegram adapter. Protocol types do not escape this crate.

mod config;
mod qr;
mod session;

use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};

use anyhow::{Context, Result, anyhow};
use grammers_client::{
    Client, SenderPool, SignInError,
    client::{LoginToken, UpdateStream},
    sender::{ConnectionParams, UpdatesConfiguration},
    update::Update,
};
use grammers_session::types::{PeerId, PeerRef};
use slopogram_core::{Chat, ChatId, Message, normalize_phone, validate_text};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::timeout,
};

pub use config::Config;
use session::StoredSession;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
pub const HISTORY_LIMIT: usize = 100;

// No Debug: commands contain phone numbers, codes, passwords, and message text.
pub enum Command {
    RequestCode(String),
    SignIn(String),
    Password(String),
    StartQrLogin,
    ResetLogin,
    LoadChats,
    OpenChat(ChatId),
    MarkRead(ChatId),
    Send {
        request: u64,
        chat: ChatId,
        text: String,
    },
    Shutdown,
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuthStep {
    Phone,
    Code,
    Password { hint: String },
    Qr { url: String, expires_at: i64 },
}

impl std::fmt::Debug for AuthStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Phone => f.write_str("Phone"),
            Self::Code => f.write_str("Code"),
            Self::Password { hint } => f.debug_struct("Password").field("hint", hint).finish(),
            Self::Qr { expires_at, .. } => f
                .debug_struct("Qr")
                .field("url", &"<redacted>")
                .field("expires_at", expires_at)
                .finish(),
        }
    }
}

#[cfg(test)]
mod auth_step_tests {
    use super::AuthStep;

    #[test]
    fn qr_debug_redacts_token_url() {
        let step = AuthStep::Qr {
            url: "tg://login?token=secret-token".into(),
            expires_at: 123,
        };
        let debug = format!("{step:?}");
        assert!(!debug.contains("secret-token"));
        assert!(debug.contains("<redacted>"));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Login,
    Chats,
    History(ChatId),
    Send(u64),
    Updates,
}

#[derive(Debug, Clone)]
pub enum Event {
    Auth(AuthStep),
    Authorized {
        name: String,
    },
    Chats(Vec<Chat>),
    ChatUpdated(Chat),
    History {
        chat: ChatId,
        messages: Vec<Message>,
    },
    Message(Message),
    Deleted {
        chat: Option<ChatId>,
        ids: Vec<i32>,
    },
    Sent {
        request: u64,
        message: Message,
    },
    Connection {
        connected: bool,
    },
    Failure {
        operation: Operation,
        message: String,
    },
    Fatal(String),
    Stopped,
}

/// Start on a Tokio runtime. Dropping the command sender or event receiver stops
/// the worker. No credentials or message contents are written to diagnostic logs.
pub fn start(config: Config) -> (mpsc::Sender<Command>, mpsc::Receiver<Event>) {
    let (commands, receiver) = mpsc::channel(32);
    let (events, event_receiver) = mpsc::channel(256);
    tokio::spawn(async move {
        let result = tokio::select! {
            result = run(config, receiver, events.clone()) => result,
            _ = events.closed() => return,
        };
        if let Err(error) = result {
            let _ = events.send(Event::Fatal(format!("{error:#}"))).await;
        }
        let _ = events.send(Event::Stopped).await;
    });
    (commands, event_receiver)
}

struct Connection {
    client: Client,
    runner: JoinHandle<()>,
    _stored: Arc<StoredSession>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.client.disconnect();
        self.runner.abort();
    }
}

struct UpdateWorker(JoinHandle<()>);

impl Drop for UpdateWorker {
    fn drop(&mut self) {
        self.0.abort();
    }
}

async fn request<T>(
    future: impl Future<Output = Result<T, grammers_client::InvocationError>>,
) -> Result<T> {
    timeout(REQUEST_TIMEOUT, future)
        .await
        .context("Telegram did not respond. Check your connection and try again.")?
        .map_err(Into::into)
}

async fn emit(events: &mpsc::Sender<Event>, event: Event) -> Result<()> {
    events
        .send(event)
        .await
        .map_err(|_| anyhow!("The application was closed."))
}

async fn failure(
    events: &mpsc::Sender<Event>,
    operation: Operation,
    error: impl std::fmt::Display,
) {
    let _ = events
        .send(Event::Failure {
            operation,
            message: error.to_string(),
        })
        .await;
}

async fn run(
    config: Config,
    mut commands: mpsc::Receiver<Command>,
    events: mpsc::Sender<Event>,
) -> Result<()> {
    let stored = Arc::new(StoredSession::open(&config.data_dir).await?);
    let params = ConnectionParams {
        device_model: "Slopogram Desktop".into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        updates_channel_capacity: 4096.try_into().expect("nonzero capacity"),
        proxy_url: config.proxy.clone(),
        ..Default::default()
    };
    let mut pool =
        SenderPool::with_configuration(Arc::clone(&stored.session), config.api_id, params);
    let client = Client::new(pool.handle);
    let connection = Connection {
        client: client.clone(),
        runner: tokio::spawn(pool.runner.run()),
        _stored: stored,
    };

    if !request(client.is_authorized()).await?
        && !authenticate(
            &client,
            config.api_id,
            &config.api_hash,
            &connection._stored.session,
            &mut commands,
            &events,
            &mut pool.updates,
        )
        .await?
    {
        return Ok(());
    }
    let me = request(client.get_me()).await?;
    emit(
        &events,
        Event::Authorized {
            name: me.first_name().unwrap_or("You").into(),
        },
    )
    .await?;

    // Populate the peer cache before starting catch-up, as required by grammers.
    let mut peers = HashMap::new();
    match load_chats(&client, &mut peers).await {
        Ok(chats) => emit(&events, Event::Chats(chats)).await?,
        Err(error) => failure(&events, Operation::Chats, error).await,
    }
    let updates = timeout(
        REQUEST_TIMEOUT,
        client.stream_updates(pool.updates, UpdatesConfiguration { catch_up: true }),
    )
    .await
    .context("Timed out starting Telegram updates. Retry the connection.")?
    .map_err(|error| anyhow!("Cannot start Telegram updates: {error}"))?;
    let (stop, stop_receiver) = oneshot::channel();
    let stored_guard = Arc::clone(&connection._stored);
    let update_events = events.clone();
    let mut updater = UpdateWorker(tokio::spawn(async move {
        // Keep the session lock until the update task has actually stopped.
        let _stored = stored_guard;
        receive_updates(updates, update_events, stop_receiver).await;
    }));
    emit(&events, Event::Connection { connected: true }).await?;

    loop {
        let command = tokio::select! {
            command = commands.recv() => command,
            _ = events.closed() => None,
            _ = &mut updater.0 => return Err(anyhow!("Telegram updates stopped. Reconnect to continue.")),
        };
        match command {
            None | Some(Command::Shutdown) => break,
            Some(Command::LoadChats) => match load_chats(&client, &mut peers).await {
                Ok(chats) => emit(&events, Event::Chats(chats)).await?,
                Err(error) => failure(&events, Operation::Chats, error).await,
            },
            Some(Command::OpenChat(chat)) => match history(&client, &peers, chat).await {
                Ok(messages) => emit(&events, Event::History { chat, messages }).await?,
                Err(error) => failure(&events, Operation::History(chat), error).await,
            },
            Some(Command::MarkRead(chat)) => {
                if let Err(error) = mark_read(&client, &peers, chat).await {
                    failure(
                        &events,
                        Operation::Updates,
                        format!("Could not mark messages as read: {error}"),
                    )
                    .await;
                }
            }
            Some(Command::Send {
                request: id,
                chat,
                text,
            }) => {
                let result = async {
                    validate_text(&text).map_err(|error| anyhow!(error))?;
                    let peer = peer_ref(&client, &peers, chat).await?;
                    let message = request(client.send_message(peer, text)).await?;
                    Ok::<_, anyhow::Error>(convert_message(&message))
                }
                .await;
                match result {
                    Ok(message) => emit(&events, Event::Sent { request: id, message }).await?,
                    Err(error) => failure(&events, Operation::Send(id), format!("Send not confirmed: {error}. Check the conversation before sending again.")).await,
                }
            }
            Some(_) => failure(&events, Operation::Login, "Already signed in.").await,
        }
    }
    let _ = stop.send(());
    let _ = timeout(Duration::from_secs(5), &mut updater.0).await;
    drop(connection);
    Ok(())
}

async fn authenticate(
    client: &Client,
    api_id: i32,
    api_hash: &str,
    session: &grammers_session::storages::SqliteSession,
    commands: &mut mpsc::Receiver<Command>,
    events: &mpsc::Sender<Event>,
    raw_updates: &mut mpsc::Receiver<grammers_session::updates::UpdatesLike>,
) -> Result<bool> {
    let mut token: Option<LoginToken> = None;
    let mut password_required = false;
    let mut qr_active = true;
    let mut last_url: Option<String> = None;
    let mut tick = tokio::time::interval(Duration::from_secs(30));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    emit(events, Event::Auth(AuthStep::Phone)).await?;
    loop {
        let command = if qr_active {
            tokio::select! {
                biased;
                command = commands.recv() => command,
                update = raw_updates.recv(), if qr_active => {
                    match update {
                        Some(update) if qr::is_login_update(&update) => tick.reset_immediately(),
                        Some(_) => {},
                        None => return Err(anyhow!("Telegram connection closed during QR login.")),
                    }
                    continue;
                }
                outcome = async {
                    tick.tick().await;
                    timeout(REQUEST_TIMEOUT, qr::advance(client, session, api_id, api_hash)).await
                } => {
                    match outcome {
                        Ok(Ok(qr::QrResult::Pending { url, expires_at })) => {
                            if last_url.as_deref() != Some(&url) {
                                last_url = Some(url.clone());
                                emit(events, Event::Auth(AuthStep::Qr { url, expires_at })).await?;
                            }
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;
                            tick.reset_after(Duration::from_secs((expires_at - now).max(1) as u64));
                        }
                        Ok(Ok(qr::QrResult::Authorized)) => {
                            qr::persist_authorized(client, session).await?;
                            return Ok(true);
                        }
                        Ok(Ok(qr::QrResult::Migrate { .. })) => unreachable!("advance handles migration"),
                        Ok(Err(error)) if error.downcast_ref::<grammers_client::InvocationError>().is_some_and(|e| e.is("SESSION_PASSWORD_NEEDED")) => {
                            qr_active = false;
                            password_required = true;
                            let grammers_client::tl::enums::account::Password::Password(challenge) = request(client.invoke(&grammers_client::tl::functions::account::GetPassword {})).await?;
                            emit(events, Event::Auth(AuthStep::Password { hint: challenge.hint.unwrap_or_default() })).await?;
                        }
                        Ok(Err(error)) => {
                            qr_active = false;
                            failure(events, Operation::Login, error).await;
                            emit(events, Event::Auth(AuthStep::Phone)).await?;
                        }
                        Err(error) => {
                            qr_active = false;
                            failure(events, Operation::Login, error).await;
                            emit(events, Event::Auth(AuthStep::Phone)).await?;
                        }
                    }
                    continue;
                }
            }
        } else {
            commands.recv().await
        };
        let Some(command) = command else {
            return Ok(false);
        };
        let result: Result<bool> = match command {
            Command::Shutdown => return Ok(false),
            Command::StartQrLogin => {
                qr_active = true;
                last_url = None;
                token = None;
                password_required = false;
                tick.reset_immediately();
                Ok(false)
            }
            Command::ResetLogin => {
                qr_active = false;
                last_url = None;
                token = None;
                password_required = false;
                emit(events, Event::Auth(AuthStep::Phone)).await?;
                Ok(false)
            }
            Command::RequestCode(phone) => {
                qr_active = false;
                last_url = None;
                match normalize_phone(&phone).map_err(|error| anyhow!(error)) {
                    Err(error) => Err(error),
                    Ok(phone) => match request(client.request_login_code(&phone, api_hash)).await {
                        Ok(login) => {
                            token = Some(login);
                            password_required = false;
                            emit(events, Event::Auth(AuthStep::Code)).await?;
                            Ok(false)
                        }
                        Err(error) => Err(error),
                    },
                }
            }
            Command::SignIn(code) => match token.as_ref() {
                None => Err(anyhow!("Request a login code first.")),
                Some(token) => {
                    match timeout(REQUEST_TIMEOUT, client.sign_in(token, code.trim())).await {
                        Ok(Ok(_)) => Ok(true),
                        Ok(Err(SignInError::PasswordRequired(password))) => {
                            password_required = true;
                            emit(
                                events,
                                Event::Auth(AuthStep::Password {
                                    hint: password.hint().unwrap_or_default().into(),
                                }),
                            )
                            .await?;
                            Ok(false)
                        }
                        Ok(Err(error)) => Err(error.into()),
                        Err(_) => Err(anyhow!(
                            "Login timed out. Check your connection and try again."
                        )),
                    }
                }
            },
            Command::Password(password) if password_required => {
                // Fetch a fresh challenge on every attempt: check_password consumes
                // its token, including on network failure or an invalid password.
                async {
                    let grammers_client::tl::enums::account::Password::Password(challenge) =
                        request(
                            client.invoke(&grammers_client::tl::functions::account::GetPassword {}),
                        )
                        .await?;
                    timeout(
                        REQUEST_TIMEOUT,
                        client.check_password(
                            grammers_client::client::PasswordToken::new(challenge),
                            password,
                        ),
                    )
                    .await
                    .context("Password check timed out. Try again.")??;
                    Ok::<_, anyhow::Error>(true)
                }
                .await
            }
            _ => Err(anyhow!("Finish signing in first.")),
        };
        match result {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(error) => failure(events, Operation::Login, error).await,
        }
    }
}

async fn load_chats(client: &Client, peers: &mut HashMap<ChatId, PeerRef>) -> Result<Vec<Chat>> {
    timeout(REQUEST_TIMEOUT, async {
        let mut dialogs = client.iter_dialogs();
        let mut chats = Vec::new();
        while let Some(dialog) = dialogs.next().await? {
            let id = chat_id(dialog.peer_id());
            peers.insert(id, dialog.peer_ref());
            chats.push(Chat {
                id,
                title: dialog.peer().name().unwrap_or("Deleted account").into(),
                preview: dialog
                    .last_message
                    .as_ref()
                    .map(|message| preview(message.text()))
                    .unwrap_or_default(),
            });
        }
        Ok::<_, anyhow::Error>(chats)
    })
    .await
    .context("Loading conversations timed out. Retry to load them again.")?
}

async fn peer_ref(
    client: &Client,
    peers: &HashMap<ChatId, PeerRef>,
    chat: ChatId,
) -> Result<PeerRef> {
    if let Some(peer) = peers.get(&chat) {
        return Ok(*peer);
    }
    let id = PeerId::from_bot_api_dialog_id(chat.0).context("Invalid conversation identifier.")?;
    let peer = request(client.resolve_peer(id.to_ambient_ref())).await?;
    peer.to_ref()
        .await
        .map_err(|error| anyhow!("Cannot resolve conversation: {error}"))?
        .context("Refresh conversations before opening this chat.")
}

async fn history(
    client: &Client,
    peers: &HashMap<ChatId, PeerRef>,
    chat: ChatId,
) -> Result<Vec<Message>> {
    let peer = peer_ref(client, peers, chat).await?;
    let messages = timeout(REQUEST_TIMEOUT, async {
        let mut history = client.iter_messages(peer).limit(HISTORY_LIMIT);
        let mut messages = Vec::new();
        while let Some(message) = history.next().await? {
            messages.push(convert_message(&message));
        }
        Ok::<_, anyhow::Error>(messages)
    })
    .await
    .context("Loading messages timed out. Open the chat again to retry.")??;
    Ok(messages)
}

async fn mark_read(client: &Client, peers: &HashMap<ChatId, PeerRef>, chat: ChatId) -> Result<()> {
    let peer = peer_ref(client, peers, chat).await?;
    request(client.mark_as_read(peer)).await
}

async fn receive_updates(
    mut updates: UpdateStream,
    events: mpsc::Sender<Event>,
    mut stop: oneshot::Receiver<()>,
) {
    loop {
        let update = tokio::select! {
            _ = &mut stop => break,
            _ = events.closed() => break,
            update = updates.next() => update,
        };
        let event = match update {
            Ok(Update::NewMessage(message) | Update::MessageEdited(message)) => {
                if let Some(title) = message.peer().and_then(|peer| peer.name()) {
                    let _ = emit(
                        &events,
                        Event::ChatUpdated(Chat {
                            id: chat_id(message.peer_id()),
                            title: title.into(),
                            preview: preview(message.text()),
                        }),
                    )
                    .await;
                }
                Some(Event::Message(convert_message(&message)))
            }
            Ok(Update::MessageDeleted(deletion)) => Some(Event::Deleted {
                chat: deletion
                    .channel_id()
                    .map(|id| chat_id(PeerId::channel_unchecked(id))),
                ids: deletion.into_messages(),
            }),
            Ok(_) => None,
            Err(error) => {
                let _ = emit(&events, Event::Connection { connected: false }).await;
                failure(&events, Operation::Updates, error).await;
                tokio::select! {
                    _ = &mut stop => break,
                    _ = events.closed() => break,
                    _ = tokio::time::sleep(Duration::from_secs(3)) => {}
                }
                continue;
            }
        };
        if let Some(event) = event
            && emit(&events, event).await.is_err()
        {
            break;
        }
        if let Err(error) = updates.sync_update_state().await {
            failure(
                &events,
                Operation::Updates,
                format!("Cannot save synchronization state: {error}"),
            )
            .await;
        }
        let _ = emit(&events, Event::Connection { connected: true }).await;
    }
    if let Err(error) = updates.sync_update_state().await {
        failure(
            &events,
            Operation::Updates,
            format!("Cannot save synchronization state: {error}"),
        )
        .await;
    }
}

fn chat_id(id: PeerId) -> ChatId {
    ChatId(id.bot_api_dialog_id_unchecked())
}

fn preview(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .take(80)
        .collect()
}

fn convert_message(message: &grammers_client::message::Message) -> Message {
    Message {
        chat: chat_id(message.peer_id()),
        id: message.id(),
        text: if message.text().is_empty() {
            "[Media or service message]".into()
        } else {
            message.text().into()
        },
        sender: message
            .sender()
            .and_then(|peer| peer.name())
            .unwrap_or(if message.outgoing() {
                "You"
            } else {
                "Unknown sender"
            })
            .into(),
        outgoing: message.outgoing(),
        sent_at: message.date().as_second(),
        edited_at: message.edit_date().map(|date| date.as_second()),
    }
}

#[cfg(test)]
mod qr_auth_tests {
    use super::*;

    #[tokio::test]
    async fn queued_reset_and_shutdown_win_before_qr_network_poll() {
        let root = tempfile::tempdir().unwrap();
        let stored = StoredSession::open(&root.path().join("private"))
            .await
            .unwrap();
        let mut pool = SenderPool::with_configuration(
            Arc::clone(&stored.session),
            1,
            ConnectionParams::default(),
        );
        let client = Client::new(pool.handle);
        let (command_tx, mut command_rx) = mpsc::channel(4);
        let (event_tx, mut event_rx) = mpsc::channel(4);
        command_tx.send(Command::ResetLogin).await.unwrap();
        command_tx.send(Command::Shutdown).await.unwrap();

        let result = timeout(
            Duration::from_millis(200),
            authenticate(
                &client,
                1,
                &"a".repeat(32),
                &stored.session,
                &mut command_rx,
                &event_tx,
                &mut pool.updates,
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!result);
        assert!(matches!(
            event_rx.recv().await,
            Some(Event::Auth(AuthStep::Phone))
        ));
        assert!(matches!(
            event_rx.recv().await,
            Some(Event::Auth(AuthStep::Phone))
        ));
        assert!(event_rx.try_recv().is_err());
    }
}
