use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

use iced::{
    Alignment, Color, Element, Fill, Length, Subscription, Task, Theme,
    futures::{SinkExt, Stream},
    widget::{Space, button, column, container, row, scrollable, text, text_input},
};
use slopogram_core::{Chat, ChatId, Conversations, normalize_phone, validate_text};
use slopogram_telegram::{AuthStep, Command, Config, Event, Operation};
use tokio::sync::mpsc;

fn text_primary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().text),
    }
}
fn text_muted(theme: &Theme) -> text::Style {
    let fg = theme.palette().text;
    let bg = theme.palette().background;
    let mix = |a: f32, b: f32| a * 0.68 + b * 0.32;
    text::Style {
        color: Some(Color::from_rgb(
            mix(fg.r, bg.r),
            mix(fg.g, bg.g),
            mix(fg.b, bg.b),
        )),
    }
}
fn text_danger(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.palette().danger),
    }
}

#[derive(Clone)]
pub enum UiMessage {
    Started(u64, mpsc::Sender<Command>),
    Backend(u64, Event),
    Phone(String),
    Code(String),
    Password(String),
    Authenticate,
    StartQr,
    Back,
    Search(String),
    Open(ChatId),
    Draft(String),
    Send,
    Restore(u64),
    RefreshChats,
    RefreshHistory,
    Reconnect,
    DismissError,
    Close(iced::window::Id),
    ForceClose(iced::window::Id),
    Focused(bool),
}

// Iced requires Debug messages; omit all user input and backend event payloads.
impl fmt::Debug for UiMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("UiMessage [payload redacted]")
    }
}

struct PendingSend {
    chat: ChatId,
    text: String,
    failure: Option<String>,
}

#[derive(Default)]
pub struct App {
    generation: u64,
    commands: Option<mpsc::Sender<Command>>,
    auth: Option<AuthStep>,
    qr_data: Option<iced::widget::qr_code::Data>,
    authorized: bool,
    connected: bool,
    fatal: bool,
    busy: bool,
    phone: String,
    code: String,
    password: String,
    name: String,
    chats: Vec<Chat>,
    search: String,
    selected: Option<ChatId>,
    conversations: Conversations,
    loading: HashSet<ChatId>,
    drafts: HashMap<ChatId, String>,
    pending: BTreeMap<u64, PendingSend>,
    next_request: u64,
    chats_loading: bool,
    error: Option<String>,
    closing: Option<iced::window::Id>,
    focused: bool,
}

impl App {
    pub fn subscription(&self) -> Subscription<UiMessage> {
        Subscription::batch([
            Subscription::run_with(self.generation, backend_stream),
            iced::window::close_requests().map(UiMessage::Close),
            iced::window::events().filter_map(|(_, event)| match event {
                iced::window::Event::Focused => Some(UiMessage::Focused(true)),
                iced::window::Event::Unfocused => Some(UiMessage::Focused(false)),
                _ => None,
            }),
        ])
    }

    fn dispatch(&mut self, command: Command) -> bool {
        if let Some(sender) = &self.commands
            && sender.try_send(command).is_ok()
        {
            return true;
        }
        self.error = Some(
            "Cannot queue this action. Wait a moment, or reconnect if the connection has stopped."
                .into(),
        );
        false
    }

    pub fn update(&mut self, message: UiMessage) -> Task<UiMessage> {
        match message {
            UiMessage::Focused(focused) => {
                self.focused = focused;
                #[cfg(target_os = "macos")]
                if focused {
                    slopogram_macos::install_backdrop();
                }
                if focused
                    && self.authorized
                    && !self.fatal
                    && let Some(chat) = self.selected
                {
                    self.dispatch(Command::MarkRead(chat));
                }
            }
            UiMessage::Started(generation, commands) if generation == self.generation => {
                self.commands = Some(commands);
            }
            UiMessage::Backend(generation, event) if generation == self.generation => {
                if let Event::Stopped = event {
                    self.commands = None;
                    self.connected = false;
                    if let Some(window) = self.closing {
                        return iced::window::close(window);
                    }
                    if !self.fatal {
                        self.fatal = true;
                        self.error =
                            Some("The Telegram connection stopped. Reconnect to continue.".into());
                    }
                } else {
                    self.handle_event(event);
                }
            }
            UiMessage::Phone(value) => self.phone = value,
            UiMessage::Code(value) => self.code = value,
            UiMessage::Password(value) => self.password = value,
            UiMessage::StartQr if !self.busy => {
                self.qr_data = None;
                self.busy = self.dispatch(Command::StartQrLogin);
            }
            UiMessage::Authenticate if !self.busy => {
                self.error = None;
                let command = match self.auth.as_ref() {
                    Some(AuthStep::Phone) => match normalize_phone(&self.phone) {
                        Ok(phone) => Some(Command::RequestCode(phone)),
                        Err(error) => {
                            self.error = Some(error.into());
                            None
                        }
                    },
                    Some(AuthStep::Code) if !self.code.trim().is_empty() => {
                        Some(Command::SignIn(std::mem::take(&mut self.code)))
                    }
                    Some(AuthStep::Password { .. }) if !self.password.is_empty() => {
                        Some(Command::Password(std::mem::take(&mut self.password)))
                    }
                    _ => {
                        self.error = Some("Complete the field to continue.".into());
                        None
                    }
                };
                if let Some(command) = command {
                    self.busy = self.dispatch(command);
                }
            }
            UiMessage::Back if !self.busy => {
                self.qr_data = None;
                self.code.clear();
                self.password.clear();
                self.error = None;
                self.busy = self.dispatch(Command::ResetLogin);
            }
            UiMessage::Search(value) => self.search = value,
            UiMessage::Open(chat) => {
                self.selected = Some(chat);
                self.error = None;
                self.conversations.prepare(chat);
                if !self.loading.contains(&chat) && self.dispatch(Command::OpenChat(chat)) {
                    self.loading.insert(chat);
                }
            }
            UiMessage::Draft(value) => {
                if let Some(chat) = self.selected {
                    self.drafts.insert(chat, value);
                }
            }
            UiMessage::Send if !self.fatal => {
                if let Some(chat) = self.selected {
                    let value = self.drafts.get(&chat).cloned().unwrap_or_default();
                    if let Err(error) = validate_text(&value) {
                        self.error = Some(error.into());
                    } else {
                        let request = self.next_request;
                        self.next_request += 1;
                        if self.dispatch(Command::Send {
                            request,
                            chat,
                            text: value.clone(),
                        }) {
                            self.drafts.remove(&chat);
                            self.pending.insert(
                                request,
                                PendingSend {
                                    chat,
                                    text: value,
                                    failure: None,
                                },
                            );
                            self.error = None;
                        }
                    }
                }
            }
            UiMessage::Restore(request) => {
                if let Some(pending) = self.pending.remove(&request) {
                    let draft = self.drafts.entry(pending.chat).or_default();
                    if !draft.is_empty() {
                        draft.push('\n');
                    }
                    draft.push_str(&pending.text);
                }
            }
            UiMessage::RefreshChats if !self.chats_loading => {
                self.chats_loading = self.dispatch(Command::LoadChats);
            }
            UiMessage::RefreshHistory => {
                if let Some(chat) = self.selected
                    && !self.loading.contains(&chat)
                    && self.dispatch(Command::OpenChat(chat))
                {
                    self.loading.insert(chat);
                }
            }
            UiMessage::Reconnect => {
                // Preserve drafts and unconfirmed messages in memory. An explicit
                // reconnect must not silently discard text the user wrote.
                self.generation += 1;
                self.commands = None;
                self.fatal = false;
                self.connected = false;
                self.authorized = false;
                self.auth = None;
                self.qr_data = None;
                self.busy = false;
                self.loading.clear();
                self.chats_loading = false;
                self.error = None;
                self.code.clear();
                self.password.clear();
                for pending in self.pending.values_mut() {
                    if pending.failure.is_none() {
                        pending.failure =
                            Some("Send not confirmed. Check the chat before sending again.".into());
                    }
                }
            }
            UiMessage::DismissError => self.error = None,
            UiMessage::Close(window) => {
                if self.closing.is_none() {
                    self.closing = Some(window);
                    if !self.dispatch(Command::Shutdown) {
                        return iced::window::close(window);
                    }
                    return Task::perform(
                        async move {
                            tokio::time::sleep(std::time::Duration::from_secs(8)).await;
                            window
                        },
                        UiMessage::ForceClose,
                    );
                }
            }
            UiMessage::ForceClose(window) => return iced::window::close(window),
            _ => {}
        }
        Task::none()
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Auth(step) => {
                self.qr_data = match &step {
                    AuthStep::Qr { url, .. } => iced::widget::qr_code::Data::new(url).ok(),
                    _ => None,
                };
                self.auth = Some(step);
                self.busy = false;
                self.code.clear();
                self.password.clear();
            }
            Event::Authorized { name } => {
                self.authorized = true;
                self.qr_data = None;
                self.name = name;
                self.auth = None;
                self.busy = false;
                self.chats_loading = true;
                self.phone.clear();
                self.code.clear();
                self.password.clear();
            }
            Event::Chats(chats) => {
                self.chats = chats;
                self.chats_loading = false;
            }
            Event::ChatUpdated(chat) => {
                if let Some(existing) = self
                    .chats
                    .iter_mut()
                    .find(|existing| existing.id == chat.id)
                {
                    *existing = chat;
                } else {
                    self.chats.insert(0, chat);
                }
            }
            Event::History { chat, messages } => {
                self.conversations.merge_history(chat, messages);
                self.loading.remove(&chat);
                if self.selected == Some(chat) && self.focused && !self.fatal {
                    self.dispatch(Command::MarkRead(chat));
                }
            }
            Event::Message(message) => {
                if self.selected == Some(message.chat)
                    && self.focused
                    && !message.outgoing
                    && !self.fatal
                {
                    self.dispatch(Command::MarkRead(message.chat));
                }
                if let Some(chat) = self.chats.iter_mut().find(|chat| chat.id == message.chat) {
                    chat.preview = message.text.chars().take(80).collect();
                } else {
                    self.chats.insert(
                        0,
                        Chat {
                            id: message.chat,
                            title: message.sender.clone(),
                            preview: message.text.chars().take(80).collect(),
                        },
                    );
                }
                self.conversations.upsert(message);
            }
            Event::Sent { request, message } => {
                self.pending.remove(&request);
                if let Some(chat) = self.chats.iter_mut().find(|chat| chat.id == message.chat) {
                    chat.preview = message.text.chars().take(80).collect();
                }
                self.conversations.upsert(message);
            }
            Event::Deleted { chat, ids } => self.conversations.delete(chat, &ids),
            Event::Connection { connected } => self.connected = connected,
            Event::Failure { operation, message } => {
                match operation {
                    Operation::Login => self.busy = false,
                    Operation::Chats => self.chats_loading = false,
                    Operation::History(chat) => {
                        self.loading.remove(&chat);
                    }
                    Operation::Send(request) => {
                        if let Some(pending) = self.pending.get_mut(&request) {
                            pending.failure = Some(message.clone());
                        }
                    }
                    Operation::Updates => {}
                }
                self.error = Some(message);
            }
            Event::Fatal(message) => {
                self.fatal = true;
                self.busy = false;
                self.connected = false;
                self.error = Some(message);
            }
            Event::Stopped => {}
        }
    }

    pub fn view(&self) -> Element<'_, UiMessage> {
        let body = if self.authorized {
            self.messenger()
        } else {
            self.login()
        };
        let mut layout = column![body].height(Fill);
        if let Some(error) = &self.error {
            layout = layout.push(
                container(
                    row![
                        text(error).size(14).style(text_danger).width(Fill),
                        button("Dismiss")
                            .style(button::text)
                            .on_press(UiMessage::DismissError),
                    ]
                    .spacing(16)
                    .align_y(Alignment::Center),
                )
                .padding(16)
                .width(Fill)
                .style(|t| container::Style {
                    background: Some(
                        Color {
                            a: 0.15,
                            ..t.palette().danger
                        }
                        .into(),
                    ),
                    ..Default::default()
                }),
            );
        }
        if self.fatal {
            layout = layout.push(
                container(button("Reconnect").on_press(UiMessage::Reconnect))
                    .padding(12)
                    .center_x(Fill),
            );
        }
        layout.into()
    }

    fn login(&self) -> Element<'_, UiMessage> {
        let mut form = column![
            text("Slopogram").size(34).style(text_primary),
            text("Your conversations, a little closer.")
                .size(16)
                .style(text_muted),
            Space::new().height(24)
        ]
        .spacing(12)
        .max_width(420);
        if self.closing.is_some() {
            form = form.push(text("Saving session and closing…"));
        } else if self.fatal {
            form = form.push(text("Connection needs attention").size(22))
                .push(text("For first-time setup, copy .env.example to .env and add your application's API ID and hash from my.telegram.org/apps. Restart after changing credentials.").style(text_muted));
        } else {
            match self.auth.as_ref() {
                None => {
                    form = form.push(text("Connecting to Telegram…").style(text_muted));
                }
                Some(AuthStep::Phone) => {
                    form = form.push(text("Sign in to Telegram").size(23))
                        .push(text("Enter the phone number for an existing account, including its country code.").style(text_muted))
                        .push(self.login_input("+1 555 123 4567", &self.phone, UiMessage::Phone, false))
                        .push(self.auth_button("Send login code"))
                        .push(button("Sign in with QR code").on_press(UiMessage::StartQr));
                }
                Some(AuthStep::Code) => {
                    form = form.push(text("Check your Telegram messages").size(23))
                        .push(text("Enter the login code Telegram sent you. Delivery may be through another signed-in app.").style(text_muted))
                        .push(self.login_input("Login code", &self.code, UiMessage::Code, true))
                        .push(self.auth_button("Continue"));
                }
                Some(AuthStep::Password { hint }) => {
                    form = form.push(text("Two-step verification").size(23)).push(
                        text("Enter your Telegram two-step verification password.")
                            .style(text_muted),
                    );
                    if !hint.is_empty() {
                        form = form.push(text(format!("Hint: {hint}")).size(14).style(text_muted));
                    }
                    form = form
                        .push(self.login_input(
                            "Password",
                            &self.password,
                            UiMessage::Password,
                            true,
                        ))
                        .push(self.auth_button("Sign in"));
                }
                Some(AuthStep::Qr { .. }) => {
                    form = form.push(text("Scan to sign in").size(23));
                    if let Some(data) = self.qr_data.as_ref() {
                        form = form.push(
                            container(
                                iced::widget::qr_code::QRCode::new(data)
                                    .total_size(210)
                                    .style(|_| iced::widget::qr_code::Style {
                                        cell: Color::BLACK,
                                        background: Color::WHITE,
                                    }),
                            )
                            .padding(16)
                            .style(|_| container::Style {
                                background: Some(Color::WHITE.into()),
                                border: iced::Border {
                                    radius: 12.0.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            }),
                        );
                    }
                    form = form
                        .push(
                            text("In Telegram, open Settings > Devices > Link Desktop Device.")
                                .style(text_muted),
                        )
                        .push(
                            text("This code refreshes automatically.")
                                .size(13)
                                .style(text_muted),
                        )
                        .push(button("Use phone number").on_press(UiMessage::Back));
                }
            }
            if matches!(self.auth, Some(AuthStep::Code | AuthStep::Password { .. })) && !self.busy {
                form = form.push(
                    button("Use a different phone number")
                        .style(button::text)
                        .on_press(UiMessage::Back),
                );
            }
        }
        form = form.push(Space::new().height(20)).push(
            text("An independent desktop client using the Telegram API.")
                .size(13)
                .style(text_muted),
        );
        let panel = container(form).padding(24).style(|theme| container::Style {
            background: Some(
                Color {
                    a: 0.72,
                    ..theme.palette().background
                }
                .into(),
            ),
            border: iced::Border {
                color: Color {
                    a: 0.16,
                    ..theme.palette().text
                },
                width: 1.0,
                radius: 24.0.into(),
            },
            shadow: iced::Shadow {
                color: Color {
                    a: 0.12,
                    ..Color::BLACK
                },
                offset: iced::Vector::new(0.0, 5.0),
                blur_radius: 18.0,
            },
            ..Default::default()
        });
        scrollable(container(panel).padding(20).center_x(Fill).center_y(Fill))
            .height(Fill)
            .width(Fill)
            .into()
    }

    fn login_input<'a>(
        &self,
        placeholder: &'a str,
        value: &'a str,
        on_input: fn(String) -> UiMessage,
        secure: bool,
    ) -> Element<'a, UiMessage> {
        let input = text_input(placeholder, value)
            .padding(14)
            .size(17)
            .secure(secure);
        if self.busy {
            input.into()
        } else {
            input
                .on_input(on_input)
                .on_submit(UiMessage::Authenticate)
                .into()
        }
    }

    fn auth_button<'a>(&self, label: &'a str) -> Element<'a, UiMessage> {
        let control = button(text(if self.busy { "Please wait…" } else { label }))
            .padding([12, 20])
            .width(Fill);
        if self.busy {
            control.into()
        } else {
            control.on_press(UiMessage::Authenticate).into()
        }
    }

    fn messenger(&self) -> Element<'_, UiMessage> {
        let status = if self.closing.is_some() {
            "Saving session…"
        } else if self.fatal {
            "Connection stopped"
        } else if self.connected {
            "Connected"
        } else {
            "Connecting…"
        };
        let mut chats = column![].spacing(4);
        let query = self.search.to_lowercase();
        for chat in self
            .chats
            .iter()
            .filter(|chat| chat.title.to_lowercase().contains(&query))
        {
            let selected = self.selected == Some(chat.id);
            let item = column![
                text(&chat.title).size(16),
                text(chat.preview.replace('\n', " "))
                    .size(13)
                    .style(text_muted)
            ]
            .spacing(5);
            chats = chats.push(
                button(item)
                    .padding(14)
                    .width(Fill)
                    .on_press(UiMessage::Open(chat.id))
                    .style(move |theme: &Theme, status| {
                        let mut style = button::subtle(theme, status);
                        if selected {
                            style.background = Some(
                                Color {
                                    a: 0.18,
                                    ..theme.palette().primary
                                }
                                .into(),
                            );
                        }
                        style.text_color = theme.palette().text;
                        style
                    }),
            );
        }
        if self.chats.is_empty() {
            chats = chats.push(
                text(if self.chats_loading {
                    "Loading conversations…"
                } else {
                    "No conversations yet. Start one in another Telegram app, then refresh."
                })
                .style(text_muted),
            );
        }
        let refresh = button(if self.chats_loading {
            "Loading…"
        } else {
            "Refresh"
        })
        .style(button::text);
        let refresh = if self.chats_loading || self.fatal {
            refresh
        } else {
            refresh.on_press(UiMessage::RefreshChats)
        };
        let sidebar = container(
            column![
                row![
                    text("Slopogram").size(25),
                    Space::new().width(Fill),
                    refresh
                ]
                .align_y(Alignment::Center),
                text_input("Find a conversation", &self.search)
                    .on_input(UiMessage::Search)
                    .padding(11),
                scrollable(chats).height(Fill),
                column![
                    text(&self.name).size(14),
                    text(status).size(12).style(text_muted)
                ]
                .spacing(4),
            ]
            .spacing(18),
        )
        .padding(20)
        .width(300)
        .height(Fill)
        .style(|theme| container::Style {
            background: Some(
                Color {
                    a: 0.70,
                    ..theme.palette().background
                }
                .into(),
            ),
            border: iced::Border {
                color: Color {
                    a: 0.18,
                    ..theme.palette().text
                },
                width: 1.0,
                radius: 18.0.into(),
            },
            shadow: iced::Shadow {
                color: Color {
                    a: 0.12,
                    ..Color::BLACK
                },
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        });
        row![sidebar, self.conversation()].height(Fill).into()
    }

    fn conversation(&self) -> Element<'_, UiMessage> {
        let Some(chat) = self.selected else {
            return container(
                column![
                    text("A place for your conversations").size(27),
                    text("Choose a chat to read and send messages.").style(text_muted)
                ]
                .spacing(12),
            )
            .center_x(Fill)
            .center_y(Fill)
            .into();
        };
        let title = self
            .chats
            .iter()
            .find(|item| item.id == chat)
            .map(|item| item.title.as_str())
            .unwrap_or("Conversation");
        let mut messages = column![].spacing(12).padding(24).width(Fill);
        let mut count = 0;
        for message in self.conversations.messages(chat) {
            count += 1;
            let label = if message.outgoing {
                "You"
            } else {
                &message.sender
            };
            let mut content = column![
                text(label).size(12).style(text_muted),
                text(&message.text).size(16)
            ]
            .spacing(6);
            if message.edited_at.is_some() {
                content = content.push(text("Edited").size(11).style(text_muted));
            }
            let outgoing = message.outgoing;
            let bubble = container(content)
                .padding([12, 16])
                .max_width(620)
                .style(move |theme| container::Style {
                    background: Some(
                        if outgoing {
                            Color {
                                a: 0.20,
                                ..theme.palette().primary
                            }
                        } else {
                            theme.extended_palette().background.weak.color
                        }
                        .into(),
                    ),
                    border: iced::Border {
                        color: Color {
                            a: 0.12,
                            ..theme.palette().text
                        },
                        width: 1.0,
                        radius: 16.0.into(),
                    },
                    shadow: iced::Shadow {
                        color: Color {
                            a: 0.08,
                            ..Color::BLACK
                        },
                        offset: iced::Vector::new(0.0, 2.0),
                        blur_radius: 8.0,
                    },
                    ..Default::default()
                });
            let line = container(bubble).width(Fill);
            messages = messages.push(if outgoing {
                line.align_right(Fill)
            } else {
                line.align_left(Fill)
            });
        }
        for (request, pending) in self
            .pending
            .iter()
            .filter(|(_, pending)| pending.chat == chat)
        {
            let mut content = column![text(&pending.text).size(16)].spacing(8);
            if let Some(error) = &pending.failure {
                content = content.push(text(error).size(12).style(text_danger)).push(
                    button("Restore text to composer")
                        .style(button::text)
                        .on_press(UiMessage::Restore(*request)),
                );
            } else {
                content = content.push(text("Sending…").size(12).style(text_muted));
            }
            messages = messages.push(
                container(
                    container(content)
                        .padding(14)
                        .max_width(620)
                        .style(|theme| container::Style {
                            background: Some(
                                Color {
                                    a: 0.20,
                                    ..theme.palette().primary
                                }
                                .into(),
                            ),
                            border: iced::Border {
                                color: Color {
                                    a: 0.12,
                                    ..theme.palette().text
                                },
                                width: 1.0,
                                radius: 16.0.into(),
                            },
                            shadow: iced::Shadow {
                                color: Color {
                                    a: 0.08,
                                    ..Color::BLACK
                                },
                                offset: iced::Vector::new(0.0, 2.0),
                                blur_radius: 8.0,
                            },
                            ..Default::default()
                        }),
                )
                .align_right(Fill),
            );
        }
        if count == 0 && self.pending.values().all(|pending| pending.chat != chat) {
            messages = messages.push(
                text(if self.loading.contains(&chat) {
                    "Loading recent messages…"
                } else {
                    "No messages yet. Write the first one below."
                })
                .style(text_muted),
            );
        }
        let draft = self
            .drafts
            .get(&chat)
            .map(String::as_str)
            .unwrap_or_default();
        let input = text_input("Write a message", draft).padding(15).size(16);
        let send = button("Send").padding([15, 22]);
        let (input, send) = if self.fatal || self.closing.is_some() {
            (input, send)
        } else {
            (
                input.on_input(UiMessage::Draft).on_submit(UiMessage::Send),
                if validate_text(draft).is_ok() {
                    send.on_press(UiMessage::Send)
                } else {
                    send
                },
            )
        };
        let refresh = button(if self.loading.contains(&chat) {
            "Loading…"
        } else {
            "Refresh"
        })
        .style(button::text);
        let refresh = if self.loading.contains(&chat) || self.fatal {
            refresh
        } else {
            refresh.on_press(UiMessage::RefreshHistory)
        };
        column![
            container(
                row![
                    column![
                        text(title).size(23),
                        text("Latest 100 messages").size(12).style(text_muted)
                    ]
                    .spacing(4),
                    Space::new().width(Fill),
                    refresh
                ]
                .align_y(Alignment::Center)
            )
            .padding([20, 24])
            .width(Fill),
            scrollable(messages).anchor_bottom().height(Fill),
            container(row![input, send].spacing(12))
                .padding(20)
                .width(Fill),
        ]
        .height(Fill)
        .width(Length::Fill)
        .into()
    }
}

fn backend_stream(generation: &u64) -> impl Stream<Item = UiMessage> + use<> {
    let generation = *generation;
    iced::stream::channel(
        256,
        async move |mut output: iced::futures::channel::mpsc::Sender<UiMessage>| {
            if let Err(error) = crate::config_file::load() {
                let _ = output
                    .send(UiMessage::Backend(generation, Event::Fatal(error)))
                    .await;
                return;
            }
            let config = match Config::from_env() {
                Ok(config) => config,
                Err(error) => {
                    let _ = output
                        .send(UiMessage::Backend(
                            generation,
                            Event::Fatal(error.to_string()),
                        ))
                        .await;
                    return;
                }
            };
            // Explicit SLOPOGRAM_PROXY wins; otherwise default to the macOS
            // system SOCKS proxy when one is configured.
            let config = if config.proxy().is_none() {
                #[cfg(target_os = "macos")]
                {
                    config.with_proxy(slopogram_macos::system_proxy())
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Ok(config)
                }
            } else {
                Ok(config)
            };
            let config = match config {
                Ok(config) => config,
                Err(error) => {
                    let _ = output
                        .send(UiMessage::Backend(
                            generation,
                            Event::Fatal(error.to_string()),
                        ))
                        .await;
                    return;
                }
            };
            let (commands, mut events) = slopogram_telegram::start(config);
            if output
                .send(UiMessage::Started(generation, commands))
                .await
                .is_err()
            {
                return;
            }
            while let Some(event) = events.recv().await {
                if output
                    .send(UiMessage::Backend(generation, event))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authentication_clears_sensitive_inputs() {
        let mut app = App {
            phone: "private phone".into(),
            code: "private code".into(),
            password: "private password".into(),
            ..App::default()
        };
        app.handle_event(Event::Authorized {
            name: "Test".into(),
        });
        assert!(app.phone.is_empty() && app.code.is_empty() && app.password.is_empty());
    }

    #[test]
    fn qr_auth_data_is_created_and_cleared_after_authorization() {
        let mut app = App::default();
        app.handle_event(Event::Auth(AuthStep::Qr {
            url: "tg://login?token=private".into(),
            expires_at: 1,
        }));
        assert!(app.qr_data.is_some());
        app.handle_event(Event::Authorized {
            name: "Test".into(),
        });
        assert!(app.qr_data.is_none());
    }

    #[test]
    fn failed_login_keeps_the_current_step_available_for_retry() {
        let mut app = App {
            auth: Some(AuthStep::Code),
            busy: true,
            ..App::default()
        };
        app.handle_event(Event::Failure {
            operation: Operation::Login,
            message: "Invalid code".into(),
        });
        assert_eq!(app.auth, Some(AuthStep::Code));
        assert!(!app.busy);
        assert!(app.error.is_some());
    }

    #[test]
    fn late_history_does_not_switch_or_mark_read_another_conversation() {
        let (commands, mut receiver) = mpsc::channel(4);
        let mut app = App {
            commands: Some(commands),
            selected: Some(ChatId(2)),
            focused: true,
            ..App::default()
        };
        app.loading.insert(ChatId(1));
        app.handle_event(Event::History {
            chat: ChatId(1),
            messages: Vec::new(),
        });
        assert_eq!(app.selected, Some(ChatId(2)));
        assert!(!app.loading.contains(&ChatId(1)));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn read_receipts_require_the_visible_conversation_and_window_focus() {
        let (commands, mut receiver) = mpsc::channel(4);
        let mut app = App {
            commands: Some(commands),
            selected: Some(ChatId(1)),
            ..App::default()
        };
        app.handle_event(Event::History {
            chat: ChatId(1),
            messages: Vec::new(),
        });
        assert!(receiver.try_recv().is_err());
        app.focused = true;
        app.handle_event(Event::History {
            chat: ChatId(1),
            messages: Vec::new(),
        });
        assert!(matches!(
            receiver.try_recv(),
            Ok(Command::MarkRead(ChatId(1)))
        ));
    }

    #[test]
    fn failed_send_keeps_text_and_restores_without_overwriting_new_draft() {
        let mut app = App::default();
        let chat = ChatId(1);
        app.drafts.insert(chat, "New draft".into());
        app.pending.insert(
            0,
            PendingSend {
                chat,
                text: "Unconfirmed".into(),
                failure: None,
            },
        );
        app.handle_event(Event::Failure {
            operation: Operation::Send(0),
            message: "Network unavailable".into(),
        });
        assert!(app.pending[&0].failure.is_some());
        let _ = app.update(UiMessage::Restore(0));
        assert_eq!(app.drafts[&chat], "New draft\nUnconfirmed");
    }

    #[test]
    fn events_from_previous_connections_are_ignored() {
        let mut app = App {
            generation: 1,
            ..App::default()
        };
        let _ = app.update(UiMessage::Backend(0, Event::Fatal("Old connection".into())));
        assert!(!app.fatal);
    }

    #[test]
    fn reconnect_preserves_drafts_and_flags_unconfirmed_sends() {
        let mut app = App::default();
        app.drafts.insert(ChatId(1), "Draft".into());
        app.pending.insert(
            1,
            PendingSend {
                chat: ChatId(1),
                text: "Pending".into(),
                failure: None,
            },
        );
        let _ = app.update(UiMessage::Reconnect);
        assert_eq!(app.drafts[&ChatId(1)], "Draft");
        assert!(app.pending[&1].failure.is_some());
    }
}
