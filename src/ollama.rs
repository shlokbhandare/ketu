use serde::{Deserialize, Serialize};
use std::time::Duration;



#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

pub async fn generate(url: &str, prompt: &str, model: &str) -> Result<String, reqwest::Error> {
    // TEMP: remove after testing distributed health sharing with a slow backend
    if url.contains("11435") {
        tokio::time::sleep(Duration::from_millis(2500)).await;
    }

    let body = GenerateRequest {
        model,
        prompt,
        stream: false,
    };

    let response = reqwest::Client::new()
        .post(format!("{}/api/generate", url))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<GenerateResponse>()
        .await?;

    Ok(response.response)
}
