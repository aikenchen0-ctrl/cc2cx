//! Process-local loopback fake-IP mapping for Cursor transparent entry.

use std::{collections::HashMap, net::Ipv4Addr};

use super::error::{CursorError, Result};

const POOL_START: u8 = 2;
const POOL_END: u8 = 254;
const LOOPBACK_NET: [u8; 3] = [127, 0, 0];

#[derive(Debug, Clone)]
pub struct FakeIpMap {
    by_host: HashMap<String, Ipv4Addr>,
    by_ip: HashMap<Ipv4Addr, String>,
}

impl FakeIpMap {
    pub fn allocate(hosts: &[String]) -> Result<Self> {
        let mut map = Self {
            by_host: HashMap::new(),
            by_ip: HashMap::new(),
        };
        for host in hosts {
            map.assign(host)?;
        }
        Ok(map)
    }

    pub fn address(&self, hostname: &str) -> Option<Ipv4Addr> {
        self.by_host.get(&normalize_host(hostname)).copied()
    }

    pub fn hostname(&self, address: Ipv4Addr) -> Option<&str> {
        self.by_ip.get(&address).map(String::as_str)
    }

    pub fn entries(&self) -> Vec<(Ipv4Addr, String)> {
        let mut entries: Vec<_> = self
            .by_ip
            .iter()
            .map(|(ip, host)| (*ip, host.clone()))
            .collect();
        entries.sort_by_key(|(ip, _)| *ip);
        entries
    }

    pub fn release(&mut self, hostname: &str) {
        if let Some(ip) = self.by_host.remove(&normalize_host(hostname)) {
            self.by_ip.remove(&ip);
        }
    }

    fn assign(&mut self, hostname: &str) -> Result<Ipv4Addr> {
        let trimmed = hostname.trim();
        if trimmed.is_empty() || trimmed.split_whitespace().nth(1).is_some() {
            return Err(CursorError::Config(format!(
                "fake-IP 主机名无效: {hostname}"
            )));
        }
        let key = normalize_host(trimmed);
        if self.by_host.contains_key(&key) {
            return Err(CursorError::Config(format!(
                "fake-IP 主机名重复: {trimmed}"
            )));
        }
        let ip = self.next_free_ip()?;
        self.by_host.insert(key, ip);
        self.by_ip.insert(ip, trimmed.to_ascii_lowercase());
        Ok(ip)
    }

    fn next_free_ip(&self) -> Result<Ipv4Addr> {
        for octet in POOL_START..=POOL_END {
            let ip = Ipv4Addr::new(LOOPBACK_NET[0], LOOPBACK_NET[1], LOOPBACK_NET[2], octet);
            if !self.by_ip.contains_key(&ip) {
                return Ok(ip);
            }
        }
        Err(CursorError::Config("fake-IP 地址池已耗尽".to_string()))
    }
}

fn normalize_host(hostname: &str) -> String {
    hostname.trim().trim_end_matches('.').to_ascii_lowercase()
}
