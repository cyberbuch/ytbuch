//! HTTP към ЕЦБ, VIES и Mistral. Клиентът ползва системните корени на rustls.

use std::time::Duration;

use serde_json::Value;

fn client(secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(secs))
        .user_agent("ytbuch/0.1")
        .build()
        .map_err(|e| e.to_string())
}

pub async fn get_text(url: &str, secs: u64) -> Result<(u16, String), String> {
    let resp = client(secs)?
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    Ok((status, body))
}

pub async fn post_json(url: &str, bearer: &str, body: &Value, secs: u64) -> Result<(u16, String), String> {
    let resp = client(secs)?
        .post(url)
        .bearer_auth(bearer)
        .json(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    Ok((status, text))
}
