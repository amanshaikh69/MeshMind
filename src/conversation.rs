use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::Mutex;
use lazy_static::lazy_static;
use chrono::{DateTime, Utc};
use crate::persistence;
use hostname;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub sender: String,
    pub message_type: MessageType,
    pub host_info: HostInfo,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MessageType {
    Question,
    Response,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HostInfo {
    pub hostname: String,
    pub ip_address: String,
    pub is_llm_host: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Conversation {
    pub id: String,
    pub messages: Vec<ChatMessage>,
    pub host_info: HostInfo,
}

pub struct ConversationStore {
    local_conversation: Mutex<Option<Conversation>>,
    peer_conversations: Mutex<HashMap<String, Conversation>>,
}

impl ConversationStore {
    pub fn new() -> Self {
        ConversationStore {
            local_conversation: Mutex::new(None),
            peer_conversations: Mutex::new(HashMap::new()),
        }
    }

    pub async fn add_message(&self, conversation_id: String, message: ChatMessage) {
        if conversation_id == "local" {
            let mut local = self.local_conversation.lock().await;
            
            if let Some(conversation) = local.as_mut() {
                conversation.messages.push(message.clone());
            } else {
                // Create new local conversation
                let hostname = hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "Unknown".to_string());
                
                let ip_address = std::net::TcpStream::connect("8.8.8.8:53")
                    .and_then(|s| s.local_addr())
                    .map(|addr| addr.ip().to_string())
                    .unwrap_or_else(|_| "Unknown".to_string());
                
                let conversation = Conversation {
                    id: "local".to_string(),
                    messages: vec![message.clone()],
                    host_info: HostInfo {
                        hostname,
                        ip_address,
                        is_llm_host: message.host_info.is_llm_host,
                    },
                };
                *local = Some(conversation.clone());
            }

            // Save local conversation
            if let Some(conversation) = local.as_ref() {
                if let Err(e) = persistence::save_local_conversation(conversation).await {
                    eprintln!("Error saving local conversation: {}", e);
                }
            }
        }
    }

    pub async fn add_peer_conversation(&self, peer_ip: String, conversation: Conversation) {
        let mut peer_conversations = self.peer_conversations.lock().await;
        peer_conversations.insert(peer_ip.clone(), conversation.clone());
        
        // Save to disk
        if let Err(e) = persistence::save_peer_conversation(&peer_ip, &conversation).await {
            eprintln!("Error saving peer conversation: {}", e);
        }
    }

    pub async fn get_local_conversation(&self) -> Option<Conversation> {
        let local = self.local_conversation.lock().await;
        local.clone()
    }

    pub async fn load_saved_conversations(&self) -> std::io::Result<()> {
        // Load local conversation
        if let Ok(Some(local)) = persistence::load_local_conversation().await {
            let mut local_lock = self.local_conversation.lock().await;
            *local_lock = Some(local);
        }

        // Load peer conversations
        let peers = persistence::load_all_peer_conversations().await?;
        let mut peers_lock = self.peer_conversations.lock().await;
        *peers_lock = peers;

        Ok(())
    }

    pub async fn get_peer_conversations(&self) -> HashMap<String, Conversation> {
        let peers = self.peer_conversations.lock().await;
        peers.clone()
    }
}

lazy_static! {
    pub static ref CONVERSATION_STORE: ConversationStore = ConversationStore::new();
} 