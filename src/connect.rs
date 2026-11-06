//! Connect プロトコルの「単純呼び出し(unary)」入口(REST全廃、2026-11-06)。
//!
//! 形式は `POST /<パッケージ>.<Service>/<Method>`、`content-type: application/json`、本文は JSON オブジェクト。
//! 例: `POST /rs_ec.v1.Posts/Get` に `{"id": 3}`。認証は従来どおり `Authorization: Bearer <セッション>`。
//! **JSON の読み込みは RS-JSON の `from_str_secure`(サイズ・深さ・重複キーの上限つき)**。
//! 成功は HTTP 200 + JSON(文字列だけを返す操作は `{"message": "..."}`)。失敗は `{"code": "...", "message": "..."}`
//! (HTTP ステータスは Connect の対応表: 400/401/403/404/409/413/429/503 など)。
//!
//! 旧 REST の経路(`/api/...`)は公開ルーターから削除した。内部の処理(ハンドラ)は非公開の内部ルーターに残し、
//! この入口が「JSON の欄 → 内部ルーターのパス引数・クエリ・本文」へ変換して呼ぶ(公開されない)。

use std::sync::Arc;

use poem::endpoint::BoxEndpoint;
use poem::http::{header, Method, StatusCode};
use poem::{Endpoint, Request, Response};
use serde_json::{Map, Value};

/// 本文の上限(64 KiB)。
const MAX_BODY: usize = 64 * 1024;

/// 1つの Connect メソッドの定義。
#[derive(Clone, Copy)]
pub struct Def {
    pub service: &'static str,
    pub method: &'static str,
    /// 内部ルーターのパス(`/Posts/Get/:id` の形)。`:名前` は JSON の同名の欄から取る。
    pub inner: &'static str,
    /// 元が GET の操作か(残りの欄をクエリ文字列にして渡す)。
    pub query: bool,
}

pub struct Call {
    pub inner: Arc<BoxEndpoint<'static, Response>>,
    pub def: Def,
    pub package: &'static str,
}

fn status_of(code: &str) -> StatusCode {
    match code {
        "invalid_argument" | "failed_precondition" | "out_of_range" => StatusCode::BAD_REQUEST,
        "unauthenticated" => StatusCode::UNAUTHORIZED,
        "permission_denied" => StatusCode::FORBIDDEN,
        "not_found" => StatusCode::NOT_FOUND,
        "aborted" | "already_exists" => StatusCode::CONFLICT,
        "resource_exhausted" => StatusCode::TOO_MANY_REQUESTS,
        "unavailable" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn code_of(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 | 415 | 422 => "invalid_argument",
        401 => "unauthenticated",
        403 => "permission_denied",
        404 => "not_found",
        405 | 501 => "unimplemented",
        409 => "aborted",
        413 | 429 => "resource_exhausted",
        502 | 503 | 504 => "unavailable",
        s if s >= 500 => "internal",
        _ => "unknown",
    }
}

fn json_response(status: StatusCode, v: &Value) -> Response {
    Response::builder()
        .status(status)
        .content_type("application/json")
        .header(header::CACHE_CONTROL, "no-store")
        .body(rust_json::fast::to_vec(v).unwrap_or_else(|_| b"{}".to_vec()))
}

fn fail_with(status: StatusCode, code: &str, message: &str) -> Response {
    json_response(status, &serde_json::json!({ "code": code, "message": message }))
}

fn fail(code: &str, message: &str) -> Response {
    fail_with(status_of(code), code, message)
}

fn pct(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

impl Endpoint for Call {
    type Output = Response;

    async fn call(&self, req: Request) -> poem::Result<Response> {
        let def = self.def;
        let _ = self.package;
        if req.method() != Method::POST {
            return Ok(fail_with(StatusCode::METHOD_NOT_ALLOWED, "unimplemented", "POST only"));
        }
        let is_json = req
            .header(header::CONTENT_TYPE)
            .map(|v| v.split(';').next().unwrap_or("").trim().eq_ignore_ascii_case("application/json"))
            .unwrap_or(false);
        if !is_json {
            return Ok(fail_with(StatusCode::UNSUPPORTED_MEDIA_TYPE, "invalid_argument", "content-type must be application/json"));
        }
        let auth = req.header(header::AUTHORIZATION).map(|s| s.to_string());
        let bytes = match req.into_body().into_bytes_limit(MAX_BODY).await {
            Ok(b) => b,
            Err(_) => return Ok(fail("resource_exhausted", "request body too large")),
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(t) => t.trim(),
            Err(_) => return Ok(fail("invalid_argument", "body is not UTF-8")),
        };
        let limits = rust_json::Limits { max_bytes: MAX_BODY, max_depth: 16, reject_duplicate_keys: true };
        let mut obj: Map<String, Value> = if text.is_empty() {
            Map::new()
        } else {
            match rust_json::from_str_secure::<Value>(text, &limits) {
                Ok(Value::Object(m)) => m,
                Ok(_) => return Ok(fail("invalid_argument", "body must be a JSON object")),
                Err(e) => return Ok(fail("invalid_argument", &format!("invalid JSON: {e}"))),
            }
        };

        // パス引数(`:名前`)を JSON の欄から取り出す。
        let mut path = String::new();
        for seg in def.inner.split('/').filter(|s| !s.is_empty()) {
            path.push('/');
            if let Some(name) = seg.strip_prefix(':') {
                match obj.remove(name).as_ref().and_then(scalar) {
                    Some(v) if !v.is_empty() => path.push_str(&pct(&v)),
                    _ => return Ok(fail("invalid_argument", &format!("missing field: {name}"))),
                }
            } else {
                path.push_str(seg);
            }
        }

        let mut builder = Request::builder().method(Method::POST);
        if let Some(a) = auth {
            builder = builder.header(header::AUTHORIZATION, a);
        }
        let inner_req = if def.query {
            let q: Vec<String> = obj.iter().filter_map(|(k, v)| scalar(v).map(|s| format!("{}={}", pct(k), pct(&s)))).collect();
            let uri = if q.is_empty() { path } else { format!("{path}?{}", q.join("&")) };
            builder.uri_str(uri).finish()
        } else {
            builder
                .uri_str(path)
                .content_type("application/json")
                .body(rust_json::fast::to_vec(&Value::Object(obj)).unwrap_or_else(|_| b"{}".to_vec()))
        };

        let resp = self.inner.get_response(inner_req).await;
        let status = resp.status();
        let body = resp.into_body().into_string().await.unwrap_or_default();
        if status == StatusCode::NOT_FOUND && body.is_empty() {
            return Ok(fail("not_found", "not found"));
        }
        if status.is_success() {
            let v = match serde_json::from_str::<Value>(&body) {
                Ok(v) => v,
                Err(_) => serde_json::json!({ "message": body }),
            };
            return Ok(json_response(StatusCode::OK, &v));
        }
        let message = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
            .unwrap_or(body);
        Ok(fail_with(status, code_of(status), &message))
    }
}
