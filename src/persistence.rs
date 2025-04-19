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
    
    println!("Loading peer conversations from: {}", received_path.display());
    
    if !received_path.exists() {
        println!("Creating received directory as it does not exist");
        fs::create_dir_all(received_path).await?;
        return Ok(peer_conversations);
    }
    
    // Read all directories in the received folder
    let mut entries = fs::read_dir(received_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        let peer_ip = entry.file_name().to_string_lossy().to_string();
        
        println!("Found entry: {} (is_dir: {})", peer_ip, file_type.is_dir());
        
        if file_type.is_dir() {
            let local_json_path = entry.path().join("local.json");
            println!("Checking for local.json at: {}", local_json_path.display());
            
            if local_json_path.exists() {
                println!("Found local.json for peer: {}", peer_ip);
                match fs::read_to_string(&local_json_path).await {
                    Ok(content) => {
                        match serde_json::from_str::<Conversation>(&content) {
                            Ok(conversation) => {
                                println!("Successfully loaded conversation for peer: {}", peer_ip);
                                println!("Conversation contains {} messages", conversation.messages.len());
                                peer_conversations.insert(peer_ip, conversation);
                            }
                            Err(e) => {
                                eprintln!("Failed to parse conversation for peer {}: {}", peer_ip, e);
                                eprintln!("Content: {}", content);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read local.json for peer {}: {}", peer_ip, e);
                    }
                }
            } else {
                println!("No local.json found at: {}", local_json_path.display());
            }
        }
    }
    
    println!("Loaded {} peer conversations", peer_conversations.len());
    for (peer, conv) in &peer_conversations {
        println!("Peer {} has {} messages", peer, conv.messages.len());
    }
    
    Ok(peer_conversations)
} 