//! Callbacks Slint pour le stock, les modales (item/objectif/quantité),
//! les courses et les repas.

use crate::state::{lock_app, App};
#[cfg(target_os = "android")]
use crate::storage::export_json;
use crate::storage::{export_json_to, import_json};
use crate::data::{self, accent_idx, AppState, ACCENT_COLORS};
use crate::{AppWindow, RefreshFn, ToastFn};
use slint::ComponentHandle;
use std::sync::{Arc, Mutex};

pub fn setup(
    ui: &AppWindow,
    app_state: &Arc<Mutex<App>>,
    rs: &RefreshFn,
    rc: &RefreshFn,
    rm: &RefreshFn,
    toast: &ToastFn,
) {
    setup_stock(ui, app_state, rs, rc, toast);
    setup_modals(ui, app_state, rs, rc, toast);
    setup_courses(ui, app_state, rs, rc, toast);
    setup_meals(ui, app_state, rm, toast);
}

fn setup_stock(
    ui: &AppWindow,
    app_state: &Arc<Mutex<App>>,
    rs: &RefreshFn,
    rc: &RefreshFn,
    toast: &ToastFn,
) {
    ui.on_stock_search_changed({
        let rs = rs.clone();
        move |_| rs()
    });

    ui.on_stock_inc({
        let app = app_state.clone();
        let rs = rs.clone();
        let rc = rc.clone();
        move |gi| {
            let mut st = lock_app(&app);
            if let Some(item) = st.state.stock.get_mut(gi as usize) {
                item.qty += 1;
            }
            st.save();
            drop(st);
            rs();
            rc();
        }
    });

    ui.on_stock_dec({
        let app = app_state.clone();
        let rs = rs.clone();
        let rc = rc.clone();
        move |gi| {
            let mut st = lock_app(&app);
            if let Some(item) = st.state.stock.get_mut(gi as usize) {
                item.qty = (item.qty - 1).max(0);
            }
            st.save();
            drop(st);
            rs();
            rc();
        }
    });

    ui.on_stock_toggle_cat({
        let app = app_state.clone();
        let rs = rs.clone();
        move |cat| {
            let mut st = lock_app(&app);
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
            let st = lock_app(&app);
            let lang = st.lang_en;

            // Sur desktop, on laisse l'utilisateur choisir l'emplacement et le
            // nom du fichier via une boîte de dialogue "Enregistrer sous".
            #[cfg(not(target_os = "android"))]
            let result = {
                let default_path = crate::storage::default_export_path();
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
            let result = export_json(&st.config, &st.state);

            match result {
                Ok(path) => toast(&format!(
                    "📤 {} : {}",
                    if lang { "Exported" } else { "Exporté" },
                    path.file_name().map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string())
                )),
                Err(e) => toast(&format!("⚠️ {}", e)),
            }
        }
    });

    ui.on_stock_import({
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
                            let mut st = lock_app(&app);
                            st.state = new_state;
                            st.save();
                            let lang = st.lang_en;
                            drop(st);
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
            toast("📥 Utiliser Config > Import");
        }
    });

    ui.on_stock_ctx_edit({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move |gi| {
            lock_app(&app).ctx_target = gi;
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.set_ctx_target(gi);
            ui.set_ctx_active(true);
        }
    });

    ui.on_ctx_close({
        let ui_weak = ui.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_ctx_active(false);
            }
        }
    });

    ui.on_ctx_edit({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move || {
            let st = lock_app(&app);
            let gi = st.ctx_target as usize;
            if let Some(item) = st.state.stock.get(gi) {
                let Some(ui) = ui_weak.upgrade() else { return };
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
            let st = lock_app(&app);
            let gi = st.ctx_target as usize;
            if let Some(item) = st.state.stock.get(gi) {
                let Some(ui) = ui_weak.upgrade() else { return };
                ui.set_obj_modal_item_name(item.name.clone().into());
                ui.set_obj_modal_value_str(item.obj.to_string().into());
                ui.set_obj_modal_active(true);
            }
        }
    });

    ui.on_ctx_delete({
        let app = app_state.clone();
        let rs = rs.clone();
        let rc = rc.clone();
        let toast = toast.clone();
        move || {
            let mut st = lock_app(&app);
            let gi = st.ctx_target as usize;
            if gi < st.state.stock.len() {
                let name = st.state.stock[gi].name.clone();
                st.state.stock.remove(gi);
                st.save();
                let lang = st.lang_en;
                drop(st);
                toast(&format!("🗑️ {}{}", name, if lang { " deleted" } else { "" }));
                rs();
                rc();
            }
        }
    });

    ui.on_stock_obj_clicked({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move |gi| {
            let mut st = lock_app(&app);
            if let Some(item) = st.state.stock.get(gi as usize) {
                let Some(ui) = ui_weak.upgrade() else { return };
                ui.set_obj_modal_item_name(item.name.clone().into());
                ui.set_obj_modal_value_str(item.obj.to_string().into());
                ui.set_obj_modal_active(true);
                st.ctx_target = gi;
            }
        }
    });
}

fn setup_modals(
    ui: &AppWindow,
    app_state: &Arc<Mutex<App>>,
    rs: &RefreshFn,
    rc: &RefreshFn,
    toast: &ToastFn,
) {
    // ── Item modal ────────────────────────────────────────────────────────

    ui.on_item_modal_cancel({
        let ui_weak = ui.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_item_modal_active(false);
            }
        }
    });

    ui.on_item_modal_confirm({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rs = rs.clone();
        let rc = rc.clone();
        let toast = toast.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let name = ui.get_item_modal_name().to_string();
            if name.trim().is_empty() {
                return;
            }
            let cat = ui.get_item_modal_cat().to_string();
            let qty: i32 = ui.get_item_modal_qty_str().parse().unwrap_or(0).max(0);
            let obj: i32 = ui.get_item_modal_obj_str().parse().unwrap_or(0).max(0);
            let is_edit = ui.get_item_modal_is_edit();

            let mut st = lock_app(&app);
            let gi = st.ctx_target as usize;
            let lang = st.lang_en;

            if is_edit && gi < st.state.stock.len() {
                st.state.stock[gi] = data::StockItem { name: name.clone(), cat, qty, obj };
                toast(&format!("✓ {} {}", name, if lang { "edited" } else { "modifié" }));
            } else {
                st.state.stock.push(data::StockItem { name: name.clone(), cat, qty, obj });
                toast(&format!("✓ {} {}", name, if lang { "added" } else { "ajouté" }));
            }
            st.save();
            drop(st);
            ui.set_item_modal_active(false);
            ui.set_item_modal_is_edit(false);
            rs();
            rc();
        }
    });

    // ── Obj modal ─────────────────────────────────────────────────────────

    ui.on_obj_modal_cancel({
        let ui_weak = ui.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_obj_modal_active(false);
            }
        }
    });

    ui.on_obj_modal_confirm({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rs = rs.clone();
        let rc = rc.clone();
        let toast = toast.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let val: i32 = ui.get_obj_modal_value_str().parse().unwrap_or(0).max(0);
            let mut st = lock_app(&app);
            let gi = st.ctx_target as usize;
            if gi < st.state.stock.len() {
                st.state.stock[gi].obj = val;
                st.save();
                let lang = st.lang_en;
                drop(st);
                toast(if lang { "✓ Target updated" } else { "✓ Objectif mis à jour" });
                rs();
                rc();
            }
            ui.set_obj_modal_active(false);
        }
    });

    // ── Qty modal ─────────────────────────────────────────────────────────

    ui.on_stock_qty_clicked({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move |gi| {
            let mut st = lock_app(&app);
            if let Some(item) = st.state.stock.get(gi as usize) {
                let Some(ui) = ui_weak.upgrade() else { return };
                ui.set_qty_modal_item_name(item.name.clone().into());
                ui.set_qty_modal_value_str(item.qty.to_string().into());
                ui.set_qty_modal_active(true);
                st.ctx_target = gi;
            }
        }
    });

    ui.on_qty_modal_cancel({
        let ui_weak = ui.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_qty_modal_active(false);
            }
        }
    });

    ui.on_qty_modal_confirm({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rs = rs.clone();
        let rc = rc.clone();
        let toast = toast.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let val: i32 = ui.get_qty_modal_value_str().parse().unwrap_or(0).max(0);
            let mut st = lock_app(&app);
            let gi = st.ctx_target as usize;
            if gi < st.state.stock.len() {
                st.state.stock[gi].qty = val;
                st.save();
                let lang = st.lang_en;
                drop(st);
                toast(if lang { "✓ Quantity updated" } else { "✓ Quantité mise à jour" });
                rs();
                rc();
            }
            ui.set_qty_modal_active(false);
        }
    });
}

fn setup_courses(
    ui: &AppWindow,
    app_state: &Arc<Mutex<App>>,
    rs: &RefreshFn,
    rc: &RefreshFn,
    toast: &ToastFn,
) {
    ui.on_course_search_changed({
        let rc = rc.clone();
        move |_| rc()
    });

    ui.on_course_toggle({
        let app = app_state.clone();
        let rs = rs.clone();
        let rc = rc.clone();
        let toast = toast.clone();
        move |gi, new_checked| {
            let mut st = lock_app(&app);
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
            rs();
            rc();
        }
    });

    ui.on_courses_reset({
        let app = app_state.clone();
        let rc = rc.clone();
        let toast = toast.clone();
        move || {
            let mut st = lock_app(&app);
            st.state.checked.clear();
            st.save();
            let lang = st.lang_en;
            drop(st);
            toast(if lang { "🔄 Reset" } else { "🔄 Réinitialisé" });
            rc();
        }
    });
}

fn setup_meals(ui: &AppWindow, app_state: &Arc<Mutex<App>>, rm: &RefreshFn, toast: &ToastFn) {
    ui.on_meal_add({
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.set_meal_modal_is_edit(false);
            ui.set_meal_modal_time("".into());
            ui.set_meal_modal_label("".into());
            ui.set_meal_modal_content("".into());
            ui.set_meal_modal_accent_idx(0);
            ui.set_meal_modal_active(true);
        }
    });

    ui.on_meal_edit_open({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move |idx| {
            let mut st = lock_app(&app);
            st.meal_target = idx;
            let i = idx as usize;
            if i < st.state.meals.len() {
                let m = &st.state.meals[i];
                let time = m.time.clone();
                let label = m.label.clone();
                let content = m.content.clone();
                let aidx = accent_idx(&m.accent);
                drop(st);
                let Some(ui) = ui_weak.upgrade() else { return };
                ui.set_meal_modal_is_edit(true);
                ui.set_meal_modal_time(time.into());
                ui.set_meal_modal_label(label.into());
                ui.set_meal_modal_content(content.into());
                ui.set_meal_modal_accent_idx(aidx);
                ui.set_meal_modal_active(true);
            }
        }
    });

    ui.on_meal_modal_cancel({
        let ui_weak = ui.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_meal_modal_active(false);
            }
        }
    });

    ui.on_meal_modal_confirm({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rm = rm.clone();
        let toast = toast.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let time = ui.get_meal_modal_time().to_string();
            let label = ui.get_meal_modal_label().to_string();
            if label.trim().is_empty() {
                return;
            }
            let content = ui.get_meal_modal_content().to_string();
            let aidx = ui.get_meal_modal_accent_idx() as usize;
            let accent = ACCENT_COLORS.get(aidx).copied().unwrap_or("#E8A87C").to_string();
            let is_edit = ui.get_meal_modal_is_edit();

            let mut st = lock_app(&app);
            let lang = st.lang_en;
            let entry = data::MealEntry { time, label: label.clone(), accent, content };
            let msg;
            if is_edit {
                let i = st.meal_target as usize;
                if i < st.state.meals.len() {
                    st.state.meals[i] = entry;
                }
                msg = if lang { format!("✓ {} updated", label) } else { format!("✓ {} modifié", label) };
            } else {
                st.state.meals.push(entry);
                msg = if lang { format!("✓ {} added", label) } else { format!("✓ {} ajouté", label) };
            }
            st.save();
            drop(st);
            toast(&msg);
            ui.set_meal_modal_active(false);
            rm();
        }
    });

    ui.on_meal_delete({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rm = rm.clone();
        let toast = toast.clone();
        move |idx| {
            let mut st = lock_app(&app);
            let i = idx as usize;
            if i < st.state.meals.len() {
                let name = st.state.meals[i].label.clone();
                st.state.meals.remove(i);
                st.save();
                let lang = st.lang_en;
                drop(st);
                let Some(ui) = ui_weak.upgrade() else { return };
                ui.set_meal_modal_active(false);
                let msg = if lang { format!("🗑️ {} deleted", name) } else { format!("🗑️ {} supprimé", name) };
                toast(&msg);
                rm();
            }
        }
    });
}
