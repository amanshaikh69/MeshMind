use std::net::IpAddr;
use ipconfig::get_adapters;

pub fn is_my_ip(ip: &str) -> bool {
    if let Ok(adapters) = get_adapters() {
        for adapter in adapters {
            for ip_addr in adapter.ip_addresses() {
                if let IpAddr::V4(ipv4) = ip_addr {
                    if ip == ipv4.to_string() {
                        return true;
                    }
                }
            }
        }
    }
    false
}