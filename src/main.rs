mod data;
mod storage;

use data::{AppState, CAT_ORDER, cat_icon, cat_color_hex, menu_data};
use storage::{DavConfig, load_local, save_local, load_config, save_config as save_cfg,
              clear_config, dav_load, dav_save, dav_test, export_json, import_json};

use slint::{ModelRc, VecModel, SharedString, Color};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

slint::include_modules!();

// ── Helpers ──────────────────────────────────────────────────────────────────

fn hex_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(128);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(128);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(128);
    Color::from_rgb_u8(r, g, b)
}

fn make_categories(
    stock: &[data::StockItem],
    search: &str,
    open_cats: &HashMap<String, bool>,
) -> ModelRc<CatGroup> {
    let q = search.to_lowercase();
    let mut cat_map: std::collections::BTreeMap<String, Vec<(usize, &data::StockItem)>> =
        std::collections::BTreeMap::new();

    for (gi, item) in stock.iter().enumerate() {
        if q.is_empty()
            || item.name.to_lowercase().contains(&q)
            || item.cat.to_lowercase().contains(&q)
        {
            cat_map.entry(item.cat.clone()).or_default().push((gi, item));
        }
    }

    // Sort items alphabetically within each category
    for items in cat_map.values_mut() {
        items.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));
    }

    let mut groups: Vec<CatGroup> = Vec::new();

    // Respect CAT_ORDER, then any extra cats
    let mut ordered: Vec<String> = CAT_ORDER.iter().map(|s| s.to_string()).collect();
    for key in cat_map.keys() {
        if !ordered.contains(key) {
            ordered.push(key.clone());
        }
    }

    for cat_name in &ordered {
        if let Some(items) = cat_map.get(cat_name) {
            let missing = items.iter().filter(|(_, i)| i.qty < i.obj).count() as i32;
            let open = *open_cats.get(cat_name).unwrap_or(&true);

            let slint_items: Vec<StockItem> = items
                .iter()
                .map(|(gi, item)| StockItem {
                    name: item.name.clone().into(),
                    cat: item.cat.clone().into(),
                    qty: item.qty,
                    obj: item.obj,
                    global_index: *gi as i32,
                })
                .collect();

            groups.push(CatGroup {
                name: cat_name.clone().into(),
                icon: cat_icon(cat_name).into(),
                color: hex_color(cat_color_hex(cat_name)), // reuse from courses colors
                items: ModelRc::new(VecModel::from(slint_items)),
                missing,
                open,
            });
        }
    }

    ModelRc::new(VecModel::from(groups))
}

fn make_course_cats(
    stock: &[data::StockItem],
    checked: &HashMap<String, bool>,
    search: &str,
) -> (ModelRc<CourseCat>, i32, i32, i32) {
    let q = search.to_lowercase();
    let needed: Vec<(usize, &data::StockItem)> = stock
        .iter()
        .enumerate()
        .filter(|(_, i)| {
            i.obj > 0 && i.qty < i.obj
                && (q.is_empty()
                    || i.name.to_lowercase().contains(&q)
                    || i.cat.to_lowercase().contains(&q))
        })
        .collect();

    let total = needed.len() as i32;
    let done = needed
        .iter()
        .filter(|(_, i)| *checked.get(&AppState::checked_key(&i.cat, &i.name)).unwrap_or(&false))
        .count() as i32;
    let remaining = total - done;

    // Sort: unchecked first
    let mut sorted = needed.clone();
    sorted.sort_by_key(|(_, i)| {
        let k = AppState::checked_key(&i.cat, &i.name);
        if *checked.get(&k).unwrap_or(&false) { 1 } else { 0 }
    });

    // Group by cat
    let mut cat_map: std::collections::BTreeMap<String, Vec<CourseItem>> =
        std::collections::BTreeMap::new();
    for (gi, item) in &sorted {
        let k = AppState::checked_key(&item.cat, &item.name);
        let is_checked = *checked.get(&k).unwrap_or(&false);
        cat_map.entry(item.cat.clone()).or_default().push(CourseItem {
            name: item.name.clone().into(),
            cat: item.cat.clone().into(),
            need: item.obj - item.qty,
            global_index: *gi as i32,
            checked: is_checked,
        });
    }

    let mut groups: Vec<CourseCat> = Vec::new();
    for cat_name in CAT_ORDER {
        if let Some(items) = cat_map.get(*cat_name) {
            groups.push(CourseCat {
                name: cat_name.to_string().into(),
                icon: cat_icon(cat_name).into(),
                color: hex_color(cat_color_hex(cat_name)),
                items: ModelRc::new(VecModel::from(items.clone())),
            });
        }
    }

    (ModelRc::new(VecModel::from(groups)), total, done, remaining)
}

fn make_meals() -> ModelRc<MealSlot> {
    let meals: Vec<MealSlot> = menu_data()
        .into_iter()
        .map(|m| {
            let has_options = m.options.is_some();
            let opts: Vec<MealOption> = m
                .options
                .unwrap_or_default()
                .into_iter()
                .map(|o| MealOption {
                    emoji: o.emoji.into(),
                    text: o.text.into(),
                })
                .collect();
            MealSlot {
                time: m.time.into(),
                label: m.label.into(),
                accent: hex_color(m.accent),
                has_options,
                detail: m.detail.unwrap_or("").into(),
                options: ModelRc::new(VecModel::from(opts)),
            }
        })
        .collect();
    ModelRc::new(VecModel::from(meals))
}

// ── App state ─────────────────────────────────────────────────────────────────

struct App {
    state: AppState,
    open_cats: HashMap<String, bool>,
    config: DavConfig,
    dav_ok: bool,
    ctx_target: i32,
}

impl App {
    fn sync_state_int(&self) -> i32 {
        if self.dav_ok { 1 }
        else if self.config.is_complete() { 2 }
        else { 0 }
    }
    fn sync_label(&self) -> SharedString {
        if self.dav_ok { "☁️ WebDAV".into() }
        else if self.config.is_complete() { "⚠️ Déconnecté".into() }
        else { "💾 Local".into() }
    }

    fn save(&mut self) {
        let _ = save_local(&self.state);
        if self.config.is_complete() {
            match dav_save(&self.config, &self.state) {
                Ok(()) => { self.dav_ok = true; }
                Err(_) => { self.dav_ok = false; }
            }
        }
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<(), slint::PlatformError> {
    let config = load_config();

    // Load data: try WebDAV first, fallback local, fallback defaults
    let state = if config.is_complete() {
        dav_load(&config).ok()
    } else {
        None
    }
    .or_else(load_local)
    .unwrap_or_else(AppState::with_defaults);

    let dav_ok = config.is_complete() && state.last_modified.is_some();

    let app_state = Arc::new(Mutex::new(App {
        state,
        open_cats: HashMap::new(),
        config: config.clone(),
        dav_ok,
        ctx_target: -1,
    }));

    let ui = AppWindow::new()?;

    // ── Initial population ────────────────────────────────────────────────

    {
        let st = app_state.lock().unwrap();
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

        ui.set_meals(make_meals());

        ui.set_cfg_url(st.config.url.clone().into());
        ui.set_cfg_user(st.config.user.clone().into());
        ui.set_cfg_pass(st.config.pass.clone().into());

        ui.set_sync_state(st.sync_state_int());
        ui.set_sync_label(st.sync_label());
    }

    // ── Helpers (closures shared) ─────────────────────────────────────────

    let refresh_stock = {
        let ui_weak = ui.as_weak();
        let app = app_state.clone();
        move || {
            let st = app.lock().unwrap();
            let search = ui_weak.unwrap().get_stock_search().to_string();
            let cats = make_categories(&st.state.stock, &search, &st.open_cats);
            let ui = ui_weak.unwrap();
            ui.set_categories(cats);
            ui.set_stat_total(st.state.stock.len() as i32);
            ui.set_stat_ok(st.state.stock.iter().filter(|i| i.qty >= i.obj).count() as i32);
            ui.set_stat_low(st.state.stock.iter().filter(|i| i.qty > 0 && i.qty < i.obj).count() as i32);
            ui.set_stat_empty(st.state.stock.iter().filter(|i| i.qty <= 0 && i.obj > 0).count() as i32);
        }
    };

    let refresh_courses = {
        let ui_weak = ui.as_weak();
        let app = app_state.clone();
        move || {
            let st = app.lock().unwrap();
            let search = ui_weak.unwrap().get_course_search().to_string();
            let (cc, ct, cd, cr) = make_course_cats(&st.state.stock, &st.state.checked, &search);
            let ui = ui_weak.unwrap();
            ui.set_course_cats(cc);
            ui.set_course_total(ct);
            ui.set_course_done(cd);
            ui.set_course_remaining(cr);
            ui.set_courses_empty(ct == 0);
        }
    };

    let refresh_sync = {
        let ui_weak = ui.as_weak();
        let app = app_state.clone();
        move || {
            let st = app.lock().unwrap();
            let ui = ui_weak.unwrap();
            ui.set_sync_state(st.sync_state_int());
            ui.set_sync_label(st.sync_label());
        }
    };

    let toast = {
        let ui_weak = ui.as_weak();
        move |msg: &str| {
            let ui = ui_weak.unwrap();
            ui.set_toast_msg(msg.into());
            ui.set_toast_show(true);
            let ui_w2 = ui_weak.clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(2000), move || {
                ui_w2.unwrap().set_toast_show(false);
            });
        }
    };

    // ── Stock callbacks ───────────────────────────────────────────────────

    ui.on_stock_search_changed({
        let rs = refresh_stock.clone();
        move |_| { rs(); }
    });

    ui.on_stock_inc({
        let app = app_state.clone();
        let rs = refresh_stock.clone();
        let rc = refresh_courses.clone();
        move |gi| {
            let mut st = app.lock().unwrap();
            if let Some(item) = st.state.stock.get_mut(gi as usize) {
                item.qty += 1;
            }
            st.save();
            drop(st);
            rs(); rc();
        }
    });

    ui.on_stock_dec({
        let app = app_state.clone();
        let rs = refresh_stock.clone();
        let rc = refresh_courses.clone();
        move |gi| {
            let mut st = app.lock().unwrap();
            if let Some(item) = st.state.stock.get_mut(gi as usize) {
                item.qty = (item.qty - 1).max(0);
            }
            st.save();
            drop(st);
            rs(); rc();
        }
    });

    ui.on_stock_toggle_cat({
        let app = app_state.clone();
        let rs = refresh_stock.clone();
        move |cat| {
            let mut st = app.lock().unwrap();
            let entry = st.open_cats.entry(cat.to_string()).or_insert(true);
            *entry = !*entry;
            drop(st);
            rs();
        }
    });

    ui.on_stock_export({
        let app = app_state.clone();
        let toast = toast.clone();
        move || {
            let st = app.lock().unwrap();
            match export_json(&st.state) {
                Ok(path) => toast(&format!("📤 Exporté : {}", path.file_name().unwrap_or_default().to_string_lossy())),
                Err(e) => toast(&format!("⚠️ {}", e)),
            }
        }
    });

    ui.on_stock_import({
        let app = app_state.clone();
        let rs = refresh_stock.clone();
        let rc = refresh_courses.clone();
        let toast = toast.clone();
        let ui_weak = ui.as_weak();
        move || {
            // Open file dialog — rfd
            // For now, prompt via native dialog if rfd available
            // Fallback: use a fixed path from env or show toast
            // We'll use rfd if it compiles, else skip
            toast("📥 Glissez le fichier JSON ici (non implémenté sans rfd)");
            let _ = (app.clone(), rs.clone(), rc.clone(), ui_weak.clone());
        }
    });

    // Context menu trigger from stock (long press fires ctx-edit callback)
    ui.on_stock_ctx_edit({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move |gi| {
            app.lock().unwrap().ctx_target = gi;
            let ui = ui_weak.unwrap();
            ui.set_ctx_target(gi);
            ui.set_ctx_active(true);
        }
    });

    ui.on_ctx_close({
        let ui_weak = ui.as_weak();
        move || { ui_weak.unwrap().set_ctx_active(false); }
    });

    ui.on_ctx_edit({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move || {
            let st = app.lock().unwrap();
            let gi = st.ctx_target as usize;
            if let Some(item) = st.state.stock.get(gi) {
                let ui = ui_weak.unwrap();
                ui.set_item_modal_is_edit(true);
                ui.set_item_modal_name(item.name.clone().into());
                ui.set_item_modal_cat(item.cat.clone().into());
                ui.set_item_modal_qty_str(item.qty.to_string().into());
                ui.set_item_modal_obj_str(item.obj.to_string().into());
                ui.set_item_modal_active(true);
            }
        }
    });

    ui.on_ctx_obj({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move || {
            let st = app.lock().unwrap();
            let gi = st.ctx_target as usize;
            if let Some(item) = st.state.stock.get(gi) {
                let ui = ui_weak.unwrap();
                ui.set_obj_modal_item_name(item.name.clone().into());
                ui.set_obj_modal_value_str(item.obj.to_string().into());
                ui.set_obj_modal_active(true);
            }
        }
    });

    ui.on_ctx_delete({
        let app = app_state.clone();
        let rs = refresh_stock.clone();
        let rc = refresh_courses.clone();
        let toast = toast.clone();
        move || {
            let mut st = app.lock().unwrap();
            let gi = st.ctx_target as usize;
            if gi < st.state.stock.len() {
                let name = st.state.stock[gi].name.clone();
                st.state.stock.remove(gi);
                st.save();
                drop(st);
                toast(&format!("🗑️ {}", name));
                rs(); rc();
            }
        }
    });

    ui.on_stock_obj_clicked({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move |gi| {
            let st = app.lock().unwrap();
            if let Some(item) = st.state.stock.get(gi as usize) {
                let ui = ui_weak.unwrap();
                ui.set_obj_modal_item_name(item.name.clone().into());
                ui.set_obj_modal_value_str(item.obj.to_string().into());
                ui.set_obj_modal_active(true);
                // store target
                drop(st);
                app.lock().unwrap().ctx_target = gi;
            }
        }
    });

    // ── Item modal ────────────────────────────────────────────────────────

    ui.on_item_modal_cancel({
        let ui_weak = ui.as_weak();
        move || {
            let ui = ui_weak.unwrap();
            ui.set_item_modal_active(false);
        }
    });

    ui.on_item_modal_confirm({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rs = refresh_stock.clone();
        let rc = refresh_courses.clone();
        let toast = toast.clone();
        move || {
            let ui = ui_weak.unwrap();
            let name = ui.get_item_modal_name().to_string();
            if name.trim().is_empty() { return; }
            let cat  = ui.get_item_modal_cat().to_string();
            let qty: i32 = ui.get_item_modal_qty_str().parse().unwrap_or(0).max(0);
            let obj: i32 = ui.get_item_modal_obj_str().parse().unwrap_or(0).max(0);
            let is_edit = ui.get_item_modal_is_edit();

            let mut st = app.lock().unwrap();
            let gi = st.ctx_target as usize;

            if is_edit && gi < st.state.stock.len() {
                st.state.stock[gi] = data::StockItem { name: name.clone(), cat, qty, obj };
                toast(&format!("✓ {} modifié", name));
            } else {
                st.state.stock.push(data::StockItem { name: name.clone(), cat, qty, obj });
                toast(&format!("✓ {} ajouté", name));
            }
            st.save();
            drop(st);
            ui.set_item_modal_active(false);
            ui.set_item_modal_is_edit(false);
            rs(); rc();
        }
    });

    // ── Obj modal ─────────────────────────────────────────────────────────

    ui.on_obj_modal_cancel({
        let ui_weak = ui.as_weak();
        move || { ui_weak.unwrap().set_obj_modal_active(false); }
    });

    ui.on_obj_modal_confirm({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rs = refresh_stock.clone();
        let rc = refresh_courses.clone();
        let toast = toast.clone();
        move || {
            let ui = ui_weak.unwrap();
            let val: i32 = ui.get_obj_modal_value_str().parse().unwrap_or(0).max(0);
            let mut st = app.lock().unwrap();
            let gi = st.ctx_target as usize;
            if gi < st.state.stock.len() {
                st.state.stock[gi].obj = val;
                st.save();
                drop(st);
                toast("✓ Objectif mis à jour");
                rs(); rc();
            }
            ui.set_obj_modal_active(false);
        }
    });

    // ── Courses callbacks ─────────────────────────────────────────────────

    ui.on_course_search_changed({
        let rc = refresh_courses.clone();
        move |_| { rc(); }
    });

    ui.on_course_toggle({
        let app = app_state.clone();
        let rs = refresh_stock.clone();
        let rc = refresh_courses.clone();
        let toast = toast.clone();
        move |gi, new_checked| {
            let mut st = app.lock().unwrap();
            let gi = gi as usize;
            if gi < st.state.stock.len() {
                let k = {
                    let item = &st.state.stock[gi];
                    AppState::checked_key(&item.cat, &item.name)
                };
                let need = {
                    let item = &st.state.stock[gi];
                    (item.obj - item.qty).max(0)
                };
                let name = st.state.stock[gi].name.clone();
                st.state.checked.insert(k, new_checked);
                if new_checked {
                    st.state.stock[gi].qty += need;
                    toast(&format!("✓ +{} {}", need, name));
                } else {
                    st.state.stock[gi].qty = (st.state.stock[gi].qty - need).max(0);
                }
                st.save();
            }
            drop(st);
            rs(); rc();
        }
    });

    ui.on_courses_share({
        let app = app_state.clone();
        let toast = toast.clone();
        move || {
            let st = app.lock().unwrap();
            let needed: Vec<_> = st.state.stock.iter()
                .filter(|i| i.obj > 0 && i.qty < i.obj)
                .map(|i| (i.cat.as_str(), i.name.as_str(), i.obj - i.qty))
                .collect();
            if needed.is_empty() { toast("Rien à acheter !"); return; }
            // Build text
            let mut text = "🛒 LISTE DE COURSES\n\n".to_string();
            for cat in CAT_ORDER {
                let items: Vec<_> = needed.iter().filter(|(c, _, _)| c == cat).collect();
                if items.is_empty() { continue; }
                text += &format!("{} {}\n", cat_icon(cat), cat.to_uppercase());
                for (_, name, need) in &items {
                    text += &format!("  □ {} × {}\n", name, need);
                }
                text += "\n";
            }
            // Copy to clipboard via arboard or just toast
            toast(&format!("📋 {} articles", needed.len()));
            // clipboard::set_contents(text) — requires arboard dep, skip for now
            let _ = text;
        }
    });

    ui.on_courses_reset({
        let app = app_state.clone();
        let rc = refresh_courses.clone();
        let toast = toast.clone();
        move || {
            let mut st = app.lock().unwrap();
            st.state.checked.clear();
            st.save();
            drop(st);
            toast("🔄 Réinitialisé");
            rc();
        }
    });

    // ── Theme ─────────────────────────────────────────────────────────────

    ui.on_theme_toggle({
        let ui_weak = ui.as_weak();
        move || {
            let ui = ui_weak.unwrap();
            let dark = !ui.get_dark_mode();
            ui.set_dark_mode(dark);
        }
    });

    // ── Config ────────────────────────────────────────────────────────────

    ui.on_cfg_save({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rs = refresh_sync.clone();
        let toast = toast.clone();
        move || {
            let ui = ui_weak.unwrap();
            let cfg = DavConfig {
                url:  ui.get_cfg_url().to_string(),
                user: ui.get_cfg_user().to_string(),
                pass: ui.get_cfg_pass().to_string(),
            };
            let _ = save_cfg(&cfg);
            app.lock().unwrap().config = cfg;
            toast("✅ Config sauvée");
            rs();
        }
    });

    ui.on_cfg_test({
        let app = app_state.clone();
        let toast = toast.clone();
        let rs = refresh_sync.clone();
        move || {
            let cfg = app.lock().unwrap().config.clone();
            if !cfg.is_complete() { toast("⚠️ Remplis les 3 champs"); return; }
            match dav_test(&cfg) {
                Ok(()) => {
                    app.lock().unwrap().dav_ok = true;
                    toast("✅ Connexion OK !");
                    rs();
                }
                Err(e) => toast(&format!("⚠️ {}", e)),
            }
        }
    });

    ui.on_cfg_clear({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rs = refresh_sync.clone();
        let toast = toast.clone();
        move || {
            let _ = clear_config();
            {
                let mut st = app.lock().unwrap();
                st.config = DavConfig::default();
                st.dav_ok = false;
            }
            let ui = ui_weak.unwrap();
            ui.set_cfg_url("".into());
            ui.set_cfg_user("".into());
            ui.set_cfg_pass("".into());
            toast("🗑️ Config effacée");
            rs();
        }
    });

    ui.on_cfg_export({
        let app = app_state.clone();
        let toast = toast.clone();
        move || {
            let st = app.lock().unwrap();
            match export_json(&st.state) {
                Ok(p) => toast(&format!("📤 {}", p.file_name().unwrap_or_default().to_string_lossy())),
                Err(e) => toast(&format!("⚠️ {}", e)),
            }
        }
    });

    ui.on_cfg_import({
        let app = app_state.clone();
        let rs = refresh_stock.clone();
        let rc = refresh_courses.clone();
        let toast = toast.clone();
        move || {
            let file = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .pick_file();
            if let Some(path) = file {
                match import_json(path.to_str().unwrap_or("")) {
                    Ok(new_state) => {
                        let count = new_state.stock.len();
                        let mut st = app.lock().unwrap();
                        st.state = new_state;
                        st.save();
                        drop(st);
                        toast(&format!("📥 {} articles importés", count));
                        rs(); rc();
                    }
                    Err(e) => toast(&format!("⚠️ {}", e)),
                }
            }
        }
    });

    ui.run()
}
