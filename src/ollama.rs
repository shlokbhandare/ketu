use serde::{Deserialize, Serialize};

const OLLAMA_GENERATE_URL: &str = "http://localhost:11434/api/generate";

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

pub async fn generate(prompt: &str, model: &str) -> Result<String, reqwest::Error> {
    let body = GenerateRequest {
        model,
        prompt,
        stream: false,
    };

    let response = reqwest::Client::new()
        .post(OLLAMA_GENERATE_URL)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<GenerateResponse>()
        .await?;

    Ok(response.response)
}
