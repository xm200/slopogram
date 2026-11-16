use slopogram_core::{Chat, ChatId, Message};
use slopogram_telegram::{AuthStep, Command, Event, Operation};
use std::{path::PathBuf, sync::Arc};
uniffi::setup_scaffolding!();

#[derive(uniffi::Record)]
pub struct EngineConfig {
    pub api_id: i32,
    pub api_hash: String,
    pub data_dir: Option<String>,
    pub proxy: Option<String>,
}
#[derive(uniffi::Record)]
pub struct ChatRecord {
    pub id: i64,
    pub title: String,
    pub preview: String,
}
#[derive(uniffi::Record)]
pub struct MessageRecord {
    pub chat_id: i64,
    pub id: i32,
    pub text: String,
    pub sender: String,
    pub outgoing: bool,
    pub sent_at: i64,
    pub edited_at: Option<i64>,
}
impl From<Chat> for ChatRecord {
    fn from(c: Chat) -> Self {
        Self {
            id: c.id.0,
            title: c.title,
            preview: c.preview,
        }
    }
}
impl From<Message> for MessageRecord {
    fn from(m: Message) -> Self {
        Self {
            chat_id: m.chat.0,
            id: m.id,
            text: m.text,
            sender: m.sender,
            outgoing: m.outgoing,
            sent_at: m.sent_at,
            edited_at: m.edited_at,
        }
    }
}
#[derive(uniffi::Enum)]
pub enum ClientEvent {
    AuthPhone,
    AuthCode,
    AuthPassword {
        hint: String,
    },
    AuthQr {
        url: String,
        expires_at: i64,
    },
    Authorized {
        name: String,
    },
    Chats {
        chats: Vec<ChatRecord>,
    },
    ChatUpdated {
        chat: ChatRecord,
    },
    History {
        chat_id: i64,
        messages: Vec<MessageRecord>,
    },
    Message {
        message: MessageRecord,
    },
    Deleted {
        chat_id: Option<i64>,
        ids: Vec<i32>,
    },
    Sent {
        request_id: u64,
        message: MessageRecord,
    },
    Connection {
        connected: bool,
    },
    Failure {
        operation: String,
        message: String,
    },
    Fatal {
        message: String,
    },
    Stopped,
    SendConfirmed {
        request_id: u64,
    },
}
impl From<Event> for ClientEvent {
    fn from(e: Event) -> Self {
        match e {
            Event::Auth(AuthStep::Phone) => Self::AuthPhone,
            Event::Auth(AuthStep::Code) => Self::AuthCode,
            Event::Auth(AuthStep::Password { hint }) => Self::AuthPassword { hint },
            Event::Auth(AuthStep::Qr { url, expires_at }) => Self::AuthQr { url, expires_at },
            Event::Authorized { name } => Self::Authorized { name },
            Event::Chats(chats) => Self::Chats {
                chats: chats.into_iter().map(Into::into).collect(),
            },
            Event::ChatUpdated(chat) => Self::ChatUpdated { chat: chat.into() },
            Event::History { chat, messages } => Self::History {
                chat_id: chat.0,
                messages: messages.into_iter().map(Into::into).collect(),
            },
            Event::Message(message) => Self::Message {
                message: message.into(),
            },
            Event::Deleted { chat, ids } => Self::Deleted {
                chat_id: chat.map(|c| c.0),
                ids,
            },
            Event::Sent { request, message } => Self::Sent {
                request_id: request,
                message: message.into(),
            },
            Event::Connection { connected } => Self::Connection { connected },
            Event::Failure { operation, message } => Self::Failure {
                operation: match operation {
                    Operation::Login => "login".into(),
                    Operation::Chats => "chats".into(),
                    Operation::History(id) => format!("history:{}", id.0),
                    Operation::Send(id) => format!("send:{id}"),
                    Operation::Updates => "updates".into(),
                },
                message,
            },
            Event::Fatal(message) => Self::Fatal { message },
            Event::Stopped => Self::Stopped,
        }
    }
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
#[derive(uniffi::Object)]
pub struct Engine {
    client: slopogram_client::Client,
}
#[uniffi::export]
impl Engine {
    #[uniffi::constructor]
    pub fn new(config: EngineConfig) -> Result<Arc<Self>, EngineError> {
        let dir = match config.data_dir {
            Some(d) => PathBuf::from(d),
            None => directories::ProjectDirs::from("one", "slopogram", "Slopogram")
                .ok_or_else(|| error("Cannot locate application data directory"))?
                .data_local_dir()
                .to_owned(),
        };
        let config =
            slopogram_telegram::Config::new(&config.api_id.to_string(), &config.api_hash, dir)
                .map_err(error)?
                .with_proxy(config.proxy)
                .map_err(error)?;
        Ok(Arc::new(Self {
            client: slopogram_client::Client::start(config).map_err(error)?,
        }))
    }
    pub async fn request_qr_login(&self) -> Result<(), EngineError> {
        self.send(Command::StartQrLogin).await
    }
    pub async fn reset_login(&self) -> Result<(), EngineError> {
        self.send(Command::ResetLogin).await
    }
    pub async fn request_code(&self, phone: String) -> Result<(), EngineError> {
        self.send(Command::RequestCode(phone)).await
    }
    pub async fn submit_code(&self, code: String) -> Result<(), EngineError> {
        self.send(Command::SignIn(code)).await
    }
    pub async fn submit_password(&self, password: String) -> Result<(), EngineError> {
        self.send(Command::Password(password)).await
    }
    pub async fn load_chats(&self) -> Result<(), EngineError> {
        self.send(Command::LoadChats).await
    }
    pub async fn open_chat(&self, chat_id: i64) -> Result<(), EngineError> {
        self.send(Command::OpenChat(ChatId(chat_id))).await
    }
    pub async fn mark_read(&self, chat_id: i64) -> Result<(), EngineError> {
        self.send(Command::MarkRead(ChatId(chat_id))).await
    }
    pub async fn send_message(
        &self,
        request_id: u64,
        chat_id: i64,
        text: String,
    ) -> Result<(), EngineError> {
        self.send(Command::Send {
            request: request_id,
            chat: ChatId(chat_id),
            text,
        })
        .await
    }
    pub async fn next_event(&self) -> Option<ClientEvent> {
        self.client.next_event().await.map(|event| match event {
            slopogram_client::ClientEvent::Backend(event) => event.into(),
            slopogram_client::ClientEvent::SendConfirmed { request } => {
                ClientEvent::SendConfirmed {
                    request_id: request,
                }
            }
        })
    }
    pub async fn shutdown(&self) -> Result<(), EngineError> {
        self.client.shutdown().await.map_err(error)
    }
}
impl Engine {
    async fn send(&self, command: Command) -> Result<(), EngineError> {
        self.client.send(command).await.map_err(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_history_and_send_failure_identity() {
        let message = Message {
            chat: ChatId(-42),
            id: 7,
            text: "text".into(),
            sender: "sender".into(),
            outgoing: true,
            sent_at: 1,
            edited_at: Some(2),
        };
        match ClientEvent::from(Event::History {
            chat: ChatId(-42),
            messages: vec![message],
        }) {
            ClientEvent::History { chat_id, messages } => {
                assert_eq!(chat_id, -42);
                assert_eq!(messages[0].id, 7);
                assert_eq!(messages[0].edited_at, Some(2));
            }
            _ => panic!("wrong event"),
        }
        match ClientEvent::from(Event::Failure {
            operation: Operation::Send(123),
            message: "failed".into(),
        }) {
            ClientEvent::Failure { operation, .. } => assert_eq!(operation, "send:123"),
            _ => panic!("wrong event"),
        }
    }

    #[test]
    fn invalid_configuration_returns_error_before_starting_worker() {
        let config = EngineConfig {
            api_id: 0,
            api_hash: String::new(),
            data_dir: Some("/tmp".into()),
            proxy: None,
        };
        assert!(Engine::new(config).is_err());
    }
}
