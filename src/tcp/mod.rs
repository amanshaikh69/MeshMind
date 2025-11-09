pub async fn set_p2p_secret(secret: String) {
    let mut s = P2P_SECRET.lock().await;
    *s = Some(secret);
}

pub async fn add_announced_file(info: FileInfo) {
    let mut v = ANNOUNCED_FILES.lock().await;
    // de-duplicate by filename + uploader_ip
    if !v.iter().any(|f| f.filename == info.filename && f.uploader_ip == info.uploader_ip) {
        v.push(info);
    }
}

pub async fn get_announced_files() -> Vec<FileInfo> {
    ANNOUNCED_FILES.lock().await.clone()
}

fn sign_file_meta(secret: &str, filename: &str, file_type: &str, file_size: u64, sha256_hex: &str, uploaded_at: &str) -> String {
    let payload = format!("{}|{}|{}|{}|{}", filename, file_type, file_size, sha256_hex, uploaded_at);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
    mac.update(payload.as_bytes());
    let res = mac.finalize().into_bytes();
    hex::encode(res)
}

fn verify_file_meta(secret: &str, filename: &str, file_type: &str, file_size: u64, sha256_hex: &str, uploaded_at: &str, hmac_hex: &str) -> bool {
    let expected = sign_file_meta(secret, filename, file_type, file_size, sha256_hex, uploaded_at);
    expected.eq_ignore_ascii_case(hmac_hex)
}

use tokio::net::{TcpStream, TcpListener};
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::sync::Mutex;
use tokio::time::sleep;
use std::sync::Arc;
use std::time::Duration;
use std::path::Path;
use std::collections::{HashSet, HashMap};
use tokio::fs;
use crate::conversation::{Conversation, CONVERSATION_STORE};
use crate::persistence::FileInfo;
use hmac::{Hmac, Mac};
use sha2::Sha256;
type HmacSha256 = Hmac<Sha256>;

use lazy_static::lazy_static;
use reqwest::Client;

const RECEIVED_DIR: &str = "received";
const PORT: i32 = 7878;
const SYNC_INTERVAL: Duration = Duration::from_secs(30);
const OLLAMA_PORT: i32 = 11434;
const OLLAMA_CHECK_URL: &str = "http://127.0.0.1:11434/api/tags";

#[derive(Debug)]
enum Message {
    ConversationFile {
        name: String,
        content: String,
    },
    FileTransfer {
        filename: String,
        file_type: String,
        file_size: u64,
        content: Vec<u8>,
    },
    FileChunk {
        filename: String,
        chunk_index: u32,
        total_chunks: u32,
        content: Vec<u8>,
    },
    FileMeta {
        filename: String,
        file_type: String,
        file_size: u64,
        sha256_hex: String,
        uploaded_at: String,
        hmac_hex: String,
    },
    SyncRequest,
    SyncResponse(Vec<Conversation>),
    LLMCapability {
        has_llm: bool,
    },
    LLMAccessRequest {
        peer_name: String,
        reason: String,
    },
    LLMAccessResponse {
        granted: bool,
        message: String,
        llm_host: Option<String>,
        llm_port: Option<i32>,
    },
}

// Store LLM-capable peers, authorized peers, and LLM connection details
lazy_static! {
    static ref LLM_PEERS: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    static ref AUTHORIZED_PEERS: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    pub static ref LLM_CONNECTIONS: Arc<Mutex<HashMap<String, (String, i32)>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref CONNECTED_PEERS: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    static ref ACTIVE_STREAMS: Arc<Mutex<HashMap<String, TcpStream>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref P2P_SECRET: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    static ref ANNOUNCED_FILES: Arc<Mutex<Vec<FileInfo>>> = Arc::new(Mutex::new(Vec::new()));
}

pub async fn broadcast_file_to_peers(filename: String, file_type: String, content: Vec<u8>) {
    // Send to all active streams regardless of who initiated the TCP connection
    let mut streams = ACTIVE_STREAMS.lock().await;
    let targets: Vec<String> = streams.keys().cloned().collect();
    // Pre-compute meta
    let file_size = content.len() as u64;
    let sha = {
        let mut hasher = Sha256::new();
        use sha2::Digest;
        hasher.update(&content);
        hex::encode(hasher.finalize())
    };
    let uploaded_at = chrono::Utc::now().to_rfc3339();
    let secret_opt = P2P_SECRET.lock().await.clone();
    let hmac_hex = secret_opt
        .as_ref()
        .map(|s| sign_file_meta(s, &filename, &file_type, file_size, &sha, &uploaded_at))
        .unwrap_or_else(|| "".to_string());

    for peer_ip in targets.iter() {
        if let Some(stream) = streams.get_mut(peer_ip) {
            // Send FILE_META first (best-effort)
            let meta = Message::FileMeta {
                filename: filename.clone(),
                file_type: file_type.clone(),
                file_size,
                sha256_hex: sha.clone(),
                uploaded_at: uploaded_at.clone(),
                hmac_hex: hmac_hex.clone(),
            };
            if let Err(e) = meta.send(stream).await {
                eprintln!("TCP: Failed to send FILE_META to {}: {}", peer_ip, e);
            }
            let msg = Message::FileTransfer {
                filename: filename.clone(),
                file_type: file_type.clone(),
                file_size,
                content: content.clone(),
            };
            match msg.send(stream).await {
                Ok(_) => println!("TCP: Broadcasted file {} to peer {}", filename, peer_ip),
                Err(e) => eprintln!("TCP: Failed to broadcast file {} to peer {}: {}", filename, peer_ip, e),
            }
        }
    }
}

impl Message {
    async fn send(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        match self {
            Message::ConversationFile { name, content } => {
                println!("TCP: Sending file {} with size {} bytes", name, content.len());

                // Send marker
                stream.write_all(b"FILE:").await?;

                // Calculate and send total length
                let full_content = format!("{}|{}", name, content);
                let len = full_content.len() as u64;
                stream.write_all(&len.to_le_bytes()).await?;

                // Send data in chunks
                let data = full_content.as_bytes();
                const CHUNK_SIZE: usize = 8192;

                for chunk in data.chunks(CHUNK_SIZE) {
                    match tokio::time::timeout(Duration::from_secs(30), stream.write_all(chunk)).await {
                        Ok(Ok(_)) => {
                            stream.flush().await?;
                        },
                        Ok(Err(e)) => {
                            eprintln!("TCP: Error sending chunk: {}", e);
                            return Err(e);
                        },
                        Err(_) => {
                            let err = std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout sending chunk");
                            eprintln!("TCP: {}", err);
                            return Err(err);
                        }
                    }
                }

                println!("TCP: Successfully sent file {}", name);
                return Ok(());
            },
            Message::SyncRequest => {
                stream.write_all(b"SYNC:").await?;
                let len = 0u64;
                stream.write_all(&len.to_le_bytes()).await?;
                return Ok(());
            },
            Message::SyncResponse(conversations) => {
                stream.write_all(b"RESP:").await?;
                let data = serde_json::to_string(conversations)?;
                let len = data.len() as u64;
                stream.write_all(&len.to_le_bytes()).await?;
                stream.write_all(data.as_bytes()).await?;
                return Ok(());
            },
            Message::LLMCapability { has_llm } => {
                stream.write_all(b"LLMC:").await?;
                let data = has_llm.to_string();
                let len = data.len() as u64;
                stream.write_all(&len.to_le_bytes()).await?;
                stream.write_all(data.as_bytes()).await?;
                return Ok(());
            },
            Message::LLMAccessRequest { peer_name, reason } => {
                stream.write_all(b"LREQ:").await?;
                let data = format!("{}|{}", peer_name, reason);
                let len = data.len() as u64;
                stream.write_all(&len.to_le_bytes()).await?;
                stream.write_all(data.as_bytes()).await?;
                return Ok(());
            },
            Message::LLMAccessResponse { granted, message, llm_host, llm_port } => {
                stream.write_all(b"LRES:").await?;
                let host_str = llm_host.as_deref().unwrap_or("");
                let port_str = llm_port.map(|p| p.to_string()).unwrap_or_default();
                let data = format!("{}|{}|{}|{}", granted, message, host_str, port_str);
                let len = data.len() as u64;
                stream.write_all(&len.to_le_bytes()).await?;
                stream.write_all(data.as_bytes()).await?;
                return Ok(());
            },
            Message::FileTransfer { filename, file_type, file_size, content } => {
                // Use a 5-byte marker to match other message markers (e.g. "FILE:")
                stream.write_all(b"FTRS:").await?;

                // Calculate and send total length
                let header = format!("{}|{}|{}", filename, file_type, file_size);
                let header_len = header.len() as u64;
                let total_len = header_len + content.len() as u64;
                stream.write_all(&total_len.to_le_bytes()).await?;

                // Send header and data
                stream.write_all(header.as_bytes()).await?;
                stream.write_all(&content).await?;
                return Ok(());
            },
            Message::FileChunk { filename, chunk_index, total_chunks, content } => {
                stream.write_all(b"CHNK:").await?;
                let header = format!("{}|{}|{}", filename, chunk_index, total_chunks);
                let header_len = header.len() as u64;
                let total_len = header_len + content.len() as u64;
                stream.write_all(&total_len.to_le_bytes()).await?;
                stream.write_all(header.as_bytes()).await?;
                stream.write_all(&content).await?;
                return Ok(());
            },
            Message::FileMeta { filename, file_type, file_size, sha256_hex, uploaded_at, hmac_hex } => {
                stream.write_all(b"FMTA:").await?;
                let data = format!("{}|{}|{}|{}|{}", filename, file_type, file_size, sha256_hex, uploaded_at);
                let payload = format!("{}|{}", data, hmac_hex);
                let len = payload.len() as u64;
                stream.write_all(&len.to_le_bytes()).await?;
                stream.write_all(payload.as_bytes()).await?;
                return Ok(());
            }
        }
    }

    async fn receive(stream: &mut TcpStream) -> std::io::Result<Option<Message>> {
        let mut marker = [0u8; 5];

        // Read marker with timeout
        match tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut marker)).await {
            Ok(Ok(_)) => (),
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout reading marker")),
        }

        // Read length with timeout
        let mut len_bytes = [0u8; 8];
        match tokio::time::timeout(Duration::from_secs(5), stream.read_exact(&mut len_bytes)).await {
            Ok(Ok(_)) => (),
            Ok(Err(e)) => {
                eprintln!("TCP: Failed to read message length: {}", e);
                return Err(e);
            }
            Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout reading length")),
        }

        let len = u64::from_le_bytes(len_bytes) as usize;
        if len > 1024 * 1024 * 50 { // 50MB limit
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Message too large: {} bytes", len)
            ));
        }

        // Read data in chunks with timeout
        let mut data = Vec::with_capacity(len);
        let mut remaining = len;
        const CHUNK_SIZE: usize = 8192;

        while remaining > 0 {
            let chunk_size = remaining.min(CHUNK_SIZE);
            let mut chunk = vec![0u8; chunk_size];

            match tokio::time::timeout(Duration::from_secs(30), stream.read_exact(&mut chunk)).await {
                Ok(Ok(_)) => {
                    data.extend_from_slice(&chunk);
                    remaining -= chunk_size;
                }
                Ok(Err(e)) => {
                    eprintln!("TCP: Failed to read chunk: {}", e);
                    return Err(e);
                }
                Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Timeout reading chunk")),
            }
        }

        match &marker {
            b"FILE:" => {
                let content = String::from_utf8_lossy(&data);
                if let Some((name, content)) = content.split_once('|') {
                    println!("TCP: Received file {} with size {} bytes", name, content.len());
                    Ok(Some(Message::ConversationFile {
                        name: name.to_string(),
                        content: content.to_string(),
                    }))
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid file format"))
                }
            },
            b"SYNC:" => Ok(Some(Message::SyncRequest)),
            b"RESP:" => {
                let conversations = serde_json::from_slice(&data)?;
                Ok(Some(Message::SyncResponse(conversations)))
            },
            b"LLMC:" => {
                let has_llm = String::from_utf8_lossy(&data).parse::<bool>().unwrap_or(false);
                Ok(Some(Message::LLMCapability { has_llm }))
            },
            b"LREQ:" => {
                let content = String::from_utf8_lossy(&data);
                if let Some((peer_name, reason)) = content.split_once('|') {
                    Ok(Some(Message::LLMAccessRequest {
                        peer_name: peer_name.to_string(),
                        reason: reason.to_string(),
                    }))
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid LLM request format"))
                }
            },
            b"LRES:" => {
                let content = String::from_utf8_lossy(&data);
                let parts: Vec<&str> = content.split('|').collect();
                if parts.len() == 4 {
                    let granted = parts[0].parse().unwrap_or(false);
                    let message = parts[1].to_string();
                    let llm_host = if !parts[2].is_empty() { Some(parts[2].to_string()) } else { None };
                    let llm_port = if !parts[3].is_empty() { parts[3].parse().ok() } else { None };
                    Ok(Some(Message::LLMAccessResponse {
                        granted,
                        message,
                        llm_host,
                        llm_port,
                    }))
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid LLM response format"))
                }
            },
            b"FTRS:" => {
                // Parse header: filename|file_type|file_size followed by binary content
                let content_str = String::from_utf8_lossy(&data);
                let parts: Vec<&str> = content_str.split('|').collect();
                if parts.len() >= 3 {
                    let filename = parts[0].to_string();
                    let file_type = parts[1].to_string();
                    let file_size_str = parts[2];
                    if let Ok(file_size) = file_size_str.parse::<u64>() {
                        // Find the end of header: filename|file_type|file_size|
                        let header_end = filename.len() + 1 + file_type.len() + 1 + file_size_str.len() + 1;
                        if data.len() >= header_end {
                            let content = data[header_end..].to_vec();
                            println!("TCP: Received file transfer {} ({} bytes)", filename, content.len());
                            Ok(Some(Message::FileTransfer {
                                filename,
                                file_type,
                                file_size,
                                content,
                            }))
                        } else {
                            Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "File content too short"))
                        }
                    } else {
                        Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid file size"))
                    }
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid file transfer format"))
                }
            },
            b"CHNK:" => {
                let content = String::from_utf8_lossy(&data);
                if let Some(header_end) = content.find('\0') {
                    let header = &content[..header_end];
                    let file_data = &data[header_end + 1..];
                    let parts: Vec<&str> = header.split('|').collect();
                    if parts.len() == 3 {
                        let filename = parts[0].to_string();
                        let chunk_index = parts[1].parse().unwrap_or(0);
                        let total_chunks = parts[2].parse().unwrap_or(1);
                        Ok(Some(Message::FileChunk {
                            filename,
                            chunk_index,
                            total_chunks,
                            content: file_data.to_vec(),
                        }))
                    } else {
                        Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid file chunk format"))
                    }
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid file chunk format"))
                }
            },
            b"FMTA:" => {
                let content = String::from_utf8_lossy(&data);
                // format: filename|file_type|file_size|sha256|uploaded_at|hmac
                let parts: Vec<&str> = content.split('|').collect();
                if parts.len() >= 6 {
                    let filename = parts[0].to_string();
                    let file_type = parts[1].to_string();
                    let file_size: u64 = parts[2].parse().unwrap_or(0);
                    let sha256_hex = parts[3].to_string();
                    let uploaded_at = parts[4].to_string();
                    let hmac_hex = parts[5].to_string();
                    let ok = if let Some(secret) = P2P_SECRET.blocking_lock().clone() { // blocking_lock ok in non-async context
                        verify_file_meta(&secret, &filename, &file_type, file_size, &sha256_hex, &uploaded_at, &hmac_hex)
                    } else { true };
                    if !ok {
                        eprintln!("TCP: Invalid HMAC for FILE_META {} — ignoring", filename);
                        // Still return Some to consume the message but not act on metadata persistently
                    } else {
                        println!("TCP: Received FILE_META {} ({} bytes) sha={}", filename, file_size, sha256_hex);
                    }
                    Ok(Some(Message::FileMeta { filename, file_type, file_size, sha256_hex, uploaded_at, hmac_hex }))
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid FILE_META format"))
                }
            },
            _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Unknown message type")),
        }
    }
}

// Make the function public
pub async fn is_ollama_available() -> bool {
    if let Ok(client) = Client::builder()
        .timeout(Duration::from_secs(2))
        .build() 
    {
        // First check if Ollama is running locally
        let local_available = match client.get(OLLAMA_CHECK_URL).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        };

        if !local_available {
            return false;
        }

        // Then check if it's accessible externally
        let local_addr = match tokio::net::TcpStream::connect(format!("127.0.0.1:{}", OLLAMA_PORT)).await {
            Ok(stream) => stream.local_addr().ok(),
            Err(_) => None,
        };

        if let Some(addr) = local_addr {
            // Try to connect using the external IP
            match tokio::net::TcpStream::connect(format!("{}:{}", addr.ip(), OLLAMA_PORT)).await {
                Ok(_) => {
                    println!("TCP: Ollama is accessible externally");
                    true
                },
                Err(e) => {
                    println!("TCP: Ollama is not accessible externally: {}", e);
                    println!("TCP: Please configure Ollama to listen on 0.0.0.0 by setting OLLAMA_HOST=0.0.0.0 in the environment");
                    false
                }
            }
        } else {
            false
        }
    } else {
        false
    }
}

pub async fn listen_for_connections() -> std::io::Result<()> {
    // Create received directory if it doesn't exist
    let received_path = Path::new(RECEIVED_DIR);
    if !received_path.exists() {
        fs::create_dir_all(received_path).await?;
    }

    let listener = TcpListener::bind(format!("0.0.0.0:{}", PORT)).await?;
    println!("TCP: Listening on port {}", PORT);

    loop {
        let (stream, addr) = listener.accept().await?;
        println!("TCP: New connection from {}", addr);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                eprintln!("TCP: Connection error with {}: {}", addr, e);
            }
        });
    }
}

// Add this new function for periodic conversation sharing
async fn periodic_conversation_share(mut stream: TcpStream, addr: std::net::SocketAddr) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    
    loop {
        interval.tick().await;
        
        // Check if we still have a valid connection
        if let Err(_) = stream.write_all(&[0u8]).await {
            println!("TCP: Lost connection to {} during periodic share", addr);
            break;
        }
        
        // Share our local conversation
        if let Some(conversation) = CONVERSATION_STORE.get_local_conversation().await {
            match serde_json::to_string(&conversation) {
                Ok(content) => {
                    let message = Message::ConversationFile {
                        name: "local.json".to_string(),
                        content,
                    };
                    
                    match message.send(&mut stream).await {
                        Ok(_) => println!("TCP: Periodic share - Sent local conversation to {}", addr),
                        Err(e) => {
                            eprintln!("TCP: Periodic share - Failed to send local conversation to {}: {}", addr, e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("TCP: Periodic share - Failed to serialize conversation: {}", e);
                    continue;
                }
            }
        }

        // Request sync from peer to ensure we have their latest conversation
        let sync_request = Message::SyncRequest;
        if let Err(e) = sync_request.send(&mut stream).await {
            eprintln!("TCP: Periodic share - Failed to send sync request to {}: {}", addr, e);
            break;
        }
    }
}

async fn handle_connection(mut stream: TcpStream) -> std::io::Result<()> {
    let addr = stream.peer_addr()?;
    println!("TCP: Connected to {}", addr);

    // Create received directory if it doesn't exist
    let received_path = Path::new(RECEIVED_DIR);
    if !received_path.exists() {
        fs::create_dir_all(received_path).await?;
    }

    // Create a directory for this peer's conversations
    let peer_dir = received_path.join(addr.ip().to_string());
    if !peer_dir.exists() {
        fs::create_dir_all(&peer_dir).await?;
    }

    // Get our local IP address for LLM access
    let local_addr = stream.local_addr()?;
    let local_ip = local_addr.ip().to_string();

    // Check Ollama availability before sending capability
    let has_llm = is_ollama_available().await;
    
    // Send our LLM capability immediately
    if let Err(e) = (Message::LLMCapability { has_llm }).send(&mut stream).await {
        return Err(e);
    }

    if has_llm {
        println!("TCP: Announced LLM capability to {}", addr);
    } else {
        println!("TCP: Announced no LLM capability to {} (Ollama not available)", addr);
    }

    // Share our local conversation immediately
    if let Some(conversation) = CONVERSATION_STORE.get_local_conversation().await {
        let content = serde_json::to_string(&conversation)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        
        let message = Message::ConversationFile {
            name: "local.json".to_string(),
            content: content.clone(),
        };
        
        if let Err(e) = message.send(&mut stream).await {
            eprintln!("TCP: Failed to send local conversation to {}: {}", addr, e);
        } else {
            println!("TCP: Sent local conversation to {}", addr);
            
            // Also save the conversation to the peer's directory
            if let Err(e) = fs::write(peer_dir.join("local.json"), content).await {
                eprintln!("TCP: Failed to save conversation for {}: {}", addr, e);
            }
        }
    }

    // Before entering the main loop, clone the socket so we have a dedicated writable stream
    // to use for broadcasts. Store it in ACTIVE_STREAMS keyed by peer IP.
    let std_socket = match stream.into_std() {
        Ok(socket) => socket,
        Err(e) => {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to extract std socket: {}", e)));
        }
    };

    // Clone for handler and broadcaster
    let std_socket_for_handler = match std_socket.try_clone() {
        Ok(s) => s,
        Err(e) => {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to clone std socket for handler: {}", e)));
        }
    };
    let std_socket_for_broadcast = match std_socket.try_clone() {
        Ok(s) => s,
        Err(e) => {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to clone std socket for broadcast: {}", e)));
        }
    };

    let mut stream = match TcpStream::from_std(std_socket_for_handler) {
        Ok(s) => s,
        Err(e) => {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create tokio stream from handler socket: {}", e)));
        }
    };

    let peer_ip_key = addr.ip().to_string();
    match TcpStream::from_std(std_socket_for_broadcast) {
        Ok(bstream) => {
            let mut map = ACTIVE_STREAMS.lock().await;
            map.insert(peer_ip_key.clone(), bstream);
        }
        Err(e) => {
            eprintln!("TCP: Failed to create broadcast stream for {}: {}", addr, e);
        }
    }

    // Main message handling loop for accepted connections
    loop {
        match Message::receive(&mut stream).await {
            Ok(Some(message)) => {
                match message {
                    Message::ConversationFile { name, content } => {
                        let file_path = peer_dir.join(&name);
                        if let Err(e) = fs::write(&file_path, content.as_bytes()).await {
                            eprintln!("TCP: Failed to save received file {}: {}", name, e);
                        } else {
                            println!("TCP: Received and saved conversation file {} from {}", name, addr);
                            if let Ok(conversation) = serde_json::from_str::<Conversation>(&content) {
                                CONVERSATION_STORE.add_peer_conversation(addr.ip().to_string(), conversation).await;
                            }
                        }
                    }
                    Message::LLMCapability { has_llm } => {
                        let mut llm_peers = LLM_PEERS.lock().await;
                        if has_llm {
                            llm_peers.insert(addr.ip().to_string());
                            println!("TCP: Peer {} has LLM capability", addr);
                        } else {
                            llm_peers.remove(&addr.ip().to_string());
                            println!("TCP: Peer {} does not have LLM capability", addr);
                        }
                    }
                    Message::LLMAccessRequest { peer_name, reason } => {
                        println!("TCP: Received LLM access request from {} ({}): {}", addr, peer_name, reason);
                        let has_llm = is_ollama_available().await;
                        if has_llm {
                            // Use the local bind IP of this TCP socket so the peer can reach us
                            let lan_ip = local_ip.clone();
                            let resp = Message::LLMAccessResponse {
                                granted: true,
                                message: "Access granted".to_string(),
                                llm_host: Some(lan_ip),
                                llm_port: Some(8080),
                            };
                            if let Err(e) = resp.send(&mut stream).await {
                                eprintln!("TCP: Failed to send LLM access response to {}: {}", addr, e);
                            }
                        } else {
                            let resp = Message::LLMAccessResponse {
                                granted: false,
                                message: "LLM not available".to_string(),
                                llm_host: None,
                                llm_port: None,
                            };
                            if let Err(e) = resp.send(&mut stream).await {
                                eprintln!("TCP: Failed to send LLM access denial to {}: {}", addr, e);
                            }
                        }
                    }
                    Message::FileMeta { filename, file_type, file_size, sha256_hex: _, uploaded_at, hmac_hex: _ } => {
                        // Store announced peer file so UI can show immediately
                        let ts = match chrono::DateTime::parse_from_rfc3339(&uploaded_at) {
                            Ok(dt) => dt.with_timezone(&chrono::Utc),
                            Err(_) => chrono::Utc::now(),
                        };
                        let info = FileInfo {
                            filename: filename.clone(),
                            file_type: file_type.clone(),
                            file_size: file_size,
                            uploader_ip: addr.ip().to_string(),
                            upload_time: ts,
                        };
                        add_announced_file(info).await;
                    }
                    Message::FileTransfer { filename, file_type, file_size: _, content } => {
                        // Save received binary content to peer dir
                        let out_path = peer_dir.join(&filename);
                        if let Err(e) = fs::write(&out_path, &content).await {
                            eprintln!("TCP: Failed to save received binary {} from {}: {}", filename, addr, e);
                        } else {
                            println!("TCP: Saved received binary {} from {}", filename, addr);
                            // Ensure it appears in /api/files immediately even if FILE_META was missed
                            let info = FileInfo {
                                filename: filename.clone(),
                                file_type: file_type.clone(),
                                file_size: content.len() as u64,
                                uploader_ip: addr.ip().to_string(),
                                upload_time: chrono::Utc::now(),
                            };
                            add_announced_file(info).await;
                        }
                    }
                    _ => {}
                }
            }
            Ok(None) => {
                println!("TCP: Connection closed by {}", addr);
                let mut map = ACTIVE_STREAMS.lock().await;
                map.remove(&addr.ip().to_string());
                break;
            }
            Err(e) => {
                eprintln!("TCP: Error reading from {}: {}", addr, e);
                let mut map = ACTIVE_STREAMS.lock().await;
                map.remove(&addr.ip().to_string());
                break;
            }
        }
    }

    Ok(())
}

pub async fn connect_to_peers(received_ips: Arc<Mutex<HashSet<String>>>) {
    loop {
        let mut ips = received_ips.lock().await;
        for ip in ips.drain() {
            // Skip if we're already connected to this peer
            let mut connected = CONNECTED_PEERS.lock().await;
            if connected.contains(&ip) {
                println!("TCP: Already connected to {}, skipping", ip);
                continue;
            }
            connected.insert(ip.clone());
            drop(connected);
            
            let addr = format!("{}:{}", ip, PORT);
            match TcpStream::connect(&addr).await {
                Ok(mut stream) => {
                    println!("TCP: Connected to {}", addr);
                    
                    // Create received directory if it doesn't exist
                    let received_path = Path::new(RECEIVED_DIR);
                    if !received_path.exists() {
                        if let Err(e) = fs::create_dir_all(received_path).await {
                            eprintln!("TCP: Failed to create received directory: {}", e);
                            let mut connected = CONNECTED_PEERS.lock().await;
                            connected.remove(&ip);
                            continue;
                        }
                    }

                    // Create a directory for this peer's conversations
                    let peer_dir = received_path.join(&ip);
                    if !peer_dir.exists() {
                        if let Err(e) = fs::create_dir_all(&peer_dir).await {
                            eprintln!("TCP: Failed to create peer directory: {}", e);
                            let mut connected = CONNECTED_PEERS.lock().await;
                            connected.remove(&ip);
                            continue;
                        }
                    }
                    
                    // Check Ollama availability before sending capability
                    let has_llm = is_ollama_available().await;
                    
                    // Send our LLM capability
                    if let Err(e) = (Message::LLMCapability { has_llm }).send(&mut stream).await {
                        eprintln!("TCP: Failed to send LLM capability to {}: {}", addr, e);
                        let mut connected = CONNECTED_PEERS.lock().await;
                        connected.remove(&ip);
                        continue;
                    }

                    if has_llm {
                        println!("TCP: Announced LLM capability to {}", addr);
                    } else {
                        println!("TCP: Announced no LLM capability to {} (Ollama not available)", addr);
                    }

                    // Share our local conversation
                    if let Some(conversation) = CONVERSATION_STORE.get_local_conversation().await {
                        let content = match serde_json::to_string(&conversation) {
                            Ok(content) => content,
                            Err(e) => {
                                eprintln!("TCP: Failed to serialize conversation: {}", e);
                                let mut connected = CONNECTED_PEERS.lock().await;
                                connected.remove(&ip);
                                continue;
                            }
                        };
                        
                        let message = Message::ConversationFile {
                            name: "local.json".to_string(),
                            content,
                        };
                        
                        if let Err(e) = message.send(&mut stream).await {
                            eprintln!("TCP: Failed to send local conversation to {}: {}", addr, e);
                            let mut connected = CONNECTED_PEERS.lock().await;
                            connected.remove(&ip);
                            continue;
                        } else {
                            println!("TCP: Sent local conversation to {}", addr);
                        }
                    }

                    // Register a dedicated writable stream for broadcasts by cloning the std socket
                    let std_socket = match stream.into_std() {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("TCP: Failed to get std socket for {}: {}", addr, e);
                            let mut connected = CONNECTED_PEERS.lock().await;
                            connected.remove(&ip);
                            continue;
                        }
                    };

                    // One clone for periodic sharing, one for main handler, one for broadcasting
                    let share_socket = match std_socket.try_clone() {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("TCP: Failed to clone share socket for {}: {}", addr, e);
                            let mut connected = CONNECTED_PEERS.lock().await;
                            connected.remove(&ip);
                            continue;
                        }
                    };
                    let handler_socket = match std_socket.try_clone() {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("TCP: Failed to clone handler socket for {}: {}", addr, e);
                            let mut connected = CONNECTED_PEERS.lock().await;
                            connected.remove(&ip);
                            continue;
                        }
                    };
                    let broadcast_socket = match std_socket.try_clone() {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("TCP: Failed to clone broadcast socket for {}: {}", addr, e);
                            let mut connected = CONNECTED_PEERS.lock().await;
                            connected.remove(&ip);
                            continue;
                        }
                    };

                    let mut stream = match TcpStream::from_std(handler_socket) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("TCP: Failed to make tokio handler stream for {}: {}", addr, e);
                            let mut connected = CONNECTED_PEERS.lock().await;
                            connected.remove(&ip);
                            continue;
                        }
                    };
                    let share_stream = match TcpStream::from_std(share_socket) {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("TCP: Failed to make tokio share stream for {}: {}", addr, e);
                            let mut connected = CONNECTED_PEERS.lock().await;
                            connected.remove(&ip);
                            continue;
                        }
                    };
                    match TcpStream::from_std(broadcast_socket) {
                        Ok(bstream) => {
                            let mut map = ACTIVE_STREAMS.lock().await;
                            map.insert(ip.clone(), bstream);
                        }
                        Err(e) => eprintln!("TCP: Failed to make tokio broadcast stream for {}: {}", addr, e),
                    }

                    // Set up periodic sharing
                    match setup_periodic_sharing(share_stream, &addr, &ip).await {
                        Ok((mut _unused, share_handle)) => {
                            // Keep connection alive and handle messages
                            loop {
                                match Message::receive(&mut stream).await {
                                    Ok(Some(message)) => {
                                        match message {
                                            Message::ConversationFile { name, content } => {
                                                // Save the conversation in the peer's directory
                                                let file_path = peer_dir.join(&name);
                                                if let Err(e) = fs::write(&file_path, content.as_bytes()).await {
                                                    eprintln!("TCP: Failed to save received file {}: {}", name, e);
                                                } else {
                                                    println!("TCP: Received and saved conversation file {} from {}", name, addr);
                                                    
                                                    // Try to load the received conversation
                                                    if let Ok(conversation) = serde_json::from_str::<Conversation>(&content) {
                                                        CONVERSATION_STORE.add_peer_conversation(ip.clone(), conversation).await;
                                                    }
                                                }
                                            }
                                            Message::LLMCapability { has_llm } => {
                                                let mut llm_peers = LLM_PEERS.lock().await;
                                                if has_llm {
                                                    llm_peers.insert(ip.clone());
                                                    println!("TCP: Peer {} has LLM capability", addr);
                                                    
                                                    // Check if we need to request access
                                                    let authorized = AUTHORIZED_PEERS.lock().await;
                                                    if !authorized.contains(&ip) {
                                                        drop(authorized);
                                                        drop(llm_peers);
                                                        if let Err(e) = request_llm_access(&mut stream, &addr).await {
                                                            eprintln!("TCP: Failed to request LLM access: {}", e);
                                                            break;
                                                        }
                                                    }
                                                } else {
                                                    llm_peers.remove(&ip);
                                                    println!("TCP: Peer {} does not have LLM capability", addr);
                                                }
                                            }
                                            Message::LLMAccessResponse { granted, message, llm_host, llm_port } => {
                                                if granted {
                                                    let mut authorized = AUTHORIZED_PEERS.lock().await;
                                                    authorized.insert(ip.clone());
                                                    
                                                    // Store LLM connection details if provided
                                                    if let (Some(host), Some(port)) = (llm_host.clone(), llm_port) {
                                                        let mut connections = LLM_CONNECTIONS.lock().await;
                                                        connections.insert(ip.clone(), (host.clone(), port));
                                                        println!("TCP: LLM access granted by {} - {} (LLM available at {}:{})", 
                                                               addr, message, host, port);
                                                    } else {
                                                        println!("TCP: LLM access granted by {} - {}", addr, message);
                                                    }
                                                } else {
                                                    println!("TCP: LLM access denied by {} - {}", addr, message);
                                                }
                                            }
                                            Message::FileMeta { filename, file_type, file_size, sha256_hex: _, uploaded_at, hmac_hex: _ } => {
                                                // Record announced peer file to show in UI immediately
                                                let ts = match chrono::DateTime::parse_from_rfc3339(&uploaded_at) {
                                                    Ok(dt) => dt.with_timezone(&chrono::Utc),
                                                    Err(_) => chrono::Utc::now(),
                                                };
                                                let info = FileInfo {
                                                    filename: filename.clone(),
                                                    file_type: file_type.clone(),
                                                    file_size: file_size,
                                                    uploader_ip: ip.clone(),
                                                    upload_time: ts,
                                                };
                                                add_announced_file(info).await;
                                            }
                                            Message::FileTransfer { filename, file_type: _, file_size: _, content } => {
                                                // Save received binary into peer_dir
                                                let out_path = peer_dir.join(&filename);
                                                if let Err(e) = fs::write(&out_path, &content).await {
                                                    eprintln!("TCP: Failed to save received binary {} from {}: {}", filename, addr, e);
                                                } else {
                                                    println!("TCP: Saved received binary {} from {}", filename, addr);
                                                }
                                            }
                                            _ => continue,
                                        }
                                    }
                                    Ok(None) => {
                                        println!("TCP: Connection closed by {}", addr);
                                        let mut connected = CONNECTED_PEERS.lock().await;
                                        connected.remove(&ip);
                                        let mut map = ACTIVE_STREAMS.lock().await;
                                        map.remove(&ip);
                                        break;
                                    }
                                    Err(e) => {
                                        eprintln!("TCP: Error reading from {}: {}", addr, e);
                                        let mut connected = CONNECTED_PEERS.lock().await;
                                        connected.remove(&ip);
                                        let mut map = ACTIVE_STREAMS.lock().await;
                                        map.remove(&ip);
                                        break;
                                    }
                                }
                            }

                            // Cancel the periodic sharing task when the connection ends
                            share_handle.abort();
                        }
                        Err(e) => {
                            eprintln!("TCP: Failed to setup periodic sharing for {}: {}", addr, e);
                            let mut connected = CONNECTED_PEERS.lock().await;
                            connected.remove(&ip);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("TCP: Failed to connect to {}: {}", addr, e);
                    let mut connected = CONNECTED_PEERS.lock().await;
                    connected.remove(&ip);
                }
            }
        }
        drop(ips);
        sleep(SYNC_INTERVAL).await;
    }
}

// Helper function to set up periodic sharing
async fn setup_periodic_sharing(
    stream: TcpStream,
    addr: &str,
    ip: &str,
) -> std::io::Result<(TcpStream, tokio::task::JoinHandle<()>)> {
    let socket = match stream.into_std() {
        Ok(socket) => socket,
        Err(e) => {
            let mut connected = CONNECTED_PEERS.lock().await;
            connected.remove(ip);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to get standard socket: {}", e)));
        }
    };

    if let Err(e) = socket.set_nonblocking(true) {
        let mut connected = CONNECTED_PEERS.lock().await;
        connected.remove(ip);
        return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to set nonblocking: {}", e)));
    }

    let share_socket = match socket.try_clone() {
        Ok(socket) => socket,
        Err(e) => {
            let mut connected = CONNECTED_PEERS.lock().await;
            connected.remove(ip);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to clone socket: {}", e)));
        }
    };

    let stream = match TcpStream::from_std(socket) {
        Ok(stream) => stream,
        Err(e) => {
            let mut connected = CONNECTED_PEERS.lock().await;
            connected.remove(ip);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create tokio stream: {}", e)));
        }
    };

    let share_stream = match TcpStream::from_std(share_socket) {
        Ok(stream) => stream,
        Err(e) => {
            let mut connected = CONNECTED_PEERS.lock().await;
            connected.remove(ip);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to create share stream: {}", e)));
        }
    };

    // Parse the address for periodic sharing
    let socket_addr = match addr.parse() {
        Ok(addr) => addr,
        Err(e) => {
            let mut connected = CONNECTED_PEERS.lock().await;
            connected.remove(ip);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Failed to parse address: {}", e)));
        }
    };

    // Spawn periodic conversation sharing task
    let share_handle = tokio::spawn(periodic_conversation_share(share_stream, socket_addr));

    Ok((stream, share_handle))
}

async fn request_llm_access(stream: &mut TcpStream, addr: &str) -> std::io::Result<()> {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let request = Message::LLMAccessRequest {
        peer_name: hostname,
        reason: "Requesting access to LLM services".to_string(),
    };

    println!("TCP: Sending LLM access request to {}", addr);
    
    // Send request with timeout
    match tokio::time::timeout(Duration::from_secs(5), request.send(stream)).await {
        Ok(Ok(_)) => println!("TCP: Successfully sent LLM access request to {}", addr),
        Ok(Err(e)) => {
            eprintln!("TCP: Failed to send LLM access request to {}: {}", addr, e);
            return Err(e);
        }
        Err(_) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Timeout while sending LLM access request"
            ));
        }
    }

    // Do not wait here; the main receive loop will capture LLMAccessResponse and store it.
    Ok(())
}
 