use crate::data::AppState;
use chrono;
use std::path::{Path, PathBuf};
use url::Url;

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

// ── Paths ─────────────────────────────────────────────────────────────────────

/// Dossier racine des données. Sur desktop : oxyshop_config/ à côté de l'exe
/// sauf si data_dir est renseigné.
fn base_dir(cfg: &DavConfig) -> PathBuf {
    #[cfg(target_os = "android")]
    {
        if let Some(p) = ANDROID_DATA_DIR.get() {
            return p.clone();
        }
        return PathBuf::from(".");
    }
    #[cfg(not(target_os = "android"))]
    {
        if !cfg.data_dir.trim().is_empty() {
            return PathBuf::from(cfg.data_dir.trim());
        }
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        exe.parent()
            .unwrap_or(std::path::Path::new("."))
            .join("oxyshop_config")
    }
}

fn backup_dir(cfg: &DavConfig) -> PathBuf {
    base_dir(cfg).join("backup")
}

fn data_path(cfg: &DavConfig) -> PathBuf {
    base_dir(cfg).join("oxyshop.json")
}

fn config_path() -> PathBuf {
    #[cfg(target_os = "android")]
    {
        local_dir().join("config.json")
    }
    #[cfg(not(target_os = "android"))]
    {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        exe.parent()
            .unwrap_or(std::path::Path::new("."))
            .join("oxyshop_config")
            .join("config.json")
    }
}

fn marker_filename() -> &'static str {
    "oxyshop.sync.json"
}

// ── Local JSON ────────────────────────────────────────────────────────────────

pub fn load_local(cfg: &DavConfig) -> Option<AppState> {
    let path = data_path(cfg);
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save_local(cfg: &DavConfig, state: &AppState) -> std::io::Result<()> {
    if !cfg.backup_local {
        return Ok(());
    }
    let path = data_path(cfg);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(state)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(&path, text)
}

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DavConfig {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub pass: String,
    #[serde(default)]
    pub url2: String,
    #[serde(default)]
    pub user2: String,
    #[serde(default)]
    pub pass2: String,
    #[serde(default)]
    pub dav2_enabled: bool,
    #[serde(default)]
    pub data_dir: String,
    #[serde(default = "default_true")]
    pub backup_local: bool,
    #[serde(default = "default_true")]
    pub backup_webdav: bool,
}

impl Default for DavConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            user: String::new(),
            pass: String::new(),
            url2: String::new(),
            user2: String::new(),
            pass2: String::new(),
            dav2_enabled: false,
            data_dir: String::new(),
            backup_local: true,
            backup_webdav: true,
        }
    }
}

fn default_true() -> bool {
    true
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

    pub fn backup_url(&self, ts: &str) -> String {
        let base = if self.url.ends_with('/') { self.url.clone() } else { format!("{}/", self.url) };
        format!("{}backup/oxyshop_{}.json", base, ts)
    }

    pub fn backup_url2(&self, ts: &str) -> String {
        let base = if self.url2.ends_with('/') { self.url2.clone() } else { format!("{}/", self.url2) };
        format!("{}backup/oxyshop_{}.json", base, ts)
    }

    pub fn data_dir_display(&self) -> String {
        if self.data_dir.trim().is_empty() {
            base_dir(self).to_string_lossy().to_string()
        } else {
            self.data_dir.clone()
        }
    }

    pub fn backup_dir_display(&self) -> String {
        backup_dir(self).to_string_lossy().to_string()
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
    // L'absence du fichier n'est pas une erreur : "pas de config" est un état valide.
    match std::fs::remove_file(config_path()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

// ── Validation des URLs ──────────────────────────────────────────────────────
//
// Toute URL WebDAV renseignée par l'utilisateur est validée avant d'être
// utilisée pour construire une requête réseau : on vérifie qu'elle est
// syntaxiquement correcte et qu'elle utilise un schéma http/https. Cela évite
// les requêtes malformées ou les tentatives d'exploitation via une URL
// bricolée (ex: "ht tp://...", "file:///etc/passwd", espaces, etc.).
pub fn validate_dav_url(raw: &str) -> Result<Url, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("URL WebDAV vide".to_string());
    }
    let url = Url::parse(trimmed).map_err(|e| format!("URL invalide : {}", e))?;
    match url.scheme() {
        "http" | "https" => Ok(url),
        other => Err(format!(
            "URL invalide : schéma '{}' non autorisé (http/https requis)",
            other
        )),
    }
}

// ── Backup ────────────────────────────────────────────────────────────────────

fn ts_filename() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H%M%S").to_string()
}

/// Backup local avec rotation 30 fichiers max.
pub fn backup_local_fn(cfg: &DavConfig, state: &AppState) {
    if !cfg.backup_local {
        return;
    }
    let dir = backup_dir(cfg);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let ts = ts_filename();
    let fname = format!("oxyshop_{}.json", ts);
    let text = match serde_json::to_string_pretty(state) {
        Ok(t) => t,
        Err(_) => return,
    };
    let _ = std::fs::write(dir.join(&fname), &text);
    // Purge : garde les 30 plus récents
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("oxyshop_"))
            .map(|e| e.path())
            .collect();
        files.sort();
        if files.len() > 30 {
            for old in &files[..files.len() - 30] {
                let _ = std::fs::remove_file(old);
            }
        }
    }
}

fn backup_dav_upload(client: &reqwest::blocking::Client, url: &str, user: &str, pass: &str, body: &str) {
    let _ = client
        .put(url)
        .basic_auth(user, Some(pass))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send();
}

/// Backup complet à la fermeture (local + WebDAV).
pub fn backup_on_exit(cfg: &DavConfig, state: &AppState) {
    backup_local_fn(cfg, state);
    if cfg.backup_webdav && (cfg.is_complete() || cfg.has_dav2()) {
        let cfg2 = cfg.clone();
        let body = match serde_json::to_string_pretty(state) {
            Ok(t) => t,
            Err(_) => return,
        };
        std::thread::spawn(move || {
            let ts = ts_filename();
            if let Ok(client) = make_client() {
                if cfg2.is_complete() && validate_dav_url(&cfg2.url).is_ok() {
                    backup_dav_upload(&client, &cfg2.backup_url(&ts), &cfg2.user, &cfg2.pass, &body);
                }
                if cfg2.has_dav2() && validate_dav_url(&cfg2.url2).is_ok() {
                    backup_dav_upload(&client, &cfg2.backup_url2(&ts), &cfg2.user2, &cfg2.pass2, &body);
                }
            }
        });
    }
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
    let text = match resp.text() { Ok(t) => t, Err(_) => return 0 };
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v["ts"].as_u64())
        .unwrap_or(0)
}

fn write_marker(client: &reqwest::blocking::Client, url: &str, user: &str, pass: &str, ts: u64) {
    let body = format!("{{\"ts\":{},\"app\":\"Oxyshop\"}}", ts);
    let _ = client.put(url).basic_auth(user, Some(pass))
        .header("Content-Type", "application/json")
        .body(body).send();
}

// ── WebDAV ────────────────────────────────────────────────────────────────────

pub fn dav_load(cfg: &DavConfig) -> Result<AppState, String> {
    let client = make_client()?;

    let dav1_ok = cfg.is_complete() && validate_dav_url(&cfg.url).is_ok();
    let dav2_ok = cfg.has_dav2() && validate_dav_url(&cfg.url2).is_ok();

    let ts1 = if dav1_ok { read_marker(&client, &cfg.marker_url(), &cfg.user, &cfg.pass) } else { 0 };
    let ts2 = if dav2_ok { read_marker(&client, &cfg.marker_url2(), &cfg.user2, &cfg.pass2) } else { 0 };

    let mut sources: Vec<(String, String, String)> = Vec::new();
    if ts2 > ts1 && dav2_ok {
        sources.push((cfg.file_url2(), cfg.user2.clone(), cfg.pass2.clone()));
        if dav1_ok { sources.push((cfg.file_url(), cfg.user.clone(), cfg.pass.clone())); }
    } else {
        if dav1_ok { sources.push((cfg.file_url(), cfg.user.clone(), cfg.pass.clone())); }
        if dav2_ok { sources.push((cfg.file_url2(), cfg.user2.clone(), cfg.pass2.clone())); }
    }

    let mut last_err = String::from("aucune source WebDAV valide configurée");
    for (url, user, pass) in &sources {
        let resp = match client.get(url).basic_auth(user, Some(pass)).send() {
            Ok(r) => r, Err(e) => { last_err = e.to_string(); continue; }
        };
        if !resp.status().is_success() { last_err = format!("HTTP {}", resp.status()); continue; }
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

    if cfg.is_complete() {
        match validate_dav_url(&cfg.url) {
            Ok(_) => {
                let resp = client.put(&cfg.file_url())
                    .basic_auth(&cfg.user, Some(&cfg.pass))
                    .header("Content-Type", "application/json")
                    .body(body.clone()).send()
                    .map_err(|e| e.to_string())?;
                let status = resp.status().as_u16();
                if matches!(status, 200 | 201 | 204) {
                    write_marker(&client, &cfg.marker_url(), &cfg.user, &cfg.pass, ts);
                } else {
                    last_err = format!("DAV1 HTTP {}", status);
                }
            }
            Err(e) => last_err = format!("DAV1 {}", e),
        }
    }

    if cfg.has_dav2() {
        match validate_dav_url(&cfg.url2) {
            Ok(_) => match client.put(&cfg.file_url2())
                .basic_auth(&cfg.user2, Some(&cfg.pass2))
                .header("Content-Type", "application/json")
                .body(body.clone()).send()
            {
                Ok(r) if matches!(r.status().as_u16(), 200 | 201 | 204) => {
                    write_marker(&client, &cfg.marker_url2(), &cfg.user2, &cfg.pass2, ts);
                }
                Ok(r)  => { last_err = format!("DAV2 HTTP {}", r.status()); }
                Err(e) => { last_err = format!("DAV2 {}", e); }
            },
            Err(e) => last_err = format!("DAV2 {}", e),
        }
    }

    if last_err.is_empty() { Ok(()) } else { Err(last_err) }
}

pub fn dav_test(cfg: &DavConfig) -> Result<(), String> {
    validate_dav_url(&cfg.url)?;
    let client = make_client()?;
    let resp = client.head(&cfg.file_url())
        .basic_auth(&cfg.user, Some(&cfg.pass))
        .send().map_err(|e| e.to_string())?;
    let s = resp.status().as_u16();
    if s < 500 { Ok(()) } else { Err(format!("HTTP {}", s)) }
}

pub fn dav_test2(cfg: &DavConfig) -> Result<(), String> {
    if !cfg.has_dav2() { return Err("WebDAV2 non configuré".into()); }
    validate_dav_url(&cfg.url2)?;
    let client = make_client()?;
    let resp = client.head(&cfg.file_url2())
        .basic_auth(&cfg.user2, Some(&cfg.pass2))
        .send().map_err(|e| e.to_string())?;
    let s = resp.status().as_u16();
    if s < 500 { Ok(()) } else { Err(format!("HTTP {}", s)) }
}

// ── Export / Import ───────────────────────────────────────────────────────────

/// Chemin par défaut si l'utilisateur n'en choisit pas un explicitement
/// (utilisé aussi sur Android, où il n'y a pas de sélecteur de fichier natif).
pub fn default_export_path() -> PathBuf {
    #[cfg(target_os = "android")]
    { local_dir().join("oxyshop.json") }
    #[cfg(not(target_os = "android"))]
    {
        dirs::download_dir().or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("oxyshop.json")
    }
}

/// Exporte vers le chemin par défaut (comportement historique / Android).
pub fn export_json(_cfg: &DavConfig, state: &AppState) -> Result<PathBuf, String> {
    export_json_to(&default_export_path(), state)
}

/// Exporte vers un chemin explicite, choisi par l'utilisateur via la boîte
/// de dialogue "Enregistrer sous" (desktop).
pub fn export_json_to(dest: &Path, state: &AppState) -> Result<PathBuf, String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(dest, text).map_err(|e| e.to_string())?;
    Ok(dest.to_path_buf())
}

pub fn import_json(path: &str) -> Result<AppState, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}
