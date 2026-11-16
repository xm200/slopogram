use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

pub(super) enum QrResult {
    Pending { url: String, expires_at: i64 },
    Authorized,
    Migrate { dc_id: i32, token: Vec<u8> },
}

impl std::fmt::Debug for QrResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending { expires_at, .. } => f
                .debug_struct("Pending")
                .field("url", &"<redacted>")
                .field("expires_at", expires_at)
                .finish(),
            Self::Authorized => f.write_str("Authorized"),
            Self::Migrate { dc_id, .. } => f
                .debug_struct("Migrate")
                .field("dc_id", dc_id)
                .field("token", &"<redacted>")
                .finish(),
        }
    }
}

pub(super) fn login_url(token: &[u8]) -> String {
    format!("tg://login?token={}", URL_SAFE_NO_PAD.encode(token))
}

pub(super) fn is_login_update(update: &grammers_session::updates::UpdatesLike) -> bool {
    use grammers_client::tl::enums::{Update, Updates};
    use grammers_session::updates::UpdatesLike;

    match update {
        UpdatesLike::Updates(Updates::UpdateShort(short)) => {
            matches!(short.update, Update::LoginToken)
        }
        UpdatesLike::Updates(Updates::Updates(updates)) => updates
            .updates
            .iter()
            .any(|update| matches!(update, Update::LoginToken)),
        UpdatesLike::Updates(Updates::Combined(updates)) => updates
            .updates
            .iter()
            .any(|update| matches!(update, Update::LoginToken)),
        UpdatesLike::ConnectionClosed => true,
        _ => false,
    }
}

pub(super) async fn export(
    client: &grammers_client::Client,
    api_id: i32,
    api_hash: &str,
) -> Result<QrResult, grammers_client::InvocationError> {
    use grammers_client::tl::{enums::auth::LoginToken, functions::auth::ExportLoginToken};

    match client
        .invoke(&ExportLoginToken {
            api_id,
            api_hash: api_hash.to_owned(),
            except_ids: Vec::new(),
        })
        .await?
    {
        LoginToken::Token(token) => Ok(QrResult::Pending {
            url: login_url(&token.token),
            expires_at: i64::from(token.expires),
        }),
        LoginToken::Success(_) => Ok(QrResult::Authorized),
        LoginToken::MigrateTo(token) => Ok(QrResult::Migrate {
            dc_id: token.dc_id,
            token: token.token,
        }),
    }
}

pub(super) async fn advance(
    client: &grammers_client::Client,
    session: &grammers_session::storages::SqliteSession,
    api_id: i32,
    api_hash: &str,
) -> anyhow::Result<QrResult> {
    use grammers_client::tl::{enums::auth::LoginToken, functions::auth::ImportLoginToken};
    use grammers_session::Session;

    match export(client, api_id, api_hash).await? {
        QrResult::Migrate { dc_id, token } => {
            let imported = client
                .invoke_in_dc(dc_id, &ImportLoginToken { token })
                .await;
            match imported {
                Ok(LoginToken::Token(token)) => {
                    session.set_home_dc_id(dc_id).await?;
                    Ok(QrResult::Pending {
                        url: login_url(&token.token),
                        expires_at: i64::from(token.expires),
                    })
                }
                Ok(LoginToken::Success(_)) => {
                    session.set_home_dc_id(dc_id).await?;
                    Ok(QrResult::Authorized)
                }
                Ok(LoginToken::MigrateTo(_)) => {
                    anyhow::bail!("Telegram requested an unexpected second QR login migration.")
                }
                Err(error) if error.is("SESSION_PASSWORD_NEEDED") => {
                    session.set_home_dc_id(dc_id).await?;
                    Err(error.into())
                }
                Err(error) => Err(error.into()),
            }
        }
        result => Ok(result),
    }
}

pub(super) async fn persist_authorized(
    client: &grammers_client::Client,
    session: &grammers_session::storages::SqliteSession,
) -> anyhow::Result<()> {
    use anyhow::Context;
    use grammers_client::tl::functions::updates::GetState;
    use grammers_session::{
        Session,
        types::{PeerInfo, UpdateState, UpdatesState},
    };

    let user = client.get_me().await?;
    let peer = user
        .to_ref()
        .await
        .map_err(|error| anyhow::anyhow!("Cannot resolve authorized user: {error}"))?
        .context("Authorized user has no peer reference")?;
    session
        .cache_peer(PeerInfo::User {
            id: user.id().bare_id_unchecked(),
            auth: Some(peer.auth),
            bot: Some(user.is_bot()),
            is_self: Some(true),
        })
        .await?;
    let grammers_client::tl::enums::updates::State::State(state) =
        client.invoke(&GetState {}).await?;
    session
        .set_update_state(UpdateState::All(UpdatesState {
            pts: state.pts,
            qts: state.qts,
            date: state.date,
            seq: state.seq,
            channels: Vec::new(),
        }))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{QrResult, is_login_update, login_url};

    #[test]
    fn encodes_token_as_unpadded_base64url() {
        assert_eq!(login_url(&[251, 255]), "tg://login?token=-_8");
        assert!(
            !login_url(&[251, 255])
                .strip_prefix("tg://login?token=")
                .unwrap()
                .contains('=')
        );
    }

    #[test]
    fn identifies_login_tokens_in_short_updates() {
        use grammers_client::tl::{
            enums::{Update, Updates},
            types::UpdateShort,
        };
        use grammers_session::updates::UpdatesLike;

        assert!(is_login_update(&UpdatesLike::Updates(
            Updates::UpdateShort(UpdateShort {
                update: Update::LoginToken,
                date: 0,
            })
        )));
        assert!(!is_login_update(&UpdatesLike::Updates(Updates::TooLong)));
    }

    #[test]
    fn debug_redacts_credentials() {
        let pending = QrResult::Pending {
            url: "tg://login?token=secret".into(),
            expires_at: 123,
        };
        let migrate = QrResult::Migrate {
            dc_id: 2,
            token: b"secret-migration-token".to_vec(),
        };
        assert!(!format!("{pending:?}").contains("secret"));
        assert!(!format!("{migrate:?}").contains("secret"));
    }
}
