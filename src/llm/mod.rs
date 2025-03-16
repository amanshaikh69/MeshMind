// LLM module for language model related functionality
use actix_web::{post, web, HttpResponse, Error};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use chrono::Utc;
use crate::conversation::{ChatMessage, CONVERSATION_STORE};

const OLLAMA_HOST: &str = "http://127.0.0.1:11434";

#[derive(Deserialize)]
pub struct ChatRequest {
    message: String,
    sender: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
}

#[derive(Serialize, Deserialize, Debug)]
struct OllamaResponse {
    model: String,
    created_at: String,
    message: OllamaMessage,
    done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    done_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_duration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    load_duration: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_eval_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eval_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eval_duration: Option<i64>,
}

#[post("/chat")]
pub async fn chat(req: web::Json<ChatRequest>) -> Result<HttpResponse, Error> {
    let client = Client::new();
    let ollama_req = OllamaRequest {
        model: "qwen2.5-coder:7b".to_string(),
        messages: vec![
            OllamaMessage {
                role: "user".to_string(),
                content: req.message.clone(),
            }
        ],
    };
    
    let response = match client
        .post(format!("{}/api/chat", OLLAMA_HOST))
        .json(&ollama_req)
        .send()
        .await {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("Error: Failed to connect to LLM server: {}", e);
                return Ok(HttpResponse::ServiceUnavailable()
                    .json(serde_json::json!({
                        "error": "Failed to connect to LLM server",
                        "details": format!("Connection error: {}. Please ensure Ollama is running on {}", e, OLLAMA_HOST)
                    })));
            }
        };

    if !response.status().is_success() {
        eprintln!("Error: LLM server returned status: {}", response.status());
        return Ok(HttpResponse::BadGateway()
            .json(serde_json::json!({
                "error": "LLM server error",
                "status": response.status().as_u16(),
                "details": format!("Server returned: {}", response.status())
            })));
    }
    
    let mut full_response = String::new();
    let body = response.text().await.map_err(|e| {
        eprintln!("Error: Failed to get LLM response: {}", e);
        actix_web::error::ErrorInternalServerError(e)
    })?;

    let mut response_complete = false;
    for line in body.lines() {
        if let Ok(resp) = serde_json::from_str::<OllamaResponse>(line) {
            full_response.push_str(&resp.message.content);
            if resp.done {
                response_complete = true;
            }
        }
    }

    if !response_complete {
        eprintln!("Error: Incomplete response from LLM");
        return Ok(HttpResponse::InternalServerError()
            .json(serde_json::json!({
                "error": "Incomplete response from LLM",
                "details": "The model response stream ended unexpectedly"
            })));
    }

    if full_response.trim().is_empty() {
        eprintln!("Error: Empty response from LLM");
        return Ok(HttpResponse::InternalServerError()
            .json(serde_json::json!({
                "error": "Empty response from LLM",
                "details": "The model returned an empty response. This might indicate an issue with the model loading or processing."
            })));
    }

    let chat_message = ChatMessage {
        content: full_response,
        timestamp: Utc::now(),
        sender: req.sender.clone(),
    };

    CONVERSATION_STORE.add_message("local".to_string(), chat_message.clone()).await;

    Ok(HttpResponse::Ok().json(chat_message))
}