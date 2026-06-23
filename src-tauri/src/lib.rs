use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::{Command, Child, Stdio};
use std::sync::Mutex;
use std::io::{BufRead, BufReader};
use std::thread;
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

fn normalize_path(path: &std::path::Path) -> std::path::PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with(r"\\?\") {
        std::path::PathBuf::from(&s[4..])
    } else {
        path.to_path_buf()
    }
}

lazy_static::lazy_static! {
    static ref CORE_PROCESS: Mutex<Option<Child>> = Mutex::new(None);
    static ref CORE_PID: Mutex<Option<u32>> = Mutex::new(None);
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
    if let Ok(mut logs) = LOG_BUFFER.lock() {
        logs.push(LogEntry {
            timestamp,
            level: level.to_string(),
            message: message.to_string(),
        });
        if logs.len() > 2000 {
            logs.remove(0);
        }
    }
}

fn get_config_path() -> std::path::PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| "C:\\ProgramData".to_string());
    let config_dir = std::path::Path::new(&app_data).join("MAX STRIKE");
    let _ = std::fs::create_dir_all(&config_dir);
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
        .map_err(|e| format!("Serialize error: {}", e))?;
    std::fs::write(&path, json)
        .map_err(|e| format!("Write error: {}", e))?;
    Ok(())
}

fn is_base64(s: &str) -> bool {
    s.len() > 50 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=' || c.is_whitespace())
}

fn base64_decode(s: &str) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let decoded = STANDARD.decode(s.trim()).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&decoded).to_string())
}

fn parse_subscription_content(content: &str) -> Vec<Server> {
    content.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("//"))
        .filter_map(parse_share_link)
        .collect()
}

fn parse_share_link(link: &str) -> Option<Server> {
    if link.starts_with("vless://") { parse_vless(link) }
    else if link.starts_with("trojan://") { parse_trojan(link) }
    else if link.starts_with("hysteria2://") || link.starts_with("hy2://") { parse_hysteria2(link) }
    else { None }
}

fn parse_query_params(query: &str) -> HashMap<&str, &str> {
    query.split('&').filter_map(|p| {
        let mut parts = p.splitn(2, '=');
        Some((parts.next()?, parts.next()?))
    }).collect()
}

fn decode_name(name: &str) -> String {
    urlencoding::decode(name).unwrap_or(std::borrow::Cow::Borrowed(name)).to_string()
}

fn parse_vless(link: &str) -> Option<Server> {
    let without_protocol = &link[8..];
    let parts: Vec<&str> = without_protocol.splitn(2, '#').collect();
    let main_part = parts[0];
    let name = decode_name(parts.get(1).unwrap_or(&"VLESS Server"));
    let parts2: Vec<&str> = main_part.splitn(2, '?').collect();
    let user_info_and_host = parts2[0];
    let query_string = parts2.get(1).unwrap_or(&"");
    let at_index = user_info_and_host.find('@')?;
    let uuid = &user_info_and_host[..at_index];
    let host_port = &user_info_and_host[at_index + 1..];
    let colon_index = host_port.rfind(':')?;
    let address = &host_port[..colon_index];
    let port: u16 = host_port[colon_index + 1..].parse().ok()?;
    let params = parse_query_params(query_string);
    Some(Server {
        id: uuid::Uuid::new_v4().to_string(),
        name, protocol: "vless".to_string(), address: address.to_string(), port,
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
    let name = decode_name(parts.get(1).unwrap_or(&"Trojan Server"));
    let parts2: Vec<&str> = main_part.splitn(2, '?').collect();
    let user_info_and_host = parts2[0];
    let query_string = parts2.get(1).unwrap_or(&"");
    let at_index = user_info_and_host.find('@')?;
    let password = &user_info_and_host[..at_index];
    let host_port = &user_info_and_host[at_index + 1..];
    let colon_index = host_port.rfind(':')?;
    let address = &host_port[..colon_index];
    let port: u16 = host_port[colon_index + 1..].parse().ok()?;
    let params = parse_query_params(query_string);
    Some(Server {
        id: uuid::Uuid::new_v4().to_string(),
        name, protocol: "trojan".to_string(), address: address.to_string(), port,
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
    let name = decode_name(parts.get(1).unwrap_or(&"Hysteria2 Server"));
    let parts2: Vec<&str> = main_part.splitn(2, '?').collect();
    let user_info_and_host = parts2[0];
    let query_string = parts2.get(1).unwrap_or(&"");
    let at_index = user_info_and_host.find('@')?;
    let auth = &user_info_and_host[..at_index];
    let host_port = &user_info_and_host[at_index + 1..];
    let colon_index = host_port.rfind(':')?;
    let address = &host_port[..colon_index];
    let port: u16 = host_port[colon_index + 1..].parse().ok()?;
    let params = parse_query_params(query_string);
    Some(Server {
        id: uuid::Uuid::new_v4().to_string(),
        name, protocol: "hysteria2".to_string(), address: address.to_string(), port,
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
        .build().map_err(|e| format!("Client error: {}", e))?;
    let response = client.get(&url)
        .header("User-Agent", "MAX-STRIKE/1.0")
        .send().await.map_err(|e| format!("Fetch error: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let content = response.text().await.map_err(|e| format!("Read error: {}", e))?;
    let decoded = if is_base64(&content) {
        base64_decode(&content).unwrap_or(content)
    } else { content };
    let servers = parse_subscription_content(&decoded);
    add_log("INFO", &format!("Parsed {} servers", servers.len()));
    Ok(servers)
}

#[tauri::command]
async fn parse_subscription_content_string(content: String) -> Result<Vec<Server>, String> {
    add_log("INFO", "Parsing subscription content");
    let decoded = if is_base64(&content) {
        base64_decode(&content).unwrap_or(content)
    } else { content };
    let servers = parse_subscription_content(&decoded);
    if servers.is_empty() {
        return Err("Не удалось найти ни одного сервера".to_string());
    }
    add_log("INFO", &format!("Parsed {} servers", servers.len()));
    Ok(servers)
}

// Убиваем core и все дочерние процессы (включая xray)
fn kill_core() {
    if let Ok(mut pid_lock) = CORE_PID.lock() {
        if let Some(pid) = pid_lock.take() {
            let _ = Command::new("taskkill")
                .args(&["/F", "/T", "/PID", &pid.to_string()])
                .creation_flags(0x08000000)
                .output();
            add_log("DEBUG", &format!("Killed core process tree with PID {}", pid));
        }
    }
}

fn kill_all_xray() {
    let _ = Command::new("taskkill")
        .args(&["/F", "/IM", "xray.exe"])
        .creation_flags(0x08000000)
        .output();
    add_log("DEBUG", "Killed all xray.exe processes");
}

// ИСПРАВЛЕНО: точная проверка порта (не совпадает :1080 с :10808)
fn is_port_listening(port: u16) -> bool {
    if let Ok(output) = Command::new("netstat")
        .args(&["-ano"])
        .creation_flags(0x08000000)
        .output() 
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let port_str = format!(":{}\t", port);
        let port_str2 = format!(":{} ", port);
        stdout.contains(&port_str) || stdout.contains(&port_str2)
    } else {
        false
    }
}

// Читаем stdout от core в отдельном потоке
fn spawn_output_reader(child_stdout: Option<std::process::ChildStdout>, prefix: &str) {
    if let Some(stdout) = child_stdout {
        let prefix = prefix.to_string();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                add_log("CORE-STDOUT", &format!("[{}] {}", prefix, line));
            }
        });
    }
}

// Читаем stderr от core в отдельном потоке
fn spawn_error_reader(child_stderr: Option<std::process::ChildStderr>, prefix: &str) {
    if let Some(stderr) = child_stderr {
        let prefix = prefix.to_string();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().flatten() {
                add_log("CORE-STDERR", &format!("[{}] {}", prefix, line));
            }
        });
    }
}

#[tauri::command]
async fn connect_to_server(app: tauri::AppHandle, server: Server) -> Result<String, String> {
    add_log("INFO", &format!("Connecting to server: {} ({}:{})", server.name, server.address, server.port));
    
    // 1. Останавливаем предыдущее подключение
    {
        let mut g = CORE_PROCESS.lock().unwrap();
        if let Some(mut child) = g.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    kill_core();
    kill_all_xray();
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // 2. Записываем конфиг
    let config_json = serde_json::to_string(&server)
        .map_err(|e| format!("Serialize error: {}", e))?;
    let config_path = std::env::temp_dir().join("max-strike-config.json");
    std::fs::write(&config_path, &config_json)
        .map_err(|e| format!("Write config error: {}", e))?;
    add_log("DEBUG", &format!("Config written to: {:?}", config_path));
    
    // 3. Записываем routing
    let routing = ROUTING_CONFIG.lock().unwrap().clone();
    let routing_json = serde_json::to_string(&routing)
        .map_err(|e| format!("Serialize routing error: {}", e))?;
    let routing_path = std::env::temp_dir().join("max-strike-routing.json");
    std::fs::write(&routing_path, &routing_json)
        .map_err(|e| format!("Write routing error: {}", e))?;
    
    // 4. Ищем бинарники
    let resource_dir = app.path().resource_dir().ok();
    let core_path = find_core_path(resource_dir.as_ref());
    let xray_path = find_xray(resource_dir.as_ref());
    
    let core_path_normalized = normalize_path(std::path::Path::new(&core_path));
    let xray_path_normalized = normalize_path(std::path::Path::new(&xray_path));
    
    add_log("INFO", &format!("Starting core: {:?}", core_path_normalized));
    add_log("INFO", &format!("Using xray: {:?}", xray_path_normalized));
    
    // 5. Проверяем что файлы существуют
    if !core_path_normalized.exists() {
        let err = format!("Core binary not found: {:?}", core_path_normalized);
        add_log("ERROR", &err);
        return Err(err);
    }
    if !xray_path_normalized.exists() {
        let err = format!("Xray binary not found: {:?}", xray_path_normalized);
        add_log("ERROR", &err);
        return Err(err);
    }
    
    // 6. Запускаем core с перенаправленными stdout/stderr
    let mut child = Command::new(&core_path_normalized)
        .env("XRAY_PATH", xray_path_normalized.to_string_lossy().as_ref())
        .env("ROUTING_CONFIG", routing_path.to_string_lossy().as_ref())
        .arg("connect")
        .arg(config_path.to_string_lossy().as_ref())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| {
            let err = format!("Failed to start core: {} (path: {:?})", e, core_path_normalized);
            add_log("ERROR", &err);
            err
        })?;
    
    let pid = child.id();
    add_log("INFO", &format!("Core process started with PID: {}", pid));
    
    // 7. Запускаем потоки для чтения stdout/stderr (забираем из child)
    spawn_output_reader(child.stdout.take(), "CORE");
    spawn_error_reader(child.stderr.take(), "CORE-ERR");
    
    // 8. СНАЧАЛА сохраняем PID
    {
        if let Ok(mut pid_lock) = CORE_PID.lock() {
            *pid_lock = Some(pid);
        }
    }
    
    // 9. ПОТОМ сохраняем child в CORE_PROCESS
    {
        if let Ok(mut g) = CORE_PROCESS.lock() {
            *g = Some(child);
        }
    }
    
    // 10. Проверяем что процесс не упал сразу (теперь child уже в CORE_PROCESS!)
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    {
        if let Ok(mut g) = CORE_PROCESS.lock() {
            if let Some(ref mut child) = *g {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        add_log("ERROR", &format!("Core exited immediately: {}", status));
                        return Err(format!("Core process exited: {}", status));
                    }
                    Ok(None) => {
                        add_log("DEBUG", "Core process is running");
                    }
                    Err(e) => {
                        add_log("WARN", &format!("Failed to check process: {}", e));
                    }
                }
            }
        }
    }
    
    // 11. Ждём пока xray начнёт слушать порт (до 15 секунд)
    add_log("INFO", "Waiting for xray to start listening...");
    let mut xray_ready = false;
    for i in 1..=30 {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Проверяем что процесс жив
        let process_alive = {
            if let Ok(mut g) = CORE_PROCESS.lock() {
                if let Some(ref mut child) = *g {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            add_log("ERROR", &format!("Core exited with: {}", status));
                            false
                        }
                        Ok(None) => true,
                        Err(_) => false,
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };
        
        if !process_alive {
            return Err("Core process died during startup. Check CORE-STDERR logs.".to_string());
        }
        
        // Проверяем порт
        if is_port_listening(10808) || is_port_listening(10809) {
            add_log("INFO", &format!("Xray is listening on ports (attempt {}/30)", i));
            xray_ready = true;
            break;
        }
        
        if i % 4 == 0 {
            add_log("DEBUG", &format!("Still waiting... attempt {}/30", i));
        }
    }
    
    // КРИТИЧНО: если xray не запустился — НЕ устанавливаем прокси!
    if !xray_ready {
        add_log("ERROR", "Xray failed to start listening within 15 seconds");
        add_log("ERROR", "Check CORE-STDERR logs for xray errors");
        disconnect_internal();
        return Err("Xray failed to start. Check CORE-STDERR logs in application.".to_string());
    }
    
    add_log("INFO", "Connection established successfully");
    Ok("Connected successfully\nHTTP proxy: 127.0.0.1:10809".to_string())
}

fn disconnect_internal() {
    // Сначала убиваем через PID (дерево процессов)
    kill_core();
    
    // Потом забираем child из CORE_PROCESS
    if let Ok(mut g) = CORE_PROCESS.lock() {
        if let Some(mut child) = g.take() {
            let _ = child.kill();
            let _ = child.wait();
            add_log("DEBUG", "Child process terminated");
        }
    }
}

fn find_core_path(resource_dir: Option<&std::path::PathBuf>) -> String {
    add_log("DEBUG", &format!("find_core_path: resource_dir={:?}", resource_dir));
    
    let mut candidates = Vec::new();
    
    if let Some(dir) = resource_dir {
        let dir = normalize_path(dir);
        candidates.push(dir.join("max-strike-core.exe"));
        candidates.push(dir.join("core").join("max-strike-core.exe"));
        candidates.push(dir.join("_up_").join("core").join("max-strike-core.exe"));
    }
    
    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = normalize_path(exe.parent().unwrap());
        candidates.push(exe_dir.join("max-strike-core.exe"));
        candidates.push(exe_dir.join("core").join("max-strike-core.exe"));
        candidates.push(exe_dir.join("_up_").join("core").join("max-strike-core.exe"));
    }
    
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("core").join("max-strike-core.exe"));
    }
    
    for c in &candidates {
        add_log("DEBUG", &format!("Checking: {:?}", c));
        if c.exists() {
            add_log("INFO", &format!("Found core at: {:?}", c));
            return c.to_string_lossy().to_string();
        }
    }
    
    add_log("ERROR", "Core not found anywhere!");
    candidates.first()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "max-strike-core.exe".to_string())
}

fn find_xray(resource_dir: Option<&std::path::PathBuf>) -> String {
    add_log("DEBUG", &format!("find_xray: resource_dir={:?}", resource_dir));
    
    let mut candidates = Vec::new();
    
    if let Some(dir) = resource_dir {
        let dir = normalize_path(dir);
        candidates.push(dir.join("xray.exe"));
        candidates.push(dir.join("core").join("xray.exe"));
        candidates.push(dir.join("_up_").join("core").join("xray.exe"));
    }
    
    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = normalize_path(exe.parent().unwrap());
        candidates.push(exe_dir.join("xray.exe"));
        candidates.push(exe_dir.join("core").join("xray.exe"));
        candidates.push(exe_dir.join("_up_").join("core").join("xray.exe"));
    }
    
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("core").join("xray.exe"));
    }
    
    for c in &candidates {
        add_log("DEBUG", &format!("Checking: {:?}", c));
        if c.exists() {
            add_log("INFO", &format!("Found xray at: {:?}", c));
            return c.to_string_lossy().to_string();
        }
    }
    
    add_log("ERROR", "Xray not found anywhere!");
    candidates.first()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "xray.exe".to_string())
}

#[tauri::command]
async fn disconnect_from_server() -> Result<String, String> {
    add_log("INFO", "Disconnecting from server");
    disconnect_internal();
    add_log("INFO", "Disconnected successfully");
    Ok("Disconnected".to_string())
}

#[tauri::command]
async fn set_system_proxy(enabled: bool) -> Result<String, String> {
    add_log("INFO", &format!("Setting system proxy: {}", if enabled { "enabled" } else { "disabled" }));
    
    let ps_script = if enabled {
        r#"
$regPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
Set-ItemProperty -Path $regPath -Name ProxyEnable -Value 1 -Type DWord
Set-ItemProperty -Path $regPath -Name ProxyServer -Value '127.0.0.1:10809'
Set-ItemProperty -Path $regPath -Name ProxyOverride -Value 'localhost;127.*;10.*;172.16.*;192.168.*;169.254.*;<local>'
$signature = @'
[DllImport("wininet.dll", SetLastError=true)]
public static extern bool InternetSetOption(IntPtr hInternet, int dwOption, IntPtr lpBuffer, int lpdwBufferLength);
'@
$WinInet = Add-Type -MemberDefinition $signature -Name WinInet -Namespace Proxy -PassThru
$WinInet::InternetSetOption([IntPtr]::Zero, 39, [IntPtr]::Zero, 0)
$WinInet::InternetSetOption([IntPtr]::Zero, 75, [IntPtr]::Zero, 0)
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
$WinInet::InternetSetOption([IntPtr]::Zero, 39, [IntPtr]::Zero, 0)
$WinInet::InternetSetOption([IntPtr]::Zero, 75, [IntPtr]::Zero, 0)
Write-Output "Proxy disabled"
"#
    };
    
    let result = Command::new("powershell")
        .args(&["-ExecutionPolicy", "Bypass", "-Command", ps_script])
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("PowerShell error: {}", e))?;
    
    let stdout = String::from_utf8_lossy(&result.stdout);
    let stderr = String::from_utf8_lossy(&result.stderr);
    
    add_log("DEBUG", &format!("PowerShell stdout: {}", stdout.trim()));
    if !stderr.is_empty() {
        add_log("WARN", &format!("PowerShell stderr: {}", stderr.trim()));
    }
    
    if !result.status.success() {
        return Err(format!("PowerShell failed: {}", stderr));
    }
    
    add_log("INFO", &format!("Windows system proxy {}", if enabled { "enabled" } else { "disabled" }));
    Ok("System proxy updated".to_string())
}

#[tauri::command]
async fn get_connection_stats() -> Result<ConnectionStats, String> {
    Ok(ConnectionStats { uplink: 0, downlink: 0 })
}

#[tauri::command]
fn get_home_dir() -> Result<String, String> {
    std::env::var("USERPROFILE").map_err(|_| "Failed to get home".to_string())
}

#[tauri::command]
fn get_install_dir() -> Result<String, String> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return Ok(normalize_path(dir).to_string_lossy().to_string());
        }
    }
    std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("Error: {}", e))
}

#[tauri::command]
fn get_logs() -> Vec<LogEntry> {
    LOG_BUFFER.lock().map(|l| l.clone()).unwrap_or_default()
}

#[tauri::command]
fn clear_logs() {
    if let Ok(mut logs) = LOG_BUFFER.lock() {
        logs.clear();
    }
    add_log("INFO", "Logs cleared");
}

#[tauri::command]
async fn get_routing_config() -> Result<RoutingConfig, String> {
    let config = load_routing_config();
    if let Ok(mut current) = ROUTING_CONFIG.lock() {
        *current = config.clone();
    }
    Ok(config)
}

#[tauri::command]
async fn save_routing_config(config: RoutingConfig) -> Result<String, String> {
    add_log("INFO", &format!("Saving routing config: block_ads={}, bypass_lan={}, split_routing={}, region={}", 
        config.block_ads, config.bypass_lan, config.split_routing, config.region));
    
    if let Ok(mut current) = ROUTING_CONFIG.lock() {
        *current = config.clone();
    }
    
    save_routing_config_to_file(&config)?;
    add_log("INFO", "Routing config saved");
    Ok("Saved".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    add_log("INFO", "MAX STRIKE application started (Windows)");
    
    {
        if let Ok(mut current) = ROUTING_CONFIG.lock() {
            *current = load_routing_config();
        }
    }
    add_log("INFO", "Routing config loaded");
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let windows = handle.webview_windows();
            for (_, window) in windows {
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Destroyed = event {
                        add_log("INFO", "Window destroyed, cleaning up...");
                        disconnect_internal();
                    }
                });
            }
            Ok(())
        })
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