use serde::{Deserialize, Serialize};



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
