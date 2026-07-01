//! Callbacks Slint pour la configuration (WebDAV 1/2, backup, export/import
//! des données depuis l'écran Config, langue, fermeture de l'application).

use crate::state::{lock_app, App};
use crate::storage::{
    backup_on_exit, clear_config, dav_test, dav_test2, default_export_path, export_json_to,
    import_json, save_config as save_cfg, DavConfig,
};
use crate::{AppWindow, RefreshFn, ToastFn};
use slint::ComponentHandle;
use std::sync::{Arc, Mutex};

pub fn setup(
    ui: &AppWindow,
    app_state: &Arc<Mutex<App>>,
    rs: &RefreshFn,
    rc: &RefreshFn,
    rsync: &RefreshFn,
    toast: &ToastFn,
) {
    // ── Thème / langue ───────────────────────────────────────────────────

    ui.on_lang_toggle({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rs = rs.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let new_lang = !ui.get_lang_en();
            ui.set_lang_en(new_lang);
            lock_app(&app).lang_en = new_lang;
            rs();
        }
    });

    // ── Config WebDAV 1 ──────────────────────────────────────────────────

    ui.on_cfg_save({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rsync = rsync.clone();
        let toast = toast.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let lang;
            {
                let mut st = lock_app(&app);
                st.config.url = ui.get_cfg_url().to_string();
                st.config.user = ui.get_cfg_user().to_string();
                st.config.pass = ui.get_cfg_pass().to_string();
                if let Err(e) = save_cfg(&st.config) {
                    eprintln!("Écriture config échouée : {e}");
                }
                lang = st.lang_en;
            }
            toast(if lang { "✅ Config saved" } else { "✅ Config sauvée" });
            rsync();
        }
    });

    ui.on_cfg_test({
        let app = app_state.clone();
        let toast = toast.clone();
        let rsync = rsync.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            {
                let mut st = lock_app(&app);
                st.config.url = ui.get_cfg_url().to_string();
                st.config.user = ui.get_cfg_user().to_string();
                st.config.pass = ui.get_cfg_pass().to_string();
                if let Err(e) = save_cfg(&st.config) {
                    eprintln!("Écriture config échouée : {e}");
                }
            }
            let (cfg, lang) = {
                let st = lock_app(&app);
                (st.config.clone(), st.lang_en)
            };
            if !cfg.is_complete() {
                toast(if lang { "⚠️ Fill all 3 fields" } else { "⚠️ Remplis les 3 champs" });
                return;
            }
            let app2 = app.clone();
            let rsync2 = rsync.clone();
            let ui_weak2 = ui_weak.clone();
            std::thread::spawn(move || {
                let result = dav_test(&cfg);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak2.upgrade() else { return };
                    match result {
                        Ok(()) => {
                            lock_app(&app2).dav_ok = true;
                            rsync2();
                            ui.set_toast_msg("✅ Connexion OK !".into());
                            ui.set_toast_show(true);
                            let ui_w = ui_weak2.clone();
                            slint::Timer::single_shot(std::time::Duration::from_millis(2000), move || {
                                if let Some(u) = ui_w.upgrade() { u.set_toast_show(false); }
                            });
                        }
                        Err(e) => {
                            ui.set_toast_msg(format!("⚠️ {}", e).into());
                            ui.set_toast_show(true);
                            let ui_w = ui_weak2.clone();
                            slint::Timer::single_shot(std::time::Duration::from_millis(3000), move || {
                                if let Some(u) = ui_w.upgrade() { u.set_toast_show(false); }
                            });
                        }
                    }
                });
            });
        }
    });

    ui.on_cfg_clear({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rsync = rsync.clone();
        let toast = toast.clone();
        move || {
            if let Err(e) = clear_config() {
                eprintln!("Suppression config échouée : {e}");
            }
            let lang = {
                let mut st = lock_app(&app);
                st.config = DavConfig::default();
                st.dav_ok = false;
                st.lang_en
            };
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.set_cfg_url("".into());
            ui.set_cfg_user("".into());
            ui.set_cfg_pass("".into());
            toast(if lang { "🗑️ Config cleared" } else { "🗑️ Config effacée" });
            rsync();
        }
    });

    // ── Config WebDAV 2 ──────────────────────────────────────────────────

    ui.on_cfg2_save({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rsync = rsync.clone();
        let toast = toast.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let lang;
            {
                let mut st = lock_app(&app);
                st.config.url2 = ui.get_cfg2_url().to_string();
                st.config.user2 = ui.get_cfg2_user().to_string();
                st.config.pass2 = ui.get_cfg2_pass().to_string();
                st.config.dav2_enabled = ui.get_cfg2_enabled();
                if let Err(e) = save_cfg(&st.config) {
                    eprintln!("Écriture config échouée : {e}");
                }
                lang = st.lang_en;
            }
            toast(if lang { "✅ WebDAV2 saved" } else { "✅ WebDAV2 sauvé" });
            rsync();
        }
    });

    ui.on_cfg2_test({
        let app = app_state.clone();
        let toast = toast.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            {
                let mut st = lock_app(&app);
                st.config.url2 = ui.get_cfg2_url().to_string();
                st.config.user2 = ui.get_cfg2_user().to_string();
                st.config.pass2 = ui.get_cfg2_pass().to_string();
                st.config.dav2_enabled = ui.get_cfg2_enabled();
                if let Err(e) = save_cfg(&st.config) {
                    eprintln!("Écriture config échouée : {e}");
                }
            }
            let (cfg, lang) = {
                let st = lock_app(&app);
                (st.config.clone(), st.lang_en)
            };
            if !cfg.has_dav2() {
                toast(if lang { "⚠️ Fill all 3 DAV2 fields and enable" } else { "⚠️ Remplis les 3 champs DAV2 et active" });
                return;
            }
            let ui_weak2 = ui_weak.clone();
            std::thread::spawn(move || {
                let result = dav_test2(&cfg);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak2.upgrade() else { return };
                    match result {
                        Ok(()) => {
                            ui.set_toast_msg("✅ DAV2 OK !".into());
                            ui.set_toast_show(true);
                            let ui_w = ui_weak2.clone();
                            slint::Timer::single_shot(std::time::Duration::from_millis(2000), move || {
                                if let Some(u) = ui_w.upgrade() { u.set_toast_show(false); }
                            });
                        }
                        Err(e) => {
                            ui.set_toast_msg(format!("⚠️ DAV2: {}", e).into());
                            ui.set_toast_show(true);
                            let ui_w = ui_weak2.clone();
                            slint::Timer::single_shot(std::time::Duration::from_millis(3000), move || {
                                if let Some(u) = ui_w.upgrade() { u.set_toast_show(false); }
                            });
                        }
                    }
                });
            });
        }
    });

    ui.on_cfg2_clear({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rsync = rsync.clone();
        let toast = toast.clone();
        move || {
            let lang = {
                let mut st = lock_app(&app);
                st.config.url2 = String::new();
                st.config.user2 = String::new();
                st.config.pass2 = String::new();
                st.config.dav2_enabled = false;
                if let Err(e) = save_cfg(&st.config) {
                    eprintln!("Écriture config échouée : {e}");
                }
                st.lang_en
            };
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.set_cfg2_url("".into());
            ui.set_cfg2_user("".into());
            ui.set_cfg2_pass("".into());
            ui.set_cfg2_enabled(false);
            toast(if lang { "🗑️ DAV2 cleared" } else { "🗑️ DAV2 effacé" });
            rsync();
        }
    });

    // ── Export / Import des données depuis l'écran Config ───────────────

    ui.on_cfg_export({
        let app = app_state.clone();
        let toast = toast.clone();
        move || {
            let st = lock_app(&app);
            let lang = st.lang_en;

            #[cfg(not(target_os = "android"))]
            let result = {
                let default_path = default_export_path();
                let dialog = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .set_file_name(
                        default_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "oxyshop.json".to_string()),
                    );
                let dialog = match default_path.parent() {
                    Some(dir) => dialog.set_directory(dir),
                    None => dialog,
                };
                match dialog.save_file() {
                    Some(path) => export_json_to(&path, &st.state),
                    None => return, // Annulé par l'utilisateur
                }
            };

            #[cfg(target_os = "android")]
            let result = crate::storage::export_json(&st.config, &st.state);

            match result {
                Ok(p) => toast(&format!(
                    "📤 {} : {}",
                    if lang { "Exported" } else { "Exporté" },
                    p.file_name().map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| p.to_string_lossy().to_string())
                )),
                Err(e) => toast(&format!("⚠️ {}", e)),
            }
        }
    });

    ui.on_cfg_import({
        let app = app_state.clone();
        let rs = rs.clone();
        let rc = rc.clone();
        let toast = toast.clone();
        move || {
            #[cfg(not(target_os = "android"))]
            {
                let file = rfd::FileDialog::new().add_filter("JSON", &["json"]).pick_file();
                if let Some(path) = file {
                    let path_str = path.to_string_lossy().to_string();
                    match import_json(&path_str) {
                        Ok(new_state) => {
                            let count = new_state.stock.len();
                            let lang = {
                                let mut st = lock_app(&app);
                                st.state = new_state;
                                st.save();
                                st.lang_en
                            };
                            toast(&format!(
                                "📥 {} {}",
                                count,
                                if lang { "items imported" } else { "articles importés" }
                            ));
                            rs();
                            rc();
                        }
                        Err(e) => toast(&format!("⚠️ {}", e)),
                    }
                }
            }
            #[cfg(target_os = "android")]
            {
                // Sur Android : tente d'importer oxyshop-import.json depuis le dossier de données de l'app.
                let data_dir = crate::storage::android_data_dir();
                let path = data_dir.join("oxyshop-import.json");
                let path_str = path.to_string_lossy().to_string();
                match import_json(&path_str) {
                    Ok(new_state) => {
                        let count = new_state.stock.len();
                        {
                            let mut st = lock_app(&app);
                            st.state = new_state;
                            st.save();
                        }
                        toast(&format!("📥 {} articles importés", count));
                        rs();
                        rc();
                    }
                    Err(_) => toast("📥 Placer oxyshop-import.json dans le dossier app"),
                }
            }
        }
    });

    // ── Backup ────────────────────────────────────────────────────────────

    ui.on_cfg_set_backup_local({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move |val| {
            {
                let mut st = lock_app(&app);
                st.config.backup_local = val;
                if let Err(e) = save_cfg(&st.config) {
                    eprintln!("Écriture config échouée : {e}");
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_cfg_backup_local(val);
            }
        }
    });

    ui.on_cfg_set_backup_webdav({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move |val| {
            {
                let mut st = lock_app(&app);
                st.config.backup_webdav = val;
                if let Err(e) = save_cfg(&st.config) {
                    eprintln!("Écriture config échouée : {e}");
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_cfg_backup_webdav(val);
            }
        }
    });

    ui.on_cfg_set_data_dir({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let toast = toast.clone();
        move |path| {
            let (display, backup_display, lang) = {
                let mut st = lock_app(&app);
                st.config.data_dir = path.to_string();
                if let Err(e) = save_cfg(&st.config) {
                    eprintln!("Écriture config échouée : {e}");
                }
                (st.config.data_dir_display(), st.config.backup_dir_display(), st.lang_en)
            };
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.set_cfg_data_dir_display(display.into());
            ui.set_cfg_backup_dir_display(backup_display.into());
            toast(if lang { "📁 Folder saved" } else { "📁 Dossier sauvegardé" });
        }
    });

    // ── Backup à la fermeture ─────────────────────────────────────────────

    ui.window().on_close_requested({
        let app = app_state.clone();
        move || {
            let st = lock_app(&app);
            backup_on_exit(&st.config, &st.state);
            slint::CloseRequestResponse::HideWindow
        }
    });
}
