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
}

impl Message {
    async fn send(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        match self {
            Message::ConversationFile { name, content } => {
                // Send marker
                stream.write_all(b"FILE:").await?;
                
                // Prepare the data with a clear separator
                let data = format!("{}|{}", name, content);
                let len = data.len() as u64;
                
                // Send length and verify it was written
                let len_bytes = len.to_le_bytes();
                let bytes_written = stream.write(&len_bytes).await?;
                if bytes_written != 8 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        format!("Failed to write complete length (wrote {} of 8 bytes)", bytes_written)
                    ));
                }
                
                // Send data and verify it was written
                let bytes_written = stream.write(data.as_bytes()).await?;
                if bytes_written != data.len() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        format!("Failed to write complete data (wrote {} of {} bytes)", bytes_written, data.len())
                    ));
                }
            }
            Message::SyncRequest => {
                stream.write_all(b"SYNC:").await?;
                let len = 0u64;
                stream.write_all(&len.to_le_bytes()).await?;
            }
            Message::SyncResponse(conversations) => {
                stream.write_all(b"RESP:").await?;
                let data = serde_json::to_string(conversations)?;
                let len = data.len() as u64;
                stream.write_all(&len.to_le_bytes()).await?;
                stream.write_all(data.as_bytes()).await?;
            }
            Message::LLMCapability { has_llm } => {
                stream.write_all(b"LLMC:").await?;
                let data = has_llm.to_string();
                let len = data.len() as u64;
                stream.write_all(&len.to_le_bytes()).await?;
                stream.write_all(data.as_bytes()).await?;
            }
            Message::LLMAccessRequest { peer_name, reason } => {
                stream.write_all(b"LREQ:").await?;
                let data = format!("{}|{}", peer_name, reason);
                let len = data.len() as u64;
                stream.write_all(&len.to_le_bytes()).await?;
                stream.write_all(data.as_bytes()).await?;
            }
            Message::LLMAccessResponse { granted, message, llm_host, llm_port } => {
                stream.write_all(b"LRES:").await?;
                let host_str = llm_host.as_deref().unwrap_or("");
                let port_str = llm_port.map(|p| p.to_string()).unwrap_or_default();
                let data = format!("{}|{}|{}|{}", granted, message, host_str, port_str);
                let len = data.len() as u64;
                stream.write_all(&len.to_le_bytes()).await?;
                stream.write_all(data.as_bytes()).await?;
            }
        }
        stream.flush().await?;
        Ok(())
    }

    async fn receive(stream: &mut TcpStream) -> std::io::Result<Option<Message>> {
        let mut marker = [0u8; 5];
        
        // Read marker with proper error handling
        match stream.read_exact(&mut marker).await {
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }

        // Read length with verification
        let mut len_bytes = [0u8; 8];
        match stream.read_exact(&mut len_bytes).await {
            Ok(_) => (),
            Err(e) => {
                eprintln!("TCP: Failed to read message length: {}", e);
                return Err(e);
            }
        }
        
        let len = u64::from_le_bytes(len_bytes) as usize;
        if len > 1024 * 1024 * 10 { // 10MB limit
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Message too large: {} bytes", len)
            ));
        }

        // Read data with verification
        let mut data = vec![0u8; len];
        match stream.read_exact(&mut data).await {
            Ok(_) => (),
            Err(e) => {
                eprintln!("TCP: Failed to read message data: {}", e);
                return Err(e);
            }
        }

        match &marker {
            b"FILE:" => {
                let content = String::from_utf8_lossy(&data);
                if let Some((name, content)) = content.split_once('|') {
                    Ok(Some(Message::ConversationFile {
                        name: name.to_string(),
                        content: content.to_string(),
                    }))
                } else {
                    Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid file format"))
                }
            }
            b"SYNC:" => Ok(Some(Message::SyncRequest)),
            b"RESP:" => {
                let conversations = serde_json::from_slice(&data)?;
                Ok(Some(Message::SyncResponse(conversations)))
            }
            b"LLMC:" => {
                let has_llm = String::from_utf8_lossy(&data).parse::<bool>().unwrap_or(false);
                Ok(Some(Message::LLMCapability { has_llm }))
            }
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
            }
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
            }
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
            content,
        };
        
        if let Err(e) = message.send(&mut stream).await {
            eprintln!("TCP: Failed to send local conversation to {}: {}", addr, e);
        } else {
            println!("TCP: Sent local conversation to {}", addr);
        }
    }

    // Main message handling loop
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
                                CONVERSATION_STORE.add_peer_conversation(addr.ip().to_string(), conversation).await;
                            }
                        }
                    }
                    Message::LLMAccessRequest { peer_name, reason } => {
                        if has_llm {
                            println!("TCP: Received LLM access request from {} ({}): {}", addr, peer_name, reason);
                            
                            // Include our IP and Ollama port in the response
                            let response = Message::LLMAccessResponse {
                                granted: true,
                                message: "Access granted automatically".to_string(),
                                llm_host: Some(local_ip.clone()),
                                llm_port: Some(OLLAMA_PORT),
                            };
                            
                            // Send response immediately and ensure it's sent
                            if let Err(e) = response.send(&mut stream).await {
                                eprintln!("TCP: Failed to send LLM access response to {}: {}", addr, e);
                                return Err(e);
                            }

                            let mut authorized = AUTHORIZED_PEERS.lock().await;
                            authorized.insert(addr.ip().to_string());
                            println!("TCP: Granted LLM access to {} ({}) with port {}", addr, peer_name, OLLAMA_PORT);
                        } else {
                            let response = Message::LLMAccessResponse {
                                granted: false,
                                message: "This peer does not have LLM capability".to_string(),
                                llm_host: None,
                                llm_port: None,
                            };
                            if let Err(e) = response.send(&mut stream).await {
                                eprintln!("TCP: Failed to send LLM access response to {}: {}", addr, e);
                                return Err(e);
                            }
                        }
                    }
                    Message::LLMCapability { has_llm } => {
                        let ip = addr.ip().to_string();
                        let mut llm_peers = LLM_PEERS.lock().await;
                        if has_llm {
                            llm_peers.insert(ip.clone());
                            println!("TCP: Peer {} has LLM capability", addr);
                            
                            // Check if we need to request access
                            let authorized = AUTHORIZED_PEERS.lock().await;
                            if !authorized.contains(&ip) {
                                drop(authorized);
                                drop(llm_peers);
                                // Send request on a delay to avoid race conditions
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                if let Err(e) = request_llm_access(&mut stream, &addr.to_string()).await {
                                    eprintln!("TCP: Failed to request LLM access: {}", e);
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
                            authorized.insert(addr.ip().to_string());
                            
                            // Store LLM connection details if provided
                            if let (Some(host), Some(port)) = (llm_host.clone(), llm_port) {
                                let mut connections = LLM_CONNECTIONS.lock().await;
                                connections.insert(addr.ip().to_string(), (host.clone(), port));
                                println!("TCP: LLM access granted by {} - {} (LLM available at {}:{})", 
                                       addr, message, host, port);
                            } else {
                                println!("TCP: LLM access granted by {} - {}", addr, message);
                            }
                        } else {
                            println!("TCP: LLM access denied by {} - {}", addr, message);
                        }
                    }
                    _ => {
                        println!("TCP: Received unexpected message type from {}", addr);
                    }
                }
            }
            Ok(None) => {
                println!("TCP: Connection closed by {}", addr);
                break;
            }
            Err(e) => {
                eprintln!("TCP: Error reading from {}: {}", addr, e);
                return Err(e);
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

                    // Set up periodic sharing
                    match setup_periodic_sharing(stream, &addr, &ip).await {
                        Ok((mut stream, share_handle)) => {
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
                                            _ => continue,
                                        }
                                    }
                                    Ok(None) => {
                                        println!("TCP: Connection closed by {}", addr);
                                        let mut connected = CONNECTED_PEERS.lock().await;
                                        connected.remove(&ip);
                                        break;
                                    }
                                    Err(e) => {
                                        eprintln!("TCP: Error reading from {}: {}", addr, e);
                                        let mut connected = CONNECTED_PEERS.lock().await;
                                        connected.remove(&ip);
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

async fn request_llm_access(stream: &mut TcpStream, addr: &str) -> std::io::Result<bool> {
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

    // Wait for response with a single longer timeout
    let timeout = Duration::from_secs(15);
    
    match tokio::time::timeout(timeout, async {
        loop {
            match Message::receive(stream).await? {
                Some(Message::LLMAccessResponse { granted, message, llm_host, llm_port }) => {
                    return Ok((granted, message, llm_host, llm_port));
                }
                Some(other) => {
                    println!("TCP: Received unexpected message while waiting for LLM access response: {:?}", other);
                    // Continue waiting for the correct response
                    continue;
                }
                None => return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Connection closed")),
            }
        }
    }).await {
        Ok(Ok((granted, message, llm_host, llm_port))) => {
            if granted {
                println!("TCP: LLM access granted by {} - {}", addr, message);
                let mut authorized = AUTHORIZED_PEERS.lock().await;
                authorized.insert(addr.to_string());
                
                // Store LLM connection details if provided
                if let (Some(host), Some(port)) = (llm_host, llm_port) {
                    let mut connections = LLM_CONNECTIONS.lock().await;
                    connections.insert(addr.to_string(), (host.clone(), port));
                    println!("TCP: LLM connection details stored for {} ({}:{})", addr, host, port);
                }
                Ok(true)
            } else {
                println!("TCP: LLM access denied by {} - {}", addr, message);
                Ok(false)
            }
        }
        Ok(Err(e)) => {
            eprintln!("TCP: Error reading LLM access response from {}: {}", addr, e);
            Err(e)
        }
        Err(_) => {
            eprintln!("TCP: Timeout waiting for LLM access response from {}", addr);
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Timeout waiting for LLM access response"
            ))
        }
    }
} 