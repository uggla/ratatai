// src/ai.rs

use google_ai_rs::{GenerativeModel, genai::Response};
use std::time::Duration;
use tokio::time::sleep;

// Comment out these lines if you don't want to compile with the google_ai_rs dependency
// use google_ai_rs::{Client, GenerativeModel, genai::Response};

/// Simulates a response from Gemini.
/// In a future version, this function will contain the actual API call.
pub async fn get_gemini_response_static() -> anyhow::Result<String> {
    sleep(Duration::from_secs(2)).await; // Simulate a network delay
    Ok("This is a static response from Gemini for now. The actual API call is commented out to facilitate UI development. You can edit this text if you wish.".to_string())
}

// The old commented-out function can be placed here if you want to keep it for reference:
pub async fn get_gemini_response<'a>(
    model: GenerativeModel<'a>,
    prompt: String,
) -> anyhow::Result<Response> {
    let response = model
        //         .generate_content("What is Rust and why is it popular?")
        .generate_content(prompt)
        .await?;
    Ok(response)
}
