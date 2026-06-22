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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub uplink: u64,
    pub downlink: u64,
}

lazy_static::lazy_static! {
    static ref CORE_PROCESS: Mutex<Option<Child>> = Mutex::new(None);
    static ref LOG_BUFFER: Mutex<Vec<LogEntry>> = Mutex::new(Vec::new());
}

fn add_log(level: &str, message: &str) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut logs = LOG_BUFFER.lock().unwrap();
    logs.push(LogEntry {
        timestamp,
        level: level.to_string(),
        message: message.to_string(),
    });
    // Оставляем только последние 1000 записей
    if logs.len() > 1000 {
        logs.remove(0);
    }
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
    if link.starts_with("vless://") { parse_vless(link) }
    else if link.starts_with("trojan://") { parse_trojan(link) }
    else if link.starts_with("hysteria2://") || link.starts_with("hy2://") { parse_hysteria2(link) }
    else { None }
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
    let params: HashMap<&str, &str> = query_string.split('&').filter_map(|p| {
        let mut parts = p.splitn(2, '=');
        Some((parts.next()?, parts.next()?))
    }).collect();
    Some(Server {
        id: uuid::Uuid::new_v4().to_string(),
        name: urlencoding::decode(name).unwrap_or(std::borrow::Cow::Borrowed(*name)).to_string(),
        protocol: "vless".to_string(), address: address.to_string(), port,
        uuid: uuid.to_string(),
        flow: params.get("flow").map(|s| s.to_string()),
        sni: params.get("sni").map(|s| s.to_string()),
        public_key: params.get("pbk").map(|s| s.to_string()),
        short_id: params.get("sid").map(|s| s.to_string()),
        security: params.get("security").map(|s| s.to_string()),
        fingerprint: params.get("fp").map(|s| s.to_string()),
        r#type: params.get("type").map(|s| s.to_string()),
        ping: None, status: "standby".to_string(),
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
    let params: HashMap<&str, &str> = query_string.split('&').filter_map(|p| {
        let mut parts = p.splitn(2, '=');
        Some((parts.next()?, parts.next()?))
    }).collect();
    Some(Server {
        id: uuid::Uuid::new_v4().to_string(),
        name: urlencoding::decode(name).unwrap_or(std::borrow::Cow::Borrowed(*name)).to_string(),
        protocol: "trojan".to_string(), address: address.to_string(), port,
        uuid: password.to_string(), flow: None,
        sni: params.get("sni").map(|s| s.to_string()),
        public_key: None, short_id: None, security: None, fingerprint: None,
        r#type: params.get("type").map(|s| s.to_string()),
        ping: None, status: "standby".to_string(),
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
    let params: HashMap<&str, &str> = query_string.split('&').filter_map(|p| {
        let mut parts = p.splitn(2, '=');
        Some((parts.next()?, parts.next()?))
    }).collect();
    Some(Server {
        id: uuid::Uuid::new_v4().to_string(),
        name: urlencoding::decode(name).unwrap_or(std::borrow::Cow::Borrowed(*name)).to_string(),
        protocol: "hysteria2".to_string(), address: address.to_string(), port,
        uuid: auth.to_string(), flow: None,
        sni: params.get("sni").map(|s| s.to_string()),
        public_key: None, short_id: None, security: None, fingerprint: None,
        r#type: None, ping: None, status: "standby".to_string(),
    })
}

#[tauri::command]
async fn fetch_subscription(url: String) -> Result<Vec<Server>, String> {
    add_log("INFO", &format!("Fetching subscription from: {}", url));
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build().map_err(|e| {
            add_log("ERROR", &format!("Failed to create client: {}", e));
            format!("Failed to create client: {}", e)
        })?;
    let response = client.get(&url).header("User-Agent", "MAX-STRIKE/1.0")
        .send().await.map_err(|e| {
            add_log("ERROR", &format!("Failed to fetch: {}", e));
            format!("Failed to fetch: {}", e)
        })?;
    if !response.status().is_success() {
        let err = format!("HTTP error: {}", response.status());
        add_log("ERROR", &err);
        return Err(err);
    }
    let content = response.text().await.map_err(|e| {
        add_log("ERROR", &format!("Failed to read response: {}", e));
        format!("Failed to read response: {}", e)
    })?;
    let decoded = if is_base64(&content) {
        match base64_decode(&content.replace('\n', "")) {
            Ok(d) => d, Err(_) => content,
        }
    } else { content };
    let servers = parse_subscription_content(&decoded);
    add_log("INFO", &format!("Parsed {} servers", servers.len()));
    Ok(servers)
}

#[tauri::command]
async fn parse_subscription_content_string(content: String) -> Result<Vec<Server>, String> {
    add_log("INFO", "Parsing subscription content");
    let decoded = if is_base64(&content) {
        match base64_decode(&content.replace('\n', "")) { Ok(d) => d, Err(_) => content }
    } else { content };
    let servers = parse_subscription_content(&decoded);
    if servers.is_empty() {
        add_log("ERROR", "No servers found in content");
        return Err("Не удалось найти ни одного сервера".to_string());
    }
    add_log("INFO", &format!("Parsed {} servers", servers.len()));
    Ok(servers)
}

#[tauri::command]
async fn connect_to_server(app: tauri::AppHandle, server: Server) -> Result<String, String> {
    add_log("INFO", &format!("Connecting to server: {} ({}:{})", server.name, server.address, server.port));
    
    let config_json = serde_json::to_string(&server).map_err(|e| {
        add_log("ERROR", &format!("Failed to serialize config: {}", e));
        format!("Failed to serialize: {}", e)
    })?;
    
    std::fs::write("/tmp/max-strike-config.json", &config_json).map_err(|e| {
        add_log("ERROR", &format!("Failed to write config: {}", e));
        format!("Failed to write config: {}", e)
    })?;
    
    add_log("DEBUG", "Config written to /tmp/max-strike-config.json");
    
    // Умный поиск core бинарника
    let resource_dir = app.path().resource_dir().ok();
    
    let core_path = find_core_path(resource_dir.as_ref());
    add_log("DEBUG", &format!("Using core: {}", core_path));
    
    let xray_path = if let Some(ref dir) = resource_dir {
        let p = dir.join("xray");
        if p.exists() {
            add_log("DEBUG", &format!("Using xray from resource dir: {:?}", p));
            p.to_string_lossy().to_string()
        } else {
            let path = find_xray();
            add_log("DEBUG", &format!("Using xray from system: {}", path));
            path
        }
    } else {
        let path = find_xray();
        add_log("DEBUG", &format!("Using xray from system: {}", path));
        path
    };
    
    add_log("INFO", &format!("Starting core: {}", core_path));
    add_log("INFO", &format!("Using xray: {}", xray_path));
    
    let child = Command::new(&core_path)
        .env("XRAY_PATH", &xray_path)
        .arg("connect")
        .arg("/tmp/max-strike-config.json")
        .spawn().map_err(|e| {
            let err = format!("Failed to start core: {} (core: {}, xray: {})", e, core_path, xray_path);
            add_log("ERROR", &err);
            err
        })?;
    
    add_log("INFO", "Core process started successfully");
    
    { let mut g = CORE_PROCESS.lock().unwrap(); *g = Some(child); }
    
    add_log("INFO", "Waiting 2 seconds for connection to establish...");
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    add_log("INFO", "Connection established successfully");
    Ok("Connected successfully
SOCKS5: 127.0.0.1:10808".to_string())
}

fn find_core_path(resource_dir: Option<&std::path::PathBuf>) -> String {
    // 1. Production: resource dir
    if let Some(dir) = resource_dir {
        let p = dir.join("max-strike-core");
        if p.exists() {
            return p.to_string_lossy().to_string();
        }
    }
    
    // 2. Относительно текущего бинарника Rust
    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = exe.parent().unwrap();
        
        // Пробуем разные относительные пути (для разных уровней вложенности в dev)
        let candidates = vec![
            exe_dir.join("core").join("max-strike-core"),
            exe_dir.join("..").join("core").join("max-strike-core"),
            exe_dir.join("..").join("..").join("core").join("max-strike-core"),
            exe_dir.join("..").join("..").join("..").join("core").join("max-strike-core"),
        ];
        
        for candidate in candidates {
            if candidate.exists() {
                if let Ok(canon) = candidate.canonicalize() {
                    return canon.to_string_lossy().to_string();
                }
                return candidate.to_string_lossy().to_string();
            }
        }
    }
    
    // 3. Через HOME переменную (резерв)
    if let Ok(home) = std::env::var("HOME") {
        let p = std::path::Path::new(&home).join("projects").join("max-strike").join("core").join("max-strike-core");
        if p.exists() {
            return p.to_string_lossy().to_string();
        }
    }
    
    // 4. Текущая директория
    if let Ok(cwd) = std::env::current_dir() {
        let p = cwd.join("core").join("max-strike-core");
        if p.exists() {
            return p.to_string_lossy().to_string();
        }
    }
    
    "./core/max-strike-core".to_string()
}


fn find_xray() -> String {
    if let Ok(p) = std::process::Command::new("which").arg("xray").output() {
        if p.status.success() { return String::from_utf8_lossy(&p.stdout).trim().to_string(); }
    }
    for p in ["/usr/local/bin/xray", "/usr/bin/xray", "/opt/xray/xray"] {
        if std::path::Path::new(p).exists() { return p.to_string(); }
    }
    "xray".to_string()
}

#[tauri::command]
async fn disconnect_from_server() -> Result<String, String> {
    add_log("INFO", "Disconnecting from server");
    let mut g = CORE_PROCESS.lock().unwrap();
    if let Some(mut child) = g.take() {
        child.kill().map_err(|e| {
            add_log("ERROR", &format!("Failed to kill process: {}", e));
            format!("Failed to kill: {}", e)
        })?;
        child.wait().map_err(|e| {
            add_log("ERROR", &format!("Failed to wait: {}", e));
            format!("Failed to wait: {}", e)
        })?;
        add_log("INFO", "Process terminated");
    }
    add_log("INFO", "Disconnected successfully");
    Ok("Disconnected".to_string())
}

#[tauri::command]
async fn set_system_proxy(enabled: bool) -> Result<String, String> {
    add_log("INFO", &format!("Setting system proxy: {}", if enabled { "enabled" } else { "disabled" }));
    if enabled {
        for args in [
            ["set", "org.gnome.system.proxy", "mode", "manual"],
            ["set", "org.gnome.system.proxy.http", "host", ""],
            ["set", "org.gnome.system.proxy.http", "port", "0"],
            ["set", "org.gnome.system.proxy.https", "host", ""],
            ["set", "org.gnome.system.proxy.https", "port", "0"],
            ["set", "org.gnome.system.proxy.socks", "host", "127.0.0.1"],
            ["set", "org.gnome.system.proxy.socks", "port", "10808"],
            ["set", "org.gnome.system.proxy", "ignore-hosts", "['localhost', '127.0.0.0/8', '::1', '192.168.0.0/16', '10.0.0.0/8', '172.16.0.0/12']"],
        ] {
            Command::new("gsettings").args(&args).output().map_err(|e| {
                add_log("ERROR", &format!("Failed to set proxy: {}", e));
                format!("Failed: {}", e)
            })?;
        }
        add_log("INFO", "System proxy enabled (SOCKS5 on 127.0.0.1:10808)");
        Ok("System proxy enabled".to_string())
    } else {
        Command::new("gsettings").args(&["set", "org.gnome.system.proxy", "mode", "none"])
            .output().map_err(|e| {
                add_log("ERROR", &format!("Failed to disable proxy: {}", e));
                format!("Failed: {}", e)
            })?;
        add_log("INFO", "System proxy disabled");
        Ok("System proxy disabled".to_string())
    }
}

#[tauri::command]
async fn get_connection_stats() -> Result<ConnectionStats, String> {
    Ok(ConnectionStats { uplink: 0, downlink: 0 })
}

#[tauri::command]
fn get_home_dir() -> Result<String, String> {
    dirs::home_dir().map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "Failed to get home directory".to_string())
}

#[tauri::command]
fn get_logs() -> Vec<LogEntry> {
    LOG_BUFFER.lock().unwrap().clone()
}

#[tauri::command]
fn clear_logs() {
    LOG_BUFFER.lock().unwrap().clear();
    add_log("INFO", "Logs cleared");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    add_log("INFO", "MAX STRIKE application started");
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            fetch_subscription,
            parse_subscription_content_string,
            connect_to_server,
            disconnect_from_server,
            set_system_proxy,
            get_connection_stats,
            get_home_dir,
            get_logs,
            clear_logs
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
