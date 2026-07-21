//! SMTP経由でのOTPメール送信・アクセス許可申請の通知。
//! [`open-easy-web`]の`server/src/mail.rs`と同じ設計(`lettre`の同期SMTP
//! クライアントを`spawn_blocking`でオフロード)。`RS-Chiketto`の
//! `src/mail.rs`をそのまま移植(環境変数のみ`RSEC_*`に変更)。
//!
//! [`open-easy-web`]: https://github.com/aon-co-jp/open-easy-web

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from: String,
}

impl SmtpConfig {
    pub fn from_env() -> Option<Self> {
        Some(Self {
            host: std::env::var("RSEC_SMTP_HOST").ok()?,
            port: std::env::var("RSEC_SMTP_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or(587),
            username: std::env::var("RSEC_SMTP_USERNAME").ok()?,
            password: std::env::var("RSEC_SMTP_PASSWORD").ok()?,
            from: std::env::var("RSEC_SMTP_FROM").ok()?,
        })
    }
}

#[derive(Debug)]
pub enum MailError {
    Build(String),
    Send(String),
}

impl std::fmt::Display for MailError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MailError::Build(e) => write!(f, "メール作成失敗: {e}"),
            MailError::Send(e) => write!(f, "メール送信失敗: {e}"),
        }
    }
}

fn build_and_send(config: &SmtpConfig, to: &str, subject: &str, body: String) -> Result<(), MailError> {
    let email = Message::builder()
        .from(config.from.parse().map_err(|e| MailError::Build(format!("{e}")))?)
        .to(to.parse().map_err(|e| MailError::Build(format!("{e}")))?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body)
        .map_err(|e| MailError::Build(format!("{e}")))?;

    let creds = Credentials::new(config.username.clone(), config.password.clone());
    let mailer =
        SmtpTransport::starttls_relay(&config.host).map_err(|e| MailError::Send(format!("{e}")))?.port(config.port).credentials(creds).build();

    mailer.send(&email).map_err(|e| MailError::Send(format!("{e}")))?;
    Ok(())
}

pub async fn send_otp(config: SmtpConfig, to: String, code: String) -> Result<(), MailError> {
    let body = format!(
        "RS-EC ログイン用ワンタイムパスワード\n\n\
         コード: {code}\n\
         このコードは5分間有効です。\n\n\
         心当たりがない場合はこのメールを無視してください。"
    );
    tokio::task::spawn_blocking(move || build_and_send(&config, &to, "RS-EC ログインコード", body))
        .await
        .map_err(|e| MailError::Send(format!("task panicked: {e}")))?
}

/// 誰かが`POST /api/accounts/request`でアクセス許可を申請したことを
/// 管理者へ通知する。
pub async fn send_access_request_notice(
    config: SmtpConfig,
    admin_email: String,
    request_email: String,
    message: Option<String>,
) -> Result<(), MailError> {
    let message_line = message.as_deref().unwrap_or("(メッセージなし)");
    let body = format!(
        "RS-ECへのアクセス許可申請が届きました。\n\n\
         申請者メール: {request_email}\n\
         メッセージ: {message_line}\n\n\
         管理者としてログインし、GET /api/accounts/requests で申請一覧を確認、\n\
         POST /api/accounts/requests/:id/decide で閲覧/編集を個別に選んで\n\
         許可・不許可を決定してください。"
    );
    tokio::task::spawn_blocking(move || build_and_send(&config, &admin_email, "RS-EC アクセス許可申請", body))
        .await
        .map_err(|e| MailError::Send(format!("task panicked: {e}")))?
}

/// アクセス許可申請の審査結果(承認/却下)を申請者へ通知する。
pub async fn send_access_decision(config: SmtpConfig, to: String, approved: bool) -> Result<(), MailError> {
    let body = if approved {
        "RS-ECへのアクセス申請が承認されました。付与された権限の範囲でログイン・操作が可能です。".to_string()
    } else {
        "RS-ECへのアクセス申請は承認されませんでした。".to_string()
    };
    let subject = if approved { "RS-EC アクセス申請: 承認されました" } else { "RS-EC アクセス申請: 却下されました" };
    tokio::task::spawn_blocking(move || build_and_send(&config, &to, subject, body))
        .await
        .map_err(|e| MailError::Send(format!("task panicked: {e}")))?
}
