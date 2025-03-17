use std::path::Path;
use tokio::fs;
use serde_json;
use crate::conversation::Conversation;
use std::collections::HashMap;

pub const CONVERSATIONS_DIR: &str = "conversations";
pub const RECEIVED_DIR: &str = "received";

pub async fn init_conversations_dir() -> std::io::Result<()> {
    let conversations_path = Path::new(CONVERSATIONS_DIR);
    let received_path = Path::new(RECEIVED_DIR);
    
    if !conversations_path.exists() {
        fs::create_dir_all(conversations_path).await?;
    }
    if !received_path.exists() {
        fs::create_dir_all(received_path).await?;
    }
    Ok(())
}

pub async fn save_local_conversation(conversation: &Conversation) -> std::io::Result<()> {
    let file_path = Path::new(CONVERSATIONS_DIR).join("local.json");
    let json = serde_json::to_string_pretty(conversation)?;
    fs::write(file_path, json).await?;
    Ok(())
}

pub async fn save_peer_conversation(peer_ip: &str, conversation: &Conversation) -> std::io::Result<()> {
    let peer_dir = Path::new(RECEIVED_DIR).join(peer_ip);
    if !peer_dir.exists() {
        fs::create_dir_all(&peer_dir).await?;
    }
    
    let file_path = peer_dir.join("local.json");
    let json = serde_json::to_string_pretty(conversation)?;
    fs::write(file_path, json).await?;
    Ok(())
}

pub async fn load_local_conversation() -> std::io::Result<Option<Conversation>> {
    let file_path = Path::new(CONVERSATIONS_DIR).join("local.json");
    if !file_path.exists() {
        return Ok(None);
    }
    
    let content = fs::read_to_string(file_path).await?;
    let conversation = serde_json::from_str(&content)?;
    Ok(Some(conversation))
}

pub async fn load_all_peer_conversations() -> std::io::Result<HashMap<String, Conversation>> {
    let mut peer_conversations = HashMap::new();
    let received_path = Path::new(RECEIVED_DIR);
    
    if !received_path.exists() {
        return Ok(peer_conversations);
    }
    
    let mut entries = fs::read_dir(received_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_dir() {
            let peer_ip = entry.file_name().to_string_lossy().to_string();
            let conversation_path = entry.path().join("local.json");
            
            if conversation_path.exists() {
                let content = fs::read_to_string(conversation_path).await?;
                if let Ok(conversation) = serde_json::from_str(&content) {
                    peer_conversations.insert(peer_ip, conversation);
                }
            }
        }
    }
    
    Ok(peer_conversations)
} 