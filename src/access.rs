//! アクセス制御(閲覧・編集許可)。[`RGit`](https://github.com/aon-co-jp/RGit)の
//! `src/access.rs`・`RS-Chiketto`の同名モジュールと同じ設計思想を、
//! EC-CUBE相当の「商品カタログ全体」向けに単一リソース(`catalog`)として
//! 簡略化して移植(RS-Chikettoの`project`単位の粒度は今回のEC商品には
//! 不要と判断、CLAUDE.md参照)。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Private,
    Public,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Private
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountPermission {
    pub allow_view: bool,
    pub allow_edit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AccessConfig {
    pub mode: Mode,
    pub allow_view: bool,
    pub allow_edit: bool,
    pub accounts: HashMap<String, AccountPermission>,
}

impl Default for AccessConfig {
    fn default() -> Self {
        Self { mode: Mode::Private, allow_view: false, allow_edit: false, accounts: HashMap::new() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    View,
    Edit,
}

/// `config`と(ログイン中なら)アカウントのメールアドレスから、`need`の
/// 操作が許可されるかを判定する。管理者ログイン済みかどうかは呼び出し側
/// (`main.rs`)が別途見る——この関数は「public公開ルール、またはアカウント
/// 個別許可として許可されるか」だけを見る。
pub fn is_allowed(config: &AccessConfig, need: Need, account_email: Option<&str>) -> bool {
    if let Some(email) = account_email {
        if let Some(perm) = config.accounts.get(email) {
            let flag = match need {
                Need::View => perm.allow_view,
                Need::Edit => perm.allow_edit,
            };
            if flag {
                return true;
            }
        }
    }
    let flag = match need {
        Need::View => config.allow_view,
        Need::Edit => config.allow_edit,
    };
    flag && config.mode == Mode::Public
}

fn access_path(data_root: &Path) -> PathBuf {
    data_root.join("catalog-access.json")
}

pub async fn load(data_root: &Path) -> AccessConfig {
    match tokio::fs::read(access_path(data_root)).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => AccessConfig::default(),
    }
}

pub async fn save(data_root: &Path, config: &AccessConfig) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(config).expect("AccessConfig serialization is infallible");
    tokio::fs::write(access_path(data_root), bytes).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_catalog_denies_regardless_of_flags() {
        let config = AccessConfig { mode: Mode::Private, allow_view: true, allow_edit: true, accounts: HashMap::new() };
        assert!(!is_allowed(&config, Need::View, None));
        assert!(!is_allowed(&config, Need::Edit, None));
    }

    #[test]
    fn public_catalog_respects_view_and_edit_flags_independently() {
        let config = AccessConfig { mode: Mode::Public, allow_view: true, allow_edit: false, accounts: HashMap::new() };
        assert!(is_allowed(&config, Need::View, None));
        assert!(!is_allowed(&config, Need::Edit, None));
    }

    #[test]
    fn account_specific_grant_works_even_when_catalog_is_private() {
        let mut config = AccessConfig { mode: Mode::Private, allow_view: false, allow_edit: false, accounts: HashMap::new() };
        config.accounts.insert("member@example.com".to_string(), AccountPermission { allow_view: true, allow_edit: false });
        assert!(is_allowed(&config, Need::View, Some("member@example.com")));
        assert!(!is_allowed(&config, Need::Edit, Some("member@example.com")));
        assert!(!is_allowed(&config, Need::View, Some("someone-else@example.com")));
    }
}
