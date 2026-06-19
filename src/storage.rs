use crate::data::AppState;
use std::path::PathBuf;

#[cfg(target_os = "android")]
static ANDROID_DATA_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
pub fn set_android_data_dir(path: PathBuf) {
    let _ = ANDROID_DATA_DIR.set(path);
}

pub fn android_data_dir() -> PathBuf {
    local_dir()
}

fn local_dir() -> PathBuf {
    #[cfg(target_os = "android")]
    if let Some(p) = ANDROID_DATA_DIR.get() {
        return p.clone();
    }
    dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("oxyshop")
}

fn data_path() -> PathBuf {
    local_dir().join("oxyshop.json")
}

fn config_path() -> PathBuf {
    local_dir().join("config.json")
}

fn marker_filename() -> &'static str {
    "oxyshop.sync.json"
}

// ── Local JSON ────────────────────────────────────────────────────────────────

pub fn load_local() -> Option<AppState> {
    let path = data_path();
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_local(state: &AppState) -> std::io::Result<()> {
    let path = data_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(&path, text)
}

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct DavConfig {
    pub url:  String,
    pub user: String,
    pub pass: String,
    // WebDAV secondaire (optionnel)
    #[serde(default)]
    pub url2:     String,
    #[serde(default)]
    pub user2:    String,
    #[serde(default)]
    pub pass2:    String,
    #[serde(default)]
    pub dav2_enabled: bool,
}

impl DavConfig {
    pub fn is_complete(&self) -> bool {
        !self.url.is_empty() && !self.user.is_empty() && !self.pass.is_empty()
    }

    pub fn has_dav2(&self) -> bool {
        self.dav2_enabled
            && !self.url2.is_empty()
            && !self.user2.is_empty()
            && !self.pass2.is_empty()
    }

    pub fn file_url(&self) -> String {
        let base = if self.url.ends_with('/') { self.url.clone() } else { format!("{}/", self.url) };
        format!("{}oxyshop.json", base)
    }

    pub fn file_url2(&self) -> String {
        let base = if self.url2.ends_with('/') { self.url2.clone() } else { format!("{}/", self.url2) };
        format!("{}oxyshop.json", base)
    }

    pub fn marker_url(&self) -> String {
        let base = if self.url.ends_with('/') { self.url.clone() } else { format!("{}/", self.url) };
        format!("{}{}", base, marker_filename())
    }

    pub fn marker_url2(&self) -> String {
        let base = if self.url2.ends_with('/') { self.url2.clone() } else { format!("{}/", self.url2) };
        format!("{}{}", base, marker_filename())
    }
}

pub fn load_config() -> DavConfig {
    let path = config_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_config(cfg: &DavConfig) -> std::io::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(&path, text)
}

pub fn clear_config() -> std::io::Result<()> {
    let _ = std::fs::remove_file(config_path());
    Ok(())
}

// ── HTTP client ───────────────────────────────────────────────────────────────

pub fn make_client() -> Result<reqwest::blocking::Client, String> {
    let builder = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10));

    #[cfg(target_os = "android")]
    let builder = {
        let root_store = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        let tls = rustls::ClientConfig::builder_with_provider(
            std::sync::Arc::new(rustls::crypto::aws_lc_rs::default_provider()),
        )
        .with_safe_default_protocol_versions()
        .map_err(|e| format!("tls: {}", e))?
        .with_root_certificates(root_store)
        .with_no_client_auth();
        builder.use_preconfigured_tls(tls)
    };

    builder.build().map_err(|e| format!("client: {}", e))
}

// ── Marqueur de sync ──────────────────────────────────────────────────────────

fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn read_marker(client: &reqwest::blocking::Client, url: &str, user: &str, pass: &str) -> u64 {
    let resp = match client.get(url).basic_auth(user, Some(pass)).send() {
        Ok(r) if r.status().is_success() => r,
        _ => return 0,
    };
    let text = match resp.text() {
        Ok(t) => t,
        Err(_) => return 0,
    };
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v["ts"].as_u64())
        .unwrap_or(0)
}

fn write_marker(client: &reqwest::blocking::Client, url: &str, user: &str, pass: &str, ts: u64) {
    let body = format!("{{\"ts\":{},\"app\":\"Oxyshop\"}}", ts);
    let _ = client
        .put(url)
        .basic_auth(user, Some(pass))
        .header("Content-Type", "application/json")
        .body(body)
        .send();
}

// ── WebDAV (blocking — must be called from a background thread) ───────────────

pub fn dav_load(cfg: &DavConfig) -> Result<AppState, String> {
    let client = make_client()?;

    // Lire les marqueurs pour choisir la source la plus récente
    let ts1 = if cfg.is_complete() { read_marker(&client, &cfg.marker_url(), &cfg.user, &cfg.pass) } else { 0 };
    let ts2 = if cfg.has_dav2()    { read_marker(&client, &cfg.marker_url2(), &cfg.user2, &cfg.pass2) } else { 0 };

    // Ordonner : source la plus récente en premier, stocker des String owned
    let sources_owned: Vec<(String, String, String)> = if ts2 > ts1 && cfg.has_dav2() {
        let mut v = vec![];
        if cfg.has_dav2()    { v.push((cfg.file_url2(), cfg.user2.clone(), cfg.pass2.clone())); }
        if cfg.is_complete() { v.push((cfg.file_url(),  cfg.user.clone(),  cfg.pass.clone())); }
        v
    } else {
        let mut v = vec![];
        if cfg.is_complete() { v.push((cfg.file_url(),  cfg.user.clone(),  cfg.pass.clone())); }
        if cfg.has_dav2()    { v.push((cfg.file_url2(), cfg.user2.clone(), cfg.pass2.clone())); }
        v
    };

    let mut last_err = String::from("no source configured");
    for (url, user, pass) in &sources_owned {
        let resp = match client.get(url).basic_auth(user, Some(pass)).send() {
            Ok(r) => r,
            Err(e) => { last_err = e.to_string(); continue; }
        };
        if !resp.status().is_success() {
            last_err = format!("HTTP {}", resp.status());
            continue;
        }
        match resp.json::<AppState>() {
            Ok(state) => return Ok(state),
            Err(e) => { last_err = e.to_string(); continue; }
        }
    }
    Err(last_err)
}

pub fn dav_save(cfg: &DavConfig, state: &AppState) -> Result<(), String> {
    let body = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    let client = make_client()?;
    let ts = now_ts();
    let mut last_err = String::new();

    // Push DAV1
    if cfg.is_complete() {
        let resp = client
            .put(&cfg.file_url())
            .basic_auth(&cfg.user, Some(&cfg.pass))
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send()
            .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        if status == 200 || status == 201 || status == 204 {
            write_marker(&client, &cfg.marker_url(), &cfg.user, &cfg.pass, ts);
        } else {
            last_err = format!("DAV1 HTTP {}", status);
        }
    }

    // Push DAV2
    if cfg.has_dav2() {
        let resp = client
            .put(&cfg.file_url2())
            .basic_auth(&cfg.user2, Some(&cfg.pass2))
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send();
        match resp {
            Ok(r) if r.status().as_u16() == 200 || r.status().as_u16() == 201 || r.status().as_u16() == 204 => {
                write_marker(&client, &cfg.marker_url2(), &cfg.user2, &cfg.pass2, ts);
            }
            Ok(r) => { last_err = format!("DAV2 HTTP {}", r.status()); }
            Err(e) => { last_err = format!("DAV2 {}", e); }
        }
    }

    if last_err.is_empty() { Ok(()) } else { Err(last_err) }
}

pub fn dav_test(cfg: &DavConfig) -> Result<(), String> {
    let client = make_client()?;
    let resp = client
        .head(&cfg.file_url())
        .basic_auth(&cfg.user, Some(&cfg.pass))
        .send()
        .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    if status < 500 { Ok(()) } else { Err(format!("HTTP {}", status)) }
}

pub fn dav_test2(cfg: &DavConfig) -> Result<(), String> {
    if !cfg.has_dav2() {
        return Err("WebDAV2 non configuré".into());
    }
    let client = make_client()?;
    let resp = client
        .head(&cfg.file_url2())
        .basic_auth(&cfg.user2, Some(&cfg.pass2))
        .send()
        .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    if status < 500 { Ok(()) } else { Err(format!("HTTP {}", status)) }
}

// ── Export / Import ───────────────────────────────────────────────────────────

pub fn export_json(state: &AppState) -> Result<PathBuf, String> {
    // Nom aligné sur le fichier WebDAV pour compatibilité directe
    let filename = "oxyshop.json".to_string();
    let dest = {
        #[cfg(target_os = "android")]
        {
            local_dir().join(&filename)
        }
        #[cfg(not(target_os = "android"))]
        {
            dirs::download_dir()
                .or_else(dirs::home_dir)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(&filename)
        }
    };
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&dest, text).map_err(|e| e.to_string())?;
    Ok(dest)
}

pub fn import_json(path: &str) -> Result<AppState, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}
