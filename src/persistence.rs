use std::path::Path;
use tokio::fs;
use serde_json;
use crate::conversation::Conversation;
use std::collections::HashMap;
use chrono;

pub const CONVERSATIONS_DIR: &str = "conversations";
pub const RECEIVED_DIR: &str = "received";
pub const FILES_DIR: &str = "files";
pub const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50MB

pub async fn init_conversations_dir() -> std::io::Result<()> {
    let conversations_path = Path::new(CONVERSATIONS_DIR);
    let received_path = Path::new(RECEIVED_DIR);
    let files_path = Path::new(FILES_DIR);
    
    if !conversations_path.exists() {
        fs::create_dir_all(conversations_path).await?;
    }
    if !received_path.exists() {
        fs::create_dir_all(received_path).await?;
    }
    if !files_path.exists() {
        fs::create_dir_all(files_path).await?;
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileInfo {
    pub filename: String,
    pub file_type: String,
    pub file_size: u64,
    pub uploader_ip: String,
    pub upload_time: chrono::DateTime<chrono::Utc>,
}

pub async fn save_uploaded_file(
    filename: &str,
    file_type: &str,
    content: &[u8],
    uploader_ip: &str,
) -> std::io::Result<FileInfo> {
    // Validate file size
    if content.len() as u64 > MAX_FILE_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("File too large. Maximum size is {} bytes", MAX_FILE_SIZE),
        ));
    }

    // Validate file type
    let allowed_types = [
        "image/jpeg",
        "image/png",
        "image/gif",
        "image/webp",
        "text/plain",
        "text/markdown",
        "application/pdf",
        "application/octet-stream",
        "application/x-msdownload",
        "application/zip",
        "application/x-zip-compressed",
        "application/x-7z-compressed",
        "application/x-rar-compressed",
    ];
    if !allowed_types.contains(&file_type) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "File type not allowed",
        ));
    }

    // Create unique filename to avoid conflicts
    let timestamp = chrono::Utc::now().timestamp();
    let safe_filename = filename.replace(" ", "_").replace("/", "_");
    let unique_filename = format!("{}_{}", timestamp, safe_filename);
    
    let file_path = Path::new(FILES_DIR).join(&unique_filename);
    fs::write(&file_path, content).await?;

    let file_info = FileInfo {
        filename: filename.to_string(),
        file_type: file_type.to_string(),
        file_size: content.len() as u64,
        uploader_ip: uploader_ip.to_string(),
        upload_time: chrono::Utc::now(),
    };

    // Save file metadata
    let metadata_path = Path::new(FILES_DIR).join(format!("{}.meta", unique_filename));
    let metadata_json = serde_json::to_string_pretty(&file_info)?;
    fs::write(metadata_path, metadata_json).await?;

    Ok(file_info)
}

pub async fn get_file_info(filename: &str) -> std::io::Result<Option<FileInfo>> {
    let files_path = Path::new(FILES_DIR);
    let mut entries = fs::read_dir(files_path).await?;
    
    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name.ends_with(".meta") {
            let content = fs::read_to_string(entry.path()).await?;
            if let Ok(file_info) = serde_json::from_str::<FileInfo>(&content) {
                if file_info.filename == filename {
                    return Ok(Some(file_info));
                }
            }
        }
    }
    
    Ok(None)
}

pub async fn get_file_content(filename: &str) -> std::io::Result<Option<Vec<u8>>> {
    let files_path = Path::new(FILES_DIR);
    let mut entries = fs::read_dir(files_path).await?;
    
    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !file_name.ends_with(".meta") {
            // Check if this file matches our filename
            if let Some(file_info) = get_file_info(filename).await? {
                let timestamp = file_info.upload_time.timestamp();
                let safe_filename = filename.replace(" ", "_").replace("/", "_");
                let expected_name = format!("{}_{}", timestamp, safe_filename);
                
                if file_name == expected_name {
                    let content = fs::read(entry.path()).await?;
                    return Ok(Some(content));
                }
            }
        }
    }
    
    Ok(None)
}

pub async fn list_uploaded_files() -> std::io::Result<Vec<FileInfo>> {
    let files_path = Path::new(FILES_DIR);
    let mut files = Vec::new();
    
    if !files_path.exists() {
        return Ok(files);
    }
    
    let mut entries = fs::read_dir(files_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name.ends_with(".meta") {
            let content = fs::read_to_string(entry.path()).await?;
            if let Ok(file_info) = serde_json::from_str::<FileInfo>(&content) {
                files.push(file_info);
            }
        }
    }
    
    // Sort by upload time (newest first)
    files.sort_by(|a, b| b.upload_time.cmp(&a.upload_time));
    Ok(files)
}

// List files that were received from peers over TCP and stored under received/<peer-ip>/
// This allows the API to surface peer files even if FILE_META was missed or the process restarted.
pub async fn list_received_files() -> std::io::Result<Vec<FileInfo>> {
    let mut out: Vec<FileInfo> = Vec::new();
    let base = Path::new(RECEIVED_DIR);
    if !base.exists() {
        return Ok(out);
    }

    let mut peers = fs::read_dir(base).await?;
    while let Some(peer_entry) = peers.next_entry().await? {
        if !peer_entry.file_type().await?.is_dir() { continue; }
        let peer_ip = peer_entry.file_name().to_string_lossy().to_string();

        // Iterate files within this peer directory
        let mut dir = fs::read_dir(peer_entry.path()).await?;
        while let Some(file) = dir.next_entry().await? {
            let name = file.file_name().to_string_lossy().to_string();
            // Skip conversation JSON and obvious metadata files
            if name == "local.json" || name.ends_with(".meta") { continue; }

            // Determine size and modified time
            if let Ok(meta) = fs::metadata(file.path()).await {
                let size = meta.len();
                // Best-effort MIME type from extension
                let mime = mime_guess::from_path(&name).first_or_octet_stream().to_string();

                // Modified time as upload_time fallback
                let upload_time = match meta.modified() {
                    Ok(st) => chrono::DateTime::<chrono::Utc>::from(st),
                    Err(_) => chrono::Utc::now(),
                };

                out.push(FileInfo {
                    filename: name.clone(),
                    file_type: mime,
                    file_size: size as u64,
                    uploader_ip: peer_ip.clone(),
                    upload_time,
                });
            }
        }
    }

    // Newest first for consistency
    out.sort_by(|a, b| b.upload_time.cmp(&a.upload_time));
    Ok(out)
}