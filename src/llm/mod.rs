// LLM module for language model related functionality
use actix_web::{post, web, HttpResponse, Error};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use chrono::Utc;
use crate::conversation::{ChatMessage, CONVERSATION_STORE, HostInfo, MessageType};
use crate::tcp::LLM_CONNECTIONS;
use std::time::Duration;
use hostname;

// Always treat this as the local Ollama base URL
fn local_ollama_base() -> String {
    "http://127.0.0.1:11434".to_string()
}

// Call a remote peer's /api/chat endpoint using our ChatRequest shape.
// This is required because remote instances expect ChatRequest, not OllamaRequest.
async fn try_remote_peer_chat(message: &str, sender: &str) -> Result<String, String> {
    let connections = LLM_CONNECTIONS.lock().await;
    if connections.is_empty() {
        return Err("No remote LLM connections available".to_string());
    }

    #[derive(Serialize)]
    struct RemoteChatReq<'a> { message: &'a str, sender: &'a str }

    for (peer, (host, port)) in connections.iter() {
        let client = Client::builder()
            .timeout(REMOTE_REQUEST_TIMEOUT)
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let remote_url = format!("http://{}:{}/api/chat", host, port);
        println!("Attempting to use remote LLM at {}", remote_url);

        match client.post(&remote_url)
            .header("x-peer-llm", "1")
            .json(&RemoteChatReq { message, sender })
            .send()
            .await {
                Ok(response) => {
                    if response.status().is_success() {
                        let body = response.text().await
                            .map_err(|e| format!("Failed to get remote chat response: {}", e))?;
                        // Remote instance returns our ChatMessage JSON
                        if let Ok(msg) = serde_json::from_str::<crate::conversation::ChatMessage>(&body) {
                            if !msg.content.trim().is_empty() {
                                println!("Successfully used remote LLM from peer {} (ChatMessage)", peer);
                                return Ok(msg.content);
                            }
                        }
                        // Fallback to Ollama stream parsing just in case
                        match process_ollama_response(&body) {
                            Ok(result) => {
                                println!("Successfully used remote LLM from peer {} (Ollama stream)", peer);
                                return Ok(result)
                            },
                            Err(e) => println!("Failed to process remote chat response from {}: {}", peer, e),
                        }
                    } else {
                        println!("Remote LLM {} returned error status: {}", peer, response.status());
                    }
                },
                Err(e) => println!("Failed to connect to remote LLM {}: {}", peer, e),
            }
    }
    Err("No available LLM connections responded successfully".to_string())
}

const REMOTE_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
pub struct ChatRequest {
    message: String,
    sender: String,
    #[serde(default)]
    filename: Option<String>,
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

// Check localhost only for local availability
async fn is_local_ollama_available() -> bool {
    if let Ok(client) = Client::builder()
        .timeout(Duration::from_secs(2))
        .build() 
    {
        let url = format!("{}/api/tags", local_ollama_base());
        match client.get(&url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    } else {
        false
    }
}

async fn try_local_llm(req: &OllamaRequest) -> Result<String, String> {
    let client = Client::new();
    let url = local_ollama_base();
    let response = client
        .post(format!("{}/api/chat", url))
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to local LLM: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Local LLM error: {}", response.status()));
    }

    let body = response.text().await
        .map_err(|e| format!("Failed to get local LLM response: {}", e))?;

    process_ollama_response(&body)
}

async fn try_remote_llm(req: &OllamaRequest) -> Result<String, String> {
    let connections = LLM_CONNECTIONS.lock().await;
    
    if connections.is_empty() {
        return Err("No remote LLM connections available".to_string());
    }

    // Try each known LLM connection
    for (peer, (host, port)) in connections.iter() {
        let client = Client::builder()
            .timeout(REMOTE_REQUEST_TIMEOUT)
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let remote_url = format!("http://{}:{}/api/chat", host, port);
        
        println!("Attempting to use remote LLM at {}", remote_url);
        
        match client.post(&remote_url)
            .json(&req)
            .send()
            .await {
                Ok(response) => {
                    if response.status().is_success() {
                        let body = response.text().await
                            .map_err(|e| format!("Failed to get remote LLM response: {}", e))?;

                        // First try parsing as our app's ChatMessage JSON (when calling peer's /api/chat)
                        if let Ok(msg) = serde_json::from_str::<crate::conversation::ChatMessage>(&body) {
                            if !msg.content.trim().is_empty() {
                                println!("Successfully used remote LLM from peer {} (ChatMessage)", peer);
                                return Ok(msg.content);
                            }
                        }

                        // Fallback: handle direct Ollama streaming JSON lines (if remote proxied raw)
                        match process_ollama_response(&body) {
                            Ok(result) => {
                                println!("Successfully used remote LLM from peer {} (Ollama stream)", peer);
                                return Ok(result)
                            },
                            Err(e) => println!("Failed to process remote response from {}: {}", peer, e),
                        }
                    } else {
                        println!("Remote LLM {} returned error status: {}", peer, response.status());
                    }
                },
                Err(e) => println!("Failed to connect to remote LLM {}: {}", peer, e),
            }
    }
    
    Err("No available LLM connections responded successfully".to_string())
}

fn process_ollama_response(body: &str) -> Result<String, String> {
    let mut full_response = String::new();
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
        return Err("Incomplete response from LLM".to_string());
    }

    if full_response.trim().is_empty() {
        return Err("Empty response from LLM".to_string());
    }

    Ok(full_response)
}

#[post("/chat")]
pub async fn chat(req: web::Json<ChatRequest>) -> Result<HttpResponse, Error> {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());
    
    let ip_address = std::net::TcpStream::connect("8.8.8.8:53")
        .and_then(|s| s.local_addr())
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let host_info = HostInfo {
        hostname: hostname.clone(),
        ip_address: ip_address.clone(),
        is_llm_host: is_local_ollama_available().await,
    };

    // If filename is provided, load file content and prepend to prompt
    let mut prompt = req.message.clone();
    if let Some(filename) = &req.filename {
        match crate::persistence::get_file_content(filename).await {
            Ok(Some(content)) => {
                // Safer handling: treat PDFs and unreadable binaries via base64 preview
                let file_extension = filename.split('.').last().unwrap_or("").to_lowercase();
                if file_extension == "pdf" {
                    use base64::engine::general_purpose::STANDARD;
                    use base64::Engine;
                    let preview_len = content.len().min(8 * 1024); // 8KB preview
                    let b64 = STANDARD.encode(&content[..preview_len]);
                    prompt = format!(
                        "You are analyzing a PDF file named '{}'. The following is a base64 preview of the first {} bytes. If exact content is needed, infer structure (title, sections, abstracts, headings) and provide a high-level analysis based on this preview.\n\n[PDF_BASE64_PREVIEW]\n{}\n[/PDF_BASE64_PREVIEW]\n\n{}",
                        filename,
                        preview_len,
                        b64,
                        req.message
                    );
                } else {
                    // Try to decode as UTF-8, fallback to base64 if not text
                    let file_text = String::from_utf8_lossy(&content);
                    if file_text.is_empty() || file_text.contains('\u{FFFD}') {
                        use base64::engine::general_purpose::STANDARD;
                        use base64::Engine;
                        let preview_len = content.len().min(8 * 1024);
                        let b64 = STANDARD.encode(&content[..preview_len]);
                        prompt = format!("File '{}' appears binary. Base64 preview ({} bytes):\n{}\n\n{}", filename, preview_len, b64, req.message);
                    } else {
                        let preview = if file_text.len() > 4000 { &file_text[..4000] } else { &file_text };
                        prompt = format!("File content (analyzing file '{}'):\n{}\n\n{}", filename, preview, req.message);
                    }
                }
            }
            Ok(None) => {
                prompt = format!("(File '{}' not found)\n\n{}", filename, prompt);
            }
            Err(e) => {
                prompt = format!("(Error loading file '{}': {})\n\n{}", filename, e, prompt);
            }
        }
    }

    // Create user question message
    let question_message = ChatMessage {
        content: prompt.clone(),
        timestamp: Utc::now(),
        sender: req.sender.clone(),
        message_type: MessageType::Question,
        host_info: host_info.clone(),
    };

    // Save the question
    CONVERSATION_STORE.add_message("local".to_string(), question_message).await;

    // Use llama2 model - Ollama will handle optimization automatically
    let model_name = "llama2".to_string();
    
    let ollama_req = OllamaRequest {
        model: model_name,
        messages: vec![
            OllamaMessage {
                role: "system".to_string(),
                content: "You are an expert file analysis assistant specializing in PDF and academic document analysis. Your capabilities include:
                1. PDF Analysis: Extract and interpret key information from PDF content, focusing on academic and technical details
                2. Research Paper Analysis: Identify methodology, findings, and conclusions
                3. Technical Document Processing: Handle complex technical content and diagrams
                4. Error Handling: When content is partially available or corrupted, provide analysis based on available information
                5. Large File Management: For large documents, focus on available previews and provide meaningful insights
                
                When analyzing files:
                - Always acknowledge the file type and size
                - Provide structured analysis based on available content
                - If content is incomplete, focus on visible patterns and structure
                - For PDFs about neural networks or medical imaging, pay special attention to methodology and technical details
                
                Maintain a professional and technical tone, and be clear about any limitations in the analysis.".to_string(),
            },
            OllamaMessage {
                role: "user".to_string(),
                content: prompt,
            }
        ],
    };

    // Check if we have local Ollama first
    let has_local_llm = is_local_ollama_available().await;
    
    let response = if has_local_llm {
        // Try local first if available
        match try_local_llm(&ollama_req).await {
            Ok(response) => response,
            Err(local_error) => {
                // If local fails, try remote
                match try_remote_peer_chat(&ollama_req.messages.last().unwrap().content, &req.sender).await {
                    Ok(response) => response,
                    Err(remote_error) => {
                        return Ok(HttpResponse::ServiceUnavailable()
                            .json(serde_json::json!({
                                "error": "No available LLM service",
                                "details": format!("Local error: {}. Remote error: {}", local_error, remote_error)
                            })));
                    }
                }
            }
        }
    } else {
        // No local LLM, try remote directly
        match try_remote_peer_chat(&ollama_req.messages.last().unwrap().content, &req.sender).await {
            Ok(response) => response,
            Err(remote_error) => {
                return Ok(HttpResponse::ServiceUnavailable()
                    .json(serde_json::json!({
                        "error": "No available LLM service",
                        "details": format!("No local LLM available. Remote error: {}", remote_error)
                    })));
            }
        }
    };

    // Create response message with host info
    let response_message = ChatMessage {
        content: response.clone(),
        timestamp: Utc::now(),
        sender: "LLM".to_string(),
        message_type: MessageType::Response,
        host_info,
    };

    // Save the response
    CONVERSATION_STORE.add_message("local".to_string(), response_message.clone()).await;

    Ok(HttpResponse::Ok().json(response_message))
}