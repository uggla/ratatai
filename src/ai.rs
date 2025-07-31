// src/ai.rs

use google_ai_rs::{GenerativeModel, genai::Response};
use std::time::Duration;
use tokio::time::sleep;

// Commenter ces lignes si tu ne veux pas compiler avec la dépendance google_ai_rs
// use google_ai_rs::{Client, GenerativeModel, genai::Response};

/// Simule une réponse de Gemini.
/// Dans une version future, cette fonction contiendrait l'appel réel à l'API.
pub async fn get_gemini_response_static() -> Result<String, Box<dyn std::error::Error>> {
    sleep(Duration::from_secs(2)).await; // Simule un délai réseau
    Ok("Ceci est une réponse statique de Gemini pour l'instant. L'appel réel à l'API est commenté pour faciliter le développement de l'interface utilisateur. Vous pouvez éditer ce texte si vous le souhaitez.".to_string())
}

// L'ancienne fonction commentée peut être mise ici si tu veux la garder pour référence :
pub async fn get_gemini_response<'a>(
    model: GenerativeModel<'a>,
) -> Result<Response, Box<dyn std::error::Error>> {
    let response = model
        .generate_content("Qu'est-ce que Rust et pourquoi est-il populaire ?")
        .await?;
    Ok(response)
}
