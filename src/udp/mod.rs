use tokio::net::UdpSocket;
use tokio::time::{Duration, interval};
use std::collections::HashSet;
use std::str;
use tokio::sync::Mutex;
use std::sync::Arc;
use ipconfig::get_adapters;
use std::net::{IpAddr, Ipv4Addr};

use crate::ip::is_my_ip;

const BROADCAST_PORT: u16 = 5000;
const ONLINE_MESSAGE: &str = "ONLINE";
const BROADCAST_INTERVAL: Duration = Duration::from_secs(30);
const LISTEN_ADDR: &str = "0.0.0.0:5000";

async fn send_broadcast(broadcast_addr: String) -> Result<(), std::io::Error> {
    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.set_broadcast(true)?;
    println!("UDP: Broadcasting to {}", broadcast_addr);
    socket.send_to(ONLINE_MESSAGE.as_bytes(), broadcast_addr).await?;
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
        let (_, src) = socket.recv_from(&mut buf).await?;
        let mut ips = received_ips.lock().await;
        if !is_my_ip(&src.ip().to_string()) {
            ips.insert(src.ip().to_string());
            println!("UDP: Discovered peer {}", src.ip());
        }
    }
}
