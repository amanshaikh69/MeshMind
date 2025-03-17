use tokio::net::UdpSocket;
use tokio::time::{Duration, interval};
use std::collections::HashSet;
use std::str;
use tokio::sync::Mutex;
use std::sync::Arc;
use ipconfig::get_adapters;
use std::net::{IpAddr, Ipv4Addr};
use serde::{Serialize, Deserialize};
use reqwest::Client;

use crate::ip::is_my_ip;

const BROADCAST_PORT: u16 = 5000;
const ONLINE_MESSAGE: &str = "ONLINE";
const BROADCAST_INTERVAL: Duration = Duration::from_secs(30);
const LISTEN_ADDR: &str = "0.0.0.0:5000";
const OLLAMA_CHECK_URL: &str = "http://127.0.0.1:11434/api/tags";

#[derive(Debug, Serialize, Deserialize)]
struct BroadcastMessage {
    message_type: String,
    has_llm: bool,
}

// Check if Ollama is running
async fn is_ollama_available() -> bool {
    if let Ok(client) = Client::builder()
        .timeout(Duration::from_secs(2))
        .build() 
    {
        match client.get(OLLAMA_CHECK_URL).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    } else {
        false
    }
}

async fn send_broadcast(broadcast_addr: String) -> Result<(), std::io::Error> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;
    
    let has_llm = is_ollama_available().await;
    let message = BroadcastMessage {
        message_type: "ONLINE".to_string(),
        has_llm,
    };
    
    let message_bytes = serde_json::to_string(&message)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        .into_bytes();
    
    println!("UDP: Broadcasting to {} (LLM available: {})", broadcast_addr, has_llm);
    socket.send_to(&message_bytes, broadcast_addr).await?;
    Ok(())
}

pub async fn periodic_broadcast() {
    let mut interval = interval(BROADCAST_INTERVAL);
    loop {
        interval.tick().await;
        if let Ok(adapters) = get_adapters() {
            for adapter in adapters {
                if adapter.oper_status() == ipconfig::OperStatus::IfOperStatusUp {
                    for ip_addr in adapter.ip_addresses() {
                        if let IpAddr::V4(_ipv4_addr) = ip_addr {
                            let subnet_mask = match adapter.ip_addresses().iter().find_map(|ip| match ip {
                                IpAddr::V4(ipv4) => Some(ipv4),
                                _ => None,
                            }) {
                                Some(ipv4) => match ipv4.octets() {
                                    [a, b, c, _] => Some(Ipv4Addr::new(a, b, c, 255)),
                                },
                                None => None,
                            };
                            if let Some(broadcast_addr) = subnet_mask {
                                let broadcast_addr = format!("{}:{}", broadcast_addr, BROADCAST_PORT);
                                if let Err(e) = send_broadcast(broadcast_addr).await {
                                    eprintln!("UDP: Broadcast error: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub async fn receive_broadcast(received_ips: Arc<Mutex<HashSet<String>>>) -> Result<(), std::io::Error> {
    println!("UDP: Listening on {}", LISTEN_ADDR);
    let socket = UdpSocket::bind(LISTEN_ADDR).await?;
    let mut buf = [0; 1024];

    loop {
        let (size, src) = socket.recv_from(&mut buf).await?;
        if let Ok(message_str) = String::from_utf8(buf[..size].to_vec()) {
            if let Ok(broadcast_msg) = serde_json::from_str::<BroadcastMessage>(&message_str) {
                let mut ips = received_ips.lock().await;
                if !is_my_ip(&src.ip().to_string()) {
                    ips.insert(src.ip().to_string());
                    println!("UDP: Discovered peer {} (LLM available: {})", src.ip(), broadcast_msg.has_llm);
                }
            }
        }
    }
}
