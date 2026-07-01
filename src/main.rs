mod callbacks_config;
mod callbacks_data;
mod data;
mod state;
mod storage;

use data::AppState;
use state::{lock_app, make_categories, make_course_cats, make_meals, App};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use storage::{dav_load, load_config, load_local};

slint::include_modules!();

/// Fonction de rafraîchissement d'une vue (recalcule un modèle Slint à partir
/// de l'état applicatif courant et le pousse dans l'UI).
///
/// `Arc` (et non `Rc`) + `Send + Sync` : ces closures sont aussi appelées
/// depuis des threads d'arrière-plan (test de connexion WebDAV notamment),
/// via `std::thread::spawn` / `slint::invoke_from_event_loop`, qui exigent
/// des closures `Send`.
pub type RefreshFn = Arc<dyn Fn() + Send + Sync>;
/// Fonction d'affichage d'un toast temporaire (mêmes contraintes que ci-dessus).
pub type ToastFn = Arc<dyn Fn(&str) + Send + Sync>;

// ── Entry points ──────────────────────────────────────────────────────────────

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    if let Some(path) = app.internal_data_path() {
        storage::set_android_data_dir(path);
    }
    if let Err(e) = slint::android::init(app) {
        eprintln!("Erreur d'initialisation Android : {e}");
        return;
    }
    run();
}

#[cfg(not(target_os = "android"))]
fn main() {
    run();
}

// ── Run ───────────────────────────────────────────────────────────────────────

fn run() {
    let config = load_config();

    let (mut state, dav_ok) = if config.is_complete() || config.has_dav2() {
        match dav_load(&config) {
            Ok(s) => (s, true),
            Err(_) => (load_local(&config).unwrap_or_else(AppState::with_defaults), false),
        }
    } else {
        (load_local(&config).unwrap_or_else(AppState::with_defaults), false)
    };
    if state.meals.is_empty() {
        state.meals = data::default_meals();
    }

    let app_state = Arc::new(Mutex::new(App {
        state,
        open_cats: HashMap::new(),
        config: config.clone(),
        dav_ok,
        ctx_target: -1,
        meal_target: -1,
        lang_en: false,
    }));

    let ui = match AppWindow::new() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Impossible de créer la fenêtre de l'application : {e}");
            std::process::exit(1);
        }
    };

    #[cfg(target_os = "android")]
    ui.set_status_bar_height(48.0);

    populate_initial(&ui, &app_state);

    let refresh_stock = make_refresh_stock(&ui, &app_state);
    let refresh_courses = make_refresh_courses(&ui, &app_state);
    let refresh_meals = make_refresh_meals(&ui, &app_state);
    let refresh_sync = make_refresh_sync(&ui, &app_state);
    let toast = make_toast(&ui);

    callbacks_data::setup(&ui, &app_state, &refresh_stock, &refresh_courses, &refresh_meals, &toast);
    callbacks_config::setup(&ui, &app_state, &refresh_stock, &refresh_courses, &refresh_sync, &toast);

    if let Err(e) = ui.run() {
        eprintln!("Erreur lors de l'exécution de l'interface : {e}");
        std::process::exit(1);
    }
}

// ── Population initiale + closures partagées ────────────────────────────────

fn populate_initial(ui: &AppWindow, app_state: &Arc<Mutex<App>>) {
    let st = lock_app(app_state);
    let cats = make_categories(&st.state.stock, "", &st.open_cats);
    let (cc, ct, cd, cr) = make_course_cats(&st.state.stock, &st.state.checked, "");

    ui.set_categories(cats);
    ui.set_stat_total(st.state.stock.len() as i32);
    ui.set_stat_ok(st.state.stock.iter().filter(|i| i.qty >= i.obj).count() as i32);
    ui.set_stat_low(st.state.stock.iter().filter(|i| i.qty > 0 && i.qty < i.obj).count() as i32);
    ui.set_stat_empty(st.state.stock.iter().filter(|i| i.qty <= 0 && i.obj > 0).count() as i32);

    ui.set_course_cats(cc);
    ui.set_course_total(ct);
    ui.set_course_done(cd);
    ui.set_course_remaining(cr);
    ui.set_courses_empty(ct == 0);

    ui.set_meals(make_meals(&st.state.meals));

    ui.set_cfg_url(st.config.url.clone().into());
    ui.set_cfg_user(st.config.user.clone().into());
    ui.set_cfg_pass(st.config.pass.clone().into());
    ui.set_cfg2_url(st.config.url2.clone().into());
    ui.set_cfg2_user(st.config.user2.clone().into());
    ui.set_cfg2_pass(st.config.pass2.clone().into());
    ui.set_cfg2_enabled(st.config.dav2_enabled);

    ui.set_cfg_backup_local(st.config.backup_local);
    ui.set_cfg_backup_webdav(st.config.backup_webdav);
    ui.set_cfg_data_dir_display(st.config.data_dir_display().into());
    ui.set_cfg_backup_dir_display(st.config.backup_dir_display().into());
    ui.set_cfg_data_dir_edit(st.config.data_dir.clone().into());

    ui.set_sync_state(st.sync_state_int());
    ui.set_sync_label(st.sync_label());
}

fn make_refresh_stock(ui: &AppWindow, app_state: &Arc<Mutex<App>>) -> RefreshFn {
    let ui_weak = ui.as_weak();
    let app = app_state.clone();
    Arc::new(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        let st = lock_app(&app);
        let search = ui.get_stock_search().to_string();
        let cats = make_categories(&st.state.stock, &search, &st.open_cats);
        ui.set_categories(cats);
        ui.set_stat_total(st.state.stock.len() as i32);
        ui.set_stat_ok(st.state.stock.iter().filter(|i| i.qty >= i.obj).count() as i32);
        ui.set_stat_low(st.state.stock.iter().filter(|i| i.qty > 0 && i.qty < i.obj).count() as i32);
        ui.set_stat_empty(st.state.stock.iter().filter(|i| i.qty <= 0 && i.obj > 0).count() as i32);
    })
}

fn make_refresh_courses(ui: &AppWindow, app_state: &Arc<Mutex<App>>) -> RefreshFn {
    let ui_weak = ui.as_weak();
    let app = app_state.clone();
    Arc::new(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        let st = lock_app(&app);
        let search = ui.get_course_search().to_string();
        let (cc, ct, cd, cr) = make_course_cats(&st.state.stock, &st.state.checked, &search);
        ui.set_course_cats(cc);
        ui.set_course_total(ct);
        ui.set_course_done(cd);
        ui.set_course_remaining(cr);
        ui.set_courses_empty(ct == 0);
    })
}

fn make_refresh_meals(ui: &AppWindow, app_state: &Arc<Mutex<App>>) -> RefreshFn {
    let ui_weak = ui.as_weak();
    let app = app_state.clone();
    Arc::new(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        let st = lock_app(&app);
        ui.set_meals(make_meals(&st.state.meals));
    })
}

fn make_refresh_sync(ui: &AppWindow, app_state: &Arc<Mutex<App>>) -> RefreshFn {
    let ui_weak = ui.as_weak();
    let app = app_state.clone();
    Arc::new(move || {
        let Some(ui) = ui_weak.upgrade() else { return };
        let st = lock_app(&app);
        ui.set_sync_state(st.sync_state_int());
        ui.set_sync_label(st.sync_label());
    })
}

fn make_toast(ui: &AppWindow) -> ToastFn {
    let ui_weak = ui.as_weak();
    Arc::new(move |msg: &str| {
        let Some(ui) = ui_weak.upgrade() else { return };
        ui.set_toast_msg(msg.into());
        ui.set_toast_show(true);
        let ui_w2 = ui_weak.clone();
        slint::Timer::single_shot(std::time::Duration::from_millis(2000), move || {
            if let Some(u) = ui_w2.upgrade() {
                u.set_toast_show(false);
            }
        });
    })
}
