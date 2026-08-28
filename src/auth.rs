//! メールアドレス宛のワンタイムパスワード(OTP)認証+QR確認ログイン
//! (2026-08-28追加)。
//! [`open-easy-web`](https://github.com/aon-co-jp/open-easy-web)の
//! `server/src/auth.rs`と同じ設計(固定パスワード非保存、OTPは
//! SHA-256ハッシュのみ保持)を踏襲。`RS-Chiketto`の`src/auth.rs`を
//! そのまま移植(ロジック変更なし)。
//!
//! ## ログイン方式(2026-08-28新設、ユーザー指示「email OTP・QR撮影
//! のみ・email OTP+QR撮影、いずれも選べるように」への対応)
//!
//! `RS-EC`はEC-CUBE相当の通販サイトで、複数の登録アカウント
//! (`accounts.rs`)+注文・決済データを扱う。**「パスワード無し」
//! (`none`)モードに加えて、QR撮影単体で事前のメール確認なしに
//! ログインする`"qr"`モードも、このリポジトリでは意図的に実装しない**
//! ——`RS-Blog`(単一固定管理者)やopen-easy-web(TOTP登録済みアカウント
//! のみqr可)とは異なり、`RS-EC`は複数の登録メールアドレスのうち
//! どれでログインしようとしているのかを、OTPを経由せずには特定できない
//! (TOTP等の事前登録済み第二要素も持たない)。「メールアドレスさえ
//! 知っていればQRセッションを開始できてしまう」設計は、注文・決済に
//! 関わるアカウントの完全性を損なうため採用しない。実装するのは以下の
//! 2方式のみ:
//! - `"otp"`(既定): 既存のメールOTP単体。
//! - `"otp_qr"`: OTP検証成功後、セッションを即発行せずQR確認を第二要素
//!   として要求する(真の2FA、既にメール所有を証明した後の追加確認
//!   のためQR単体ログインの懸念は生じない)。

use rand::Rng;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const OTP_TTL: Duration = Duration::from_secs(5 * 60);
const SESSION_TTL: Duration = Duration::from_secs(12 * 60 * 60);
const MAX_ATTEMPTS: u32 = 5;
const QR_LOGIN_TTL: Duration = Duration::from_secs(3 * 60);

pub const LOGIN_MODE_OTP: &str = "otp";
pub const LOGIN_MODE_OTP_QR: &str = "otp_qr";

pub fn is_valid_login_mode(s: &str) -> bool {
    matches!(s, LOGIN_MODE_OTP | LOGIN_MODE_OTP_QR)
}

/// メールアドレスの一部を伏せる(QR確認ページの表示用)。
fn mask_email(email: &str) -> String {
    match email.find('@') {
        Some(at) => {
            let (local, domain) = email.split_at(at);
            let visible: String = local.chars().take(2).collect();
            format!("{visible}***{domain}")
        }
        None => "***".to_string(),
    }
}

fn hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn generate_otp() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1_000_000u32))
}

fn generate_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 24] = rng.gen();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

struct PendingOtp {
    code_hash: String,
    expires_at: Instant,
    attempts: u32,
}

struct Session {
    email: String,
    expires_at: Instant,
}

struct QrLoginEntry {
    account_email: String,
    confirmed: bool,
    expires_at: Instant,
}

pub struct AuthStore {
    pending: Mutex<HashMap<String, PendingOtp>>,
    sessions: Mutex<HashMap<String, Session>>,
    /// キーはQRログインセッションID。
    qr_logins: Mutex<HashMap<String, QrLoginEntry>>,
    login_mode: Mutex<String>,
}

impl Default for AuthStore {
    fn default() -> Self {
        AuthStore {
            pending: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            qr_logins: Mutex::new(HashMap::new()),
            login_mode: Mutex::new(LOGIN_MODE_OTP.to_string()),
        }
    }
}

pub enum RequestOtpOutcome {
    Issued(String),
}

impl AuthStore {
    /// `email`宛のOTPを発行し、送信すべきコードを返す(ハッシュのみ保存)。
    pub fn request_otp(&self, email: &str) -> RequestOtpOutcome {
        let code = generate_otp();
        let mut pending = self.pending.lock().unwrap();
        pending.insert(email.to_string(), PendingOtp { code_hash: hash(&code), expires_at: Instant::now() + OTP_TTL, attempts: 0 });
        RequestOtpOutcome::Issued(code)
    }

    pub fn consume_otp(&self, email: &str, code: &str) -> Result<(), VerifyError> {
        let mut pending = self.pending.lock().unwrap();
        let Some(entry) = pending.get_mut(email) else {
            return Err(VerifyError::NotRequested);
        };
        if Instant::now() > entry.expires_at {
            pending.remove(email);
            return Err(VerifyError::Expired);
        }
        if entry.attempts >= MAX_ATTEMPTS {
            pending.remove(email);
            return Err(VerifyError::TooManyAttempts);
        }
        entry.attempts += 1;
        if entry.code_hash != hash(code) {
            return Err(VerifyError::Mismatch);
        }
        pending.remove(email);
        Ok(())
    }

    /// `email`に対するセッションを発行する(このメールアドレスが管理者であるか
    /// の判定は呼び出し側の責務)。
    pub fn create_session(&self, email: &str) -> String {
        let token = generate_token();
        self.sessions.lock().unwrap().insert(token.clone(), Session { email: email.to_string(), expires_at: Instant::now() + SESSION_TTL });
        token
    }

    /// 有効なセッションに紐づくメールアドレスを返す(期限切れなら破棄し
    /// `None`)。
    pub fn session_email(&self, token: &str) -> Option<String> {
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.get(token)?;
        if Instant::now() > session.expires_at {
            sessions.remove(token);
            return None;
        }
        Some(session.email.clone())
    }

    pub fn logout(&self, token: &str) {
        self.sessions.lock().unwrap().remove(token);
    }

    pub fn login_mode(&self) -> String {
        self.login_mode.lock().unwrap().clone()
    }

    /// `is_valid_login_mode`で検証済みの値のみ渡すこと(呼び出し側の
    /// `main.rs`ハンドラで検証する)。
    pub fn set_login_mode(&self, mode: &str) {
        *self.login_mode.lock().unwrap() = mode.to_string();
    }

    /// QR確認ログインセッションを開始する(`account_email`宛)。このアプリ
    /// では常に「OTP検証成功後の第二要素」としてのみ呼ばれる設計
    /// (モジュールdoc参照、QR単体でのログイン開始は意図的に持たない)。
    /// 返り値はQRコードに埋め込むURLの末尾に付ける短命なID。
    pub fn start_qr_login(&self, account_email: &str) -> String {
        let mut logins = self.qr_logins.lock().unwrap();
        logins.retain(|_, v| v.expires_at > Instant::now());
        let id = generate_token();
        logins.insert(
            id.clone(),
            QrLoginEntry { account_email: account_email.to_string(), confirmed: false, expires_at: Instant::now() + QR_LOGIN_TTL },
        );
        id
    }

    /// QR確認ページが「どのアカウントのログインを確認しようとしているか」
    /// を表示するための、マスク済みメールアドレス。
    pub fn qr_login_masked_email(&self, id: &str) -> Option<String> {
        let logins = self.qr_logins.lock().unwrap();
        let entry = logins.get(id).filter(|e| e.expires_at > Instant::now())?;
        Some(mask_email(&entry.account_email))
    }

    /// QR確認ページの読み込み時に自動的に呼ばれる(ボタン操作は不要な
    /// 設計)。このQRセッションを「確認済み」にするだけで、まだ
    /// セッショントークンは発行しない(確認端末とプライマリ端末が
    /// 別物のため、実際のトークンはプライマリ端末側の`finish_qr_login`
    /// が発行する)。
    pub fn confirm_qr_login(&self, id: &str) -> Result<(), VerifyError> {
        let mut logins = self.qr_logins.lock().unwrap();
        let Some(entry) = logins.get_mut(id) else {
            return Err(VerifyError::NotRequested);
        };
        if Instant::now() > entry.expires_at {
            logins.remove(id);
            return Err(VerifyError::Expired);
        }
        entry.confirmed = true;
        Ok(())
    }

    /// プライマリ端末が数秒おきにポーリングして、確認済みになったかを見る。
    pub fn qr_login_status(&self, id: &str) -> Option<bool> {
        let logins = self.qr_logins.lock().unwrap();
        logins.get(id).filter(|e| e.expires_at > Instant::now()).map(|e| e.confirmed)
    }

    /// 確認済みであることを見たプライマリ端末が呼ぶ——実際のセッション
    /// トークンをここで初めて発行する(QRセッション自体は使い捨てで
    /// 直後に消費・削除する)。
    pub fn finish_qr_login(&self, id: &str) -> Result<String, VerifyError> {
        let mut logins = self.qr_logins.lock().unwrap();
        let Some(entry) = logins.get(id) else {
            return Err(VerifyError::NotRequested);
        };
        if Instant::now() > entry.expires_at {
            logins.remove(id);
            return Err(VerifyError::Expired);
        }
        if !entry.confirmed {
            return Err(VerifyError::Mismatch);
        }
        let account_email = entry.account_email.clone();
        logins.remove(id);
        drop(logins);
        Ok(self.create_session(&account_email))
    }
}

#[derive(Debug)]
pub enum VerifyError {
    NotRequested,
    Expired,
    TooManyAttempts,
    Mismatch,
}

impl VerifyError {
    pub fn message(&self) -> &'static str {
        match self {
            VerifyError::NotRequested => "この連絡先宛にOTPは発行されていません。",
            VerifyError::Expired => "OTPの有効期限が切れました。再度リクエストしてください。",
            VerifyError::TooManyAttempts => "試行回数の上限を超えました。再度リクエストしてください。",
            VerifyError::Mismatch => "コードが正しくありません。",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otp_roundtrip_succeeds_with_correct_code() {
        let store = AuthStore::default();
        let RequestOtpOutcome::Issued(code) = store.request_otp("admin@example.com");
        store.consume_otp("admin@example.com", &code).unwrap();
        let token = store.create_session("admin@example.com");
        assert_eq!(store.session_email(&token).as_deref(), Some("admin@example.com"));
    }

    #[test]
    fn wrong_code_is_rejected_and_does_not_consume_the_otp() {
        let store = AuthStore::default();
        let RequestOtpOutcome::Issued(code) = store.request_otp("admin@example.com");
        assert!(matches!(store.consume_otp("admin@example.com", "000000"), Err(VerifyError::Mismatch)));
        assert!(store.consume_otp("admin@example.com", &code).is_ok());
    }

    #[test]
    fn exceeding_max_attempts_invalidates_the_otp() {
        let store = AuthStore::default();
        let RequestOtpOutcome::Issued(_code) = store.request_otp("admin@example.com");
        for _ in 0..MAX_ATTEMPTS {
            let _ = store.consume_otp("admin@example.com", "000000");
        }
        assert!(matches!(
            store.consume_otp("admin@example.com", "000000"),
            Err(VerifyError::TooManyAttempts) | Err(VerifyError::NotRequested)
        ));
    }

    #[test]
    fn logout_invalidates_the_session_token() {
        let store = AuthStore::default();
        let RequestOtpOutcome::Issued(code) = store.request_otp("admin@example.com");
        store.consume_otp("admin@example.com", &code).unwrap();
        let token = store.create_session("admin@example.com");
        store.logout(&token);
        assert!(store.session_email(&token).is_none());
    }

    #[test]
    fn unknown_token_is_invalid() {
        let store = AuthStore::default();
        assert!(store.session_email("does-not-exist").is_none());
    }

    #[test]
    fn login_mode_defaults_to_otp_and_can_be_changed() {
        let store = AuthStore::default();
        assert_eq!(store.login_mode(), LOGIN_MODE_OTP);
        store.set_login_mode(LOGIN_MODE_OTP_QR);
        assert_eq!(store.login_mode(), LOGIN_MODE_OTP_QR);
    }

    #[test]
    fn is_valid_login_mode_only_accepts_otp_and_otp_qr() {
        assert!(is_valid_login_mode("otp"));
        assert!(is_valid_login_mode("otp_qr"));
        // RS-ECは複数アカウント制のため、QR単体(no-OTP)・パスワード無し
        // いずれも意図的に非対応(モジュールdoc参照)。
        assert!(!is_valid_login_mode("qr"));
        assert!(!is_valid_login_mode("none"));
        assert!(!is_valid_login_mode("password"));
    }

    #[test]
    fn qr_login_as_second_factor_full_flow_issues_a_session() {
        let store = AuthStore::default();
        let RequestOtpOutcome::Issued(code) = store.request_otp("member@example.com");
        store.consume_otp("member@example.com", &code).unwrap();
        // OTP検証成功後、セッションを即発行せずQRを第二要素として開始する。
        let id = store.start_qr_login("member@example.com");
        assert_eq!(store.qr_login_masked_email(&id).as_deref(), Some("me***@example.com"));
        assert_eq!(store.qr_login_status(&id), Some(false));
        store.confirm_qr_login(&id).unwrap();
        assert_eq!(store.qr_login_status(&id), Some(true));
        let token = store.finish_qr_login(&id).unwrap();
        assert_eq!(store.session_email(&token).as_deref(), Some("member@example.com"));
        // QRセッションは使い捨て(finish後は消費済み)。
        assert!(matches!(store.finish_qr_login(&id), Err(VerifyError::NotRequested)));
    }

    #[test]
    fn finishing_qr_login_before_confirmation_fails() {
        let store = AuthStore::default();
        let id = store.start_qr_login("member@example.com");
        assert!(matches!(store.finish_qr_login(&id), Err(VerifyError::Mismatch)));
    }

    #[test]
    fn unknown_qr_login_id_is_rejected() {
        let store = AuthStore::default();
        assert!(store.qr_login_masked_email("does-not-exist").is_none());
        assert!(matches!(store.confirm_qr_login("does-not-exist"), Err(VerifyError::NotRequested)));
        assert!(matches!(store.finish_qr_login("does-not-exist"), Err(VerifyError::NotRequested)));
    }

    #[test]
    fn different_accounts_get_independent_sessions() {
        let store = AuthStore::default();
        let RequestOtpOutcome::Issued(admin_code) = store.request_otp("admin@example.com");
        let RequestOtpOutcome::Issued(member_code) = store.request_otp("member@example.com");
        store.consume_otp("admin@example.com", &admin_code).unwrap();
        store.consume_otp("member@example.com", &member_code).unwrap();
        let admin_token = store.create_session("admin@example.com");
        let member_token = store.create_session("member@example.com");
        assert_eq!(store.session_email(&admin_token).as_deref(), Some("admin@example.com"));
        assert_eq!(store.session_email(&member_token).as_deref(), Some("member@example.com"));
    }
}
