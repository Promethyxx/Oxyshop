use crate::data::AppState;
use std::path::PathBuf;

#[cfg(target_os = "android")]
static ANDROID_DATA_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
pub fn set_android_data_dir(path: PathBuf) {
    let _ = ANDROID_DATA_DIR.set(path);
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
    pub url: String,
    pub user: String,
    pub pass: String,
}

impl DavConfig {
    pub fn is_complete(&self) -> bool {
        !self.url.is_empty() && !self.user.is_empty() && !self.pass.is_empty()
    }

    pub fn file_url(&self) -> String {
        let base = if self.url.ends_with('/') { self.url.clone() } else { format!("{}/", self.url) };
        format!("{}oxyshop.json", base)
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

// ── WebDAV (blocking) ─────────────────────────────────────────────────────────

pub fn dav_load(cfg: &DavConfig) -> Result<AppState, String> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(&cfg.file_url())
        .basic_auth(&cfg.user, Some(&cfg.pass))
        .send()
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let state: AppState = resp.json().map_err(|e| e.to_string())?;
    Ok(state)
}

pub fn dav_save(cfg: &DavConfig, state: &AppState) -> Result<(), String> {
    let body = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    let client = reqwest::blocking::Client::new();
    let resp = client
        .put(&cfg.file_url())
        .basic_auth(&cfg.user, Some(&cfg.pass))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .map_err(|e| e.to_string())?;

    let status = resp.status().as_u16();
    if status == 200 || status == 201 || status == 204 {
        Ok(())
    } else {
        Err(format!("HTTP {}", status))
    }
}

pub fn dav_test(cfg: &DavConfig) -> Result<(), String> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .head(&cfg.file_url())
        .basic_auth(&cfg.user, Some(&cfg.pass))
        .send()
        .map_err(|e| e.to_string())?;
    let status = resp.status().as_u16();
    if status < 500 {
        Ok(())
    } else {
        Err(format!("HTTP {}", status))
    }
}

// ── Export / Import ───────────────────────────────────────────────────────────

pub fn export_json(state: &AppState) -> Result<PathBuf, String> {
    let date = chrono_date();
    let filename = format!("oxyshop-{}.json", date);
    // Export to user's Downloads or home
    let dest = dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(&filename);
    let text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&dest, text).map_err(|e| e.to_string())?;
    Ok(dest)
}

pub fn import_json(path: &str) -> Result<AppState, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn chrono_date() -> String {
    // simple ISO date without chrono dep
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // rough: days since epoch
    let days = secs / 86400;
    let y = 1970 + days / 365;
    let d = days % 365;
    let m = d / 30 + 1;
    let day = d % 30 + 1;
    format!("{:04}-{:02}-{:02}", y, m, day)
}
