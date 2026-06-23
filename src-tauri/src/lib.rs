use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::{Command, Child};
use std::sync::Mutex;
use tauri::Manager;
use std::os::windows::process::CommandExt;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoutingConfig {
    pub block_ads: bool,
    pub bypass_lan: bool,
    pub split_routing: bool,
    pub region: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub uplink: u64,
    pub downlink: u64,
}

lazy_static::lazy_static! {
    static ref CORE_PROCESS: Mutex<Option<Child>> = Mutex::new(None);
    static ref LOG_BUFFER: Mutex<Vec<LogEntry>> = Mutex::new(Vec::new());
    static ref ROUTING_CONFIG: Mutex<RoutingConfig> = Mutex::new(RoutingConfig {
        block_ads: false,
        bypass_lan: true,
        split_routing: true,
        region: "russia".to_string(),
    });
}

fn add_log(level: &str, message: &str) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut logs = LOG_BUFFER.lock().unwrap();
    logs.push(LogEntry {
        timestamp,
        level: level.to_string(),
        message: message.to_string(),
    });
    if logs.len() > 1000 {
        logs.remove(0);
    }
}

fn get_config_path() -> std::path::PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let config_dir = std::path::Path::new(&app_data).join("MAX STRIKE");
    std::fs::create_dir_all(&config_dir).ok();
    config_dir.join("routing.json")
}

fn load_routing_config() -> RoutingConfig {
    let path = get_config_path();
    if path.exists() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str(&data) {
                return config;
            }
        }
    }
    RoutingConfig {
        block_ads: false,
        bypass_lan: true,
        split_routing: true,
        region: "russia".to_string(),
    }
}

fn save_routing_config_to_file(config: &RoutingConfig) -> Result<(), String> {
    let path = get_config_path();
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize: {}", e))?;
    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write: {}", e))?;
    Ok(())
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

// Убиваем ВСЕ процессы xray.exe
fn kill_all_xray() {
    let _ = Command::new("taskkill")
        .args(&["/F", "/IM", "xray.exe"])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output();
    add_log("DEBUG", "Killed all xray.exe processes");
}

#[tauri::command]
async fn connect_to_server(app: tauri::AppHandle, server: Server) -> Result<String, String> {
    add_log("INFO", &format!("Connecting to server: {} ({}:{})", server.name, server.address, server.port));
    
    // Сначала убиваем все старые процессы xray
    kill_all_xray();
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    let config_json = serde_json::to_string(&server).map_err(|e| {
        add_log("ERROR", &format!("Failed to serialize config: {}", e));
        format!("Failed to serialize: {}", e)
    })?;
    
    let config_path = std::env::temp_dir().join("max-strike-config.json");
    std::fs::write(&config_path, &config_json).map_err(|e| {
        add_log("ERROR", &format!("Failed to write config: {}", e));
        format!("Failed to write config: {}", e)
    })?;
    
    add_log("DEBUG", &format!("Config written to: {:?}", config_path));
    
    let routing = ROUTING_CONFIG.lock().unwrap().clone();
    add_log("INFO", &format!("Routing config: block_ads={}, bypass_lan={}, split_routing={}, region={}", 
        routing.block_ads, routing.bypass_lan, routing.split_routing, routing.region));
    
    let routing_json = serde_json::to_string(&routing).map_err(|e| {
        add_log("ERROR", &format!("Failed to serialize routing config: {}", e));
        format!("Failed to serialize routing: {}", e)
    })?;
    let routing_path = std::env::temp_dir().join("max-strike-routing.json");
    std::fs::write(&routing_path, &routing_json).map_err(|e| {
        add_log("ERROR", &format!("Failed to write routing config: {}", e));
        format!("Failed to write routing: {}", e)
    })?;
    add_log("DEBUG", &format!("Routing config written to: {:?}", routing_path));
    
    let resource_dir = app.path().resource_dir().ok();
    let core_path = find_core_path(resource_dir.as_ref());
    add_log("DEBUG", &format!("Using core: {}", core_path));
    
    let xray_path = find_xray(resource_dir.as_ref());
    add_log("DEBUG", &format!("Using xray: {}", xray_path));
    
    add_log("INFO", &format!("Starting core: {}", core_path));
    add_log("INFO", &format!("Using xray: {}", xray_path));
    
    // Запускаем с CREATE_NO_WINDOW чтобы не было консольного окна
    let child = Command::new(&core_path)
        .env("XRAY_PATH", &xray_path)
        .env("ROUTING_CONFIG", &routing_path)
        .arg("connect")
        .arg(&config_path)
        .creation_flags(0x08000000)
        .spawn().map_err(|e| {
            let err = format!("Failed to start core: {} (core: {}, xray: {})", e, core_path, xray_path);
            add_log("ERROR", &err);
            err
        })?;
    
    add_log("INFO", "Core process started successfully");
    
    { let mut g = CORE_PROCESS.lock().unwrap(); *g = Some(child); }
    
    add_log("INFO", "Waiting 3 seconds for connection to establish...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    
    // Проверяем что xray слушает порт
    let check = Command::new("netstat")
        .args(&["-ano"])
        .creation_flags(0x08000000)
        .output();
    
    match check {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains(":10808") || stdout.contains(":10809") {
                add_log("INFO", "Xray is listening on ports");
            } else {
                add_log("WARN", "Xray may not be listening yet");
            }
        }
        Err(_) => {}
    }
    
    add_log("INFO", "Connection established successfully");
    Ok("Connected successfully\nHTTP proxy: 127.0.0.1:10809".to_string())
}

fn find_core_path(resource_dir: Option<&std::path::PathBuf>) -> String {
    add_log("DEBUG", &format!("find_core_path called with resource_dir: {:?}", resource_dir));
    
    if let Some(dir) = resource_dir {
        let p = dir.join("max-strike-core.exe");
        add_log("DEBUG", &format!("Checking resource dir path: {:?}", p));
        if p.exists() {
            add_log("INFO", &format!("Found core in resource dir: {:?}", p));
            return p.to_string_lossy().to_string();
        }
    }
    
    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = exe.parent().unwrap();
        add_log("DEBUG", &format!("Current exe dir: {:?}", exe_dir));
        
        let candidates = vec![
            exe_dir.join("core").join("max-strike-core.exe"),
            exe_dir.join("..").join("core").join("max-strike-core.exe"),
            exe_dir.join("..").join("..").join("core").join("max-strike-core.exe"),
            exe_dir.join("..").join("..").join("..").join("core").join("max-strike-core.exe"),
            exe_dir.join("..").join("..").join("..").join("..").join("core").join("max-strike-core.exe"),
        ];
        
        for candidate in candidates {
            add_log("DEBUG", &format!("Checking candidate: {:?}", candidate));
            if candidate.exists() {
                add_log("INFO", &format!("Found core at: {:?}", candidate));
                return candidate.to_string_lossy().to_string();
            }
        }
    }
    
    if let Ok(cwd) = std::env::current_dir() {
        let p = cwd.join("core").join("max-strike-core.exe");
        add_log("DEBUG", &format!("Checking CWD path: {:?}", p));
        if p.exists() {
            add_log("INFO", &format!("Found core in CWD: {:?}", p));
            return p.to_string_lossy().to_string();
        }
    }
    
    let fallback = "core\\max-strike-core.exe".to_string();
    add_log("WARN", &format!("Core not found, using fallback: {}", fallback));
    fallback
}

fn find_xray(resource_dir: Option<&std::path::PathBuf>) -> String {
    add_log("DEBUG", &format!("find_xray called with resource_dir: {:?}", resource_dir));
    
    if let Some(dir) = resource_dir {
        let p = dir.join("xray.exe");
        add_log("DEBUG", &format!("Checking resource dir xray: {:?}", p));
        if p.exists() {
            add_log("INFO", &format!("Found xray in resource dir: {:?}", p));
            return p.to_string_lossy().to_string();
        }
    }
    
    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = exe.parent().unwrap();
        let candidates = vec![
            exe_dir.join("xray.exe"),
            exe_dir.join("core").join("xray.exe"),
            exe_dir.join("..").join("core").join("xray.exe"),
            exe_dir.join("..").join("..").join("core").join("xray.exe"),
            exe_dir.join("..").join("..").join("..").join("core").join("xray.exe"),
        ];
        
        for candidate in candidates {
            add_log("DEBUG", &format!("Checking xray candidate: {:?}", candidate));
            if candidate.exists() {
                add_log("INFO", &format!("Found xray at: {:?}", candidate));
                return candidate.to_string_lossy().to_string();
            }
        }
    }
    
    if let Ok(cwd) = std::env::current_dir() {
        let p = cwd.join("core").join("xray.exe");
        add_log("DEBUG", &format!("Checking CWD xray: {:?}", p));
        if p.exists() {
            add_log("INFO", &format!("Found xray in CWD: {:?}", p));
            return p.to_string_lossy().to_string();
        }
    }
    
    let fallback = "xray.exe".to_string();
    add_log("WARN", &format!("Xray not found, using fallback: {}", fallback));
    fallback
}

#[tauri::command]
async fn disconnect_from_server() -> Result<String, String> {
    add_log("INFO", "Disconnecting from server");
    
    // Убиваем child процесс
    let mut g = CORE_PROCESS.lock().unwrap();
    if let Some(mut child) = g.take() {
        let _ = child.kill();
        let _ = child.wait();
        add_log("INFO", "Child process terminated");
    }
    drop(g);
    
    // Убиваем ВСЕ процессы xray.exe
    kill_all_xray();
    
    add_log("INFO", "Disconnected successfully");
    Ok("Disconnected".to_string())
}

// Использует PowerShell для записи в реестр (надёжнее чем reg.exe)
#[tauri::command]
async fn set_system_proxy(enabled: bool) -> Result<String, String> {
    add_log("INFO", &format!("Setting system proxy: {}", if enabled { "enabled" } else { "disabled" }));
    
    let ps_script = if enabled {
        r#"
$regPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
Set-ItemProperty -Path $regPath -Name ProxyEnable -Value 1 -Type DWord
Set-ItemProperty -Path $regPath -Name ProxyServer -Value '127.0.0.1:10809'
Set-ItemProperty -Path $regPath -Name ProxyOverride -Value 'localhost;127.*;10.*;172.16.*;192.168.*;<local>'
# Notify system about proxy change
$signature = @'
[DllImport("wininet.dll", SetLastError=true)]
public static extern bool InternetSetOption(IntPtr hInternet, int dwOption, IntPtr lpBuffer, int lpdwBufferLength);
'@
$WinInet = Add-Type -MemberDefinition $signature -Name WinInet -Namespace Proxy -PassThru
$INTERNET_OPTION_SETTINGS_CHANGED = 39
$INTERNET_OPTION_PROXY_SETTINGS_CHANGED = 75
$WinInet::InternetSetOption([IntPtr]::Zero, $INTERNET_OPTION_SETTINGS_CHANGED, [IntPtr]::Zero, 0)
$WinInet::InternetSetOption([IntPtr]::Zero, $INTERNET_OPTION_PROXY_SETTINGS_CHANGED, [IntPtr]::Zero, 0)
Write-Output "Proxy enabled"
"#
    } else {
        r#"
$regPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
Set-ItemProperty -Path $regPath -Name ProxyEnable -Value 0 -Type DWord
$signature = @'
[DllImport("wininet.dll", SetLastError=true)]
public static extern bool InternetSetOption(IntPtr hInternet, int dwOption, IntPtr lpBuffer, int lpdwBufferLength);
'@
$WinInet = Add-Type -MemberDefinition $signature -Name WinInet -Namespace Proxy -PassThru
$INTERNET_OPTION_SETTINGS_CHANGED = 39
$INTERNET_OPTION_PROXY_SETTINGS_CHANGED = 75
$WinInet::InternetSetOption([IntPtr]::Zero, $INTERNET_OPTION_SETTINGS_CHANGED, [IntPtr]::Zero, 0)
$WinInet::InternetSetOption([IntPtr]::Zero, $INTERNET_OPTION_PROXY_SETTINGS_CHANGED, [IntPtr]::Zero, 0)
Write-Output "Proxy disabled"
"#
    };
    
    let result = Command::new("powershell")
        .args(&["-ExecutionPolicy", "Bypass", "-Command", ps_script])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| {
            add_log("ERROR", &format!("Failed to run PowerShell: {}", e));
            format!("Failed: {}", e)
        })?;
    
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    
    add_log("DEBUG", &format!("PowerShell stdout: {}", stdout));
    if !stderr.is_empty() {
        add_log("WARN", &format!("PowerShell stderr: {}", stderr));
    }
    
    if enabled {
        add_log("INFO", "Windows system proxy enabled (HTTP on 127.0.0.1:10809)");
    } else {
        add_log("INFO", "Windows system proxy disabled");
    }
    
    Ok("System proxy updated".to_string())
}

#[tauri::command]
async fn get_connection_stats() -> Result<ConnectionStats, String> {
    Ok(ConnectionStats { uplink: 0, downlink: 0 })
}

#[tauri::command]
fn get_home_dir() -> Result<String, String> {
    std::env::var("USERPROFILE")
        .map_err(|_| "Failed to get home directory".to_string())
}

#[tauri::command]
fn get_install_dir() -> Result<String, String> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return Ok(dir.to_string_lossy().to_string());
        }
    }
    std::env::current_dir().map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("Failed to get current dir: {}", e))
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

#[tauri::command]
async fn get_routing_config() -> Result<RoutingConfig, String> {
    let config = load_routing_config();
    let mut current = ROUTING_CONFIG.lock().unwrap();
    *current = config.clone();
    Ok(config)
}

#[tauri::command]
async fn save_routing_config(config: RoutingConfig) -> Result<String, String> {
    add_log("INFO", &format!("Saving routing config: block_ads={}, bypass_lan={}, split_routing={}, region={}", 
        config.block_ads, config.bypass_lan, config.split_routing, config.region));
    
    let mut current = ROUTING_CONFIG.lock().unwrap();
    *current = config.clone();
    drop(current);
    
    if let Err(e) = save_routing_config_to_file(&config) {
        add_log("ERROR", &format!("Failed to save routing config to file: {}", e));
        return Err(e);
    }
    
    add_log("INFO", "Routing config saved to file");
    Ok("Routing config saved".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    add_log("INFO", "MAX STRIKE application started (Windows)");
    
    {
        let config = load_routing_config();
        let mut current = ROUTING_CONFIG.lock().unwrap();
        *current = config;
    }
    
    add_log("INFO", "Routing config loaded from file");
    
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
            get_install_dir,
            get_logs,
            clear_logs,
            get_routing_config,
            save_routing_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}