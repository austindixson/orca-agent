use serde_json::Value;

use crate::config::OrcaConfig;

pub fn auth_headers(cfg: &OrcaConfig) -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    h.insert(reqwest::header::ACCEPT, "application/json".parse().unwrap());
    if let Some(ref t) = cfg.bridge.token {
        if !t.trim().is_empty() {
            let v = format!("Bearer {}", t.trim());
            h.insert(
                reqwest::header::AUTHORIZATION,
                v.parse().unwrap(),
            );
        }
    }
    h
}

pub async fn get_json(cfg: &OrcaConfig, path: &str) -> anyhow::Result<(u16, Value)> {
    let base = cfg.base_url();
    let url = format!("{}{}", base, path);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    let r = client.get(&url).headers(auth_headers(cfg)).send().await?;
    let status = r.status().as_u16();
    let text = r.text().await.unwrap_or_default();
    let body: Value = if text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text))
    };
    Ok((status, body))
}

pub async fn post_json(
    cfg: &OrcaConfig,
    path: &str,
    body: Value,
) -> anyhow::Result<(u16, Value)> {
    let base = cfg.base_url();
    let url = format!("{}{}", base, path);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;
    let r = client
        .post(&url)
        .headers(auth_headers(cfg))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&body)
        .send()
        .await?;
    let status = r.status().as_u16();
    let text = r.text().await.unwrap_or_default();
    let out: Value = if text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text))
    };
    Ok((status, out))
}
