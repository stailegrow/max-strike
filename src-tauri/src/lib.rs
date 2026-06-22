use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::{Command, Child};
use std::sync::Mutex;
use tauri::Manager;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub protocol: String,
    pub uuid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sni: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ping: Option<u32>,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub uplink: u64,
    pub downlink: u64,
}

lazy_static::lazy_static! {
    static ref CORE_PROCESS: Mutex<Option<Child>> = Mutex::new(None);
}

fn is_base64(s: &str) -> bool {
    s.len() > 50 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=' || c.is_whitespace())
}

fn base64_decode(s: &str) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let decoded = STANDARD.decode(s).map_err(|e| e.to_string())?;
    String::from_utf8(decoded).map_err(|e| e.to_string())
}

fn parse_subscription_content(content: &str) -> Vec<Server> {
    let mut servers = Vec::new();
    
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        
        if let Some(server) = parse_share_link(line) {
            servers.push(server);
        }
    }
    
    servers
}

fn parse_share_link(link: &str) -> Option<Server> {
    if link.starts_with("vless://") {
        parse_vless(link)
    } else if link.starts_with("trojan://") {
        parse_trojan(link)
    } else if link.starts_with("hysteria2://") || link.starts_with("hy2://") {
        parse_hysteria2(link)
    } else {
        None
    }
}

fn parse_vless(link: &str) -> Option<Server> {
    let without_protocol = &link[8..];
    let parts: Vec<&str> = without_protocol.splitn(2, '#').collect();
    let main_part = parts[0];
    let name = parts.get(1).unwrap_or(&"VLESS Server");
    
    let parts2: Vec<&str> = main_part.splitn(2, '?').collect();
    let user_info_and_host = parts2[0];
    let query_string = parts2.get(1).unwrap_or(&"");
    
    let at_index = user_info_and_host.find('@')?;
    let uuid = &user_info_and_host[..at_index];
    let host_port = &user_info_and_host[at_index + 1..];
    
    let colon_index = host_port.rfind(':')?;
    let address = &host_port[..colon_index];
    let port: u16 = host_port[colon_index + 1..].parse().ok()?;
    
    let params: HashMap<&str, &str> = query_string
        .split('&')
        .filter_map(|p| {
            let mut parts = p.splitn(2, '=');
            Some((parts.next()?, parts.next()?))
        })
        .collect();
    
    Some(Server {
        id: uuid::Uuid::new_v4().to_string(),
        name: urlencoding::decode(name).unwrap_or(std::borrow::Cow::Borrowed(*name)).to_string(),
        protocol: "vless".to_string(),
        address: address.to_string(),
        port,
        uuid: uuid.to_string(),
        flow: params.get("flow").map(|s| s.to_string()),
        sni: params.get("sni").map(|s| s.to_string()),
        public_key: params.get("pbk").map(|s| s.to_string()),
        short_id: params.get("sid").map(|s| s.to_string()),
        security: params.get("security").map(|s| s.to_string()),
        fingerprint: params.get("fp").map(|s| s.to_string()),
        r#type: params.get("type").map(|s| s.to_string()),
        ping: None,
        status: "standby".to_string(),
    })
}

fn parse_trojan(link: &str) -> Option<Server> {
    let without_protocol = &link[9..];
    let parts: Vec<&str> = without_protocol.splitn(2, '#').collect();
    let main_part = parts[0];
    let name = parts.get(1).unwrap_or(&"Trojan Server");
    
    let parts2: Vec<&str> = main_part.splitn(2, '?').collect();
    let user_info_and_host = parts2[0];
    let query_string = parts2.get(1).unwrap_or(&"");
    
    let at_index = user_info_and_host.find('@')?;
    let password = &user_info_and_host[..at_index];
    let host_port = &user_info_and_host[at_index + 1..];
    
    let colon_index = host_port.rfind(':')?;
    let address = &host_port[..colon_index];
    let port: u16 = host_port[colon_index + 1..].parse().ok()?;
    
    let params: HashMap<&str, &str> = query_string
        .split('&')
        .filter_map(|p| {
            let mut parts = p.splitn(2, '=');
            Some((parts.next()?, parts.next()?))
        })
        .collect();
    
    Some(Server {
        id: uuid::Uuid::new_v4().to_string(),
        name: urlencoding::decode(name).unwrap_or(std::borrow::Cow::Borrowed(*name)).to_string(),
        protocol: "trojan".to_string(),
        address: address.to_string(),
        port,
        uuid: password.to_string(),
        flow: None,
        sni: params.get("sni").map(|s| s.to_string()),
        public_key: None,
        short_id: None,
        security: None,
        fingerprint: None,
        r#type: params.get("type").map(|s| s.to_string()),
        ping: None,
        status: "standby".to_string(),
    })
}

fn parse_hysteria2(link: &str) -> Option<Server> {
    let protocol = if link.starts_with("hy2://") { "hy2://" } else { "hysteria2://" };
    let without_protocol = &link[protocol.len()..];
    
    let parts: Vec<&str> = without_protocol.splitn(2, '#').collect();
    let main_part = parts[0];
    let name = parts.get(1).unwrap_or(&"Hysteria2 Server");
    
    let parts2: Vec<&str> = main_part.splitn(2, '?').collect();
    let user_info_and_host = parts2[0];
    let query_string = parts2.get(1).unwrap_or(&"");
    
    let at_index = user_info_and_host.find('@')?;
    let auth = &user_info_and_host[..at_index];
    let host_port = &user_info_and_host[at_index + 1..];
    
    let colon_index = host_port.rfind(':')?;
    let address = &host_port[..colon_index];
    let port: u16 = host_port[colon_index + 1..].parse().ok()?;
    
    let params: HashMap<&str, &str> = query_string
        .split('&')
        .filter_map(|p| {
            let mut parts = p.splitn(2, '=');
            Some((parts.next()?, parts.next()?))
        })
        .collect();
    
    Some(Server {
        id: uuid::Uuid::new_v4().to_string(),
        name: urlencoding::decode(name).unwrap_or(std::borrow::Cow::Borrowed(*name)).to_string(),
        protocol: "hysteria2".to_string(),
        address: address.to_string(),
        port,
        uuid: auth.to_string(),
        flow: None,
        sni: params.get("sni").map(|s| s.to_string()),
        public_key: None,
        short_id: None,
        security: None,
        fingerprint: None,
        r#type: None,
        ping: None,
        status: "standby".to_string(),
    })
}

#[tauri::command]
async fn fetch_subscription(url: String) -> Result<Vec<Server>, String> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("Failed to create client: {}", e))?;
    
    let response = client
        .get(&url)
        .header("User-Agent", "MAX-STRIKE/1.0")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }
    
    let content = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;
    
    let decoded = if is_base64(&content) {
        match base64_decode(&content.replace('\n', "")) {
            Ok(d) => d,
            Err(_) => content,
        }
    } else {
        content
    };
    
    let servers = parse_subscription_content(&decoded);
    Ok(servers)
}

#[tauri::command]
async fn parse_subscription_content_string(content: String) -> Result<Vec<Server>, String> {
    let decoded = if is_base64(&content) {
        match base64_decode(&content.replace('\n', "")) {
            Ok(d) => d,
            Err(_) => content,
        }
    } else {
        content
    };
    
    let servers = parse_subscription_content(&decoded);
    if servers.is_empty() {
        return Err("Не удалось найти ни одного сервера".to_string());
    }
    Ok(servers)
}

#[tauri::command]
async fn connect_to_server(app: tauri::AppHandle, server: Server) -> Result<String, String> {
    let config_json = serde_json::to_string(&server)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    let config_path = "/tmp/max-strike-config.json";
    std::fs::write(config_path, config_json)
        .map_err(|e| format!("Failed to write config: {}", e))?;
    
    // Ищем max-strike-core: сначала в resource dir (production), потом в ./core (dev)
    let resource_dir = app.path().resource_dir().ok();
    let core_path = if let Some(ref dir) = resource_dir {
        let p = dir.join("max-strike-core");
        if p.exists() { p.to_string_lossy().to_string() }
        else { format!("./core/max-strike-core") }
    } else {
        format!("./core/max-strike-core")
    };
    
    // Ищем xray: resource dir, PATH, или /usr/local/bin/xray
    let xray_path = if let Some(ref dir) = resource_dir {
        let p = dir.join("xray");
        if p.exists() { p.to_string_lossy().to_string() }
        else { find_xray() }
    } else {
        find_xray()
    };
    
    // Передаём путь к xray через переменную окружения
    let child = Command::new(&core_path)
        .env("XRAY_PATH", &xray_path)
        .arg("connect")
        .arg(config_path)
        .spawn()
        .map_err(|e| format!("Failed to start core: {} (core: {}, xray: {})", e, core_path, xray_path))?;
    
    {
        let mut process_guard = CORE_PROCESS.lock().unwrap();
        *process_guard = Some(child);
    }
    
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    Ok("Connected successfully\nSOCKS5: 127.0.0.1:10808".to_string())
}

fn find_xray() -> String {
    if let Ok(p) = std::process::Command::new("which").arg("xray").output() {
        if p.status.success() {
            return String::from_utf8_lossy(&p.stdout).trim().to_string();
        }
    }
    
    let paths = vec![
        "/usr/local/bin/xray",
        "/usr/bin/xray",
        "/opt/xray/xray",
    ];
    
    for p in paths {
        if std::path::Path::new(p).exists() {
            return p.to_string();
        }
    }
    
    "xray".to_string()
}

#[tauri::command]
async fn disconnect_from_server() -> Result<String, String> {
    let mut process_guard = CORE_PROCESS.lock().unwrap();
    
    if let Some(mut child) = process_guard.take() {
        child.kill().map_err(|e| format!("Failed to kill process: {}", e))?;
        child.wait().map_err(|e| format!("Failed to wait: {}", e))?;
    }
    
    Ok("Disconnected".to_string())
}

#[tauri::command]
async fn set_system_proxy(enabled: bool) -> Result<String, String> {
    if enabled {
        Command::new("gsettings")
            .args(&["set", "org.gnome.system.proxy", "mode", "manual"])
            .output()
            .map_err(|e| format!("Failed to set mode: {}", e))?;
        
        Command::new("gsettings")
            .args(&["set", "org.gnome.system.proxy.http", "host", ""])
            .output()
            .map_err(|e| format!("Failed to clear HTTP host: {}", e))?;
        
        Command::new("gsettings")
            .args(&["set", "org.gnome.system.proxy.http", "port", "0"])
            .output()
            .map_err(|e| format!("Failed to clear HTTP port: {}", e))?;
        
        Command::new("gsettings")
            .args(&["set", "org.gnome.system.proxy.https", "host", ""])
            .output()
            .map_err(|e| format!("Failed to clear HTTPS host: {}", e))?;
        
        Command::new("gsettings")
            .args(&["set", "org.gnome.system.proxy.https", "port", "0"])
            .output()
            .map_err(|e| format!("Failed to clear HTTPS port: {}", e))?;
        
        Command::new("gsettings")
            .args(&["set", "org.gnome.system.proxy.socks", "host", "127.0.0.1"])
            .output()
            .map_err(|e| format!("Failed to set SOCKS host: {}", e))?;
        
        Command::new("gsettings")
            .args(&["set", "org.gnome.system.proxy.socks", "port", "10808"])
            .output()
            .map_err(|e| format!("Failed to set SOCKS port: {}", e))?;
        
        Command::new("gsettings")
            .args(&["set", "org.gnome.system.proxy", "ignore-hosts", "['localhost', '127.0.0.0/8', '::1', '192.168.0.0/16', '10.0.0.0/8', '172.16.0.0/12']"])
            .output()
            .map_err(|e| format!("Failed to set ignore-hosts: {}", e))?;
        
        Ok("System proxy enabled (SOCKS5 only)".to_string())
    } else {
        Command::new("gsettings")
            .args(&["set", "org.gnome.system.proxy", "mode", "none"])
            .output()
            .map_err(|e| format!("Failed to disable proxy: {}", e))?;
        
        Ok("System proxy disabled".to_string())
    }
}

#[tauri::command]
async fn get_connection_stats() -> Result<ConnectionStats, String> {
    Ok(ConnectionStats {
        uplink: 0,
        downlink: 0,
    })
}

#[tauri::command]
fn get_home_dir() -> Result<String, String> {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Failed to get home directory".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            fetch_subscription,
            parse_subscription_content_string,
            connect_to_server,
            disconnect_from_server,
            set_system_proxy,
            get_connection_stats,
            get_home_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
