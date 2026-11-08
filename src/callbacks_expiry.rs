use crate::expiry;
use crate::state::{lock_app, App};
use crate::{AppWindow, RefreshFn, ToastFn};
use slint::ComponentHandle;
use std::sync::{Arc, Mutex};

fn open_add_lot(ui: &AppWindow, st: &mut App, key: &str) {
    let Some((_, name)) = st.state.expiry_resolve_key(key) else {
        return;
    };
    st.expiry_edit_key = key.to_string();
    st.expiry_edit_lot = -1;
    ui.set_lot_modal_is_edit(false);
    ui.set_lot_modal_show_name(false);
    ui.set_lot_modal_title(name.into());
    ui.set_lot_modal_name("".into());
    ui.set_lot_modal_cat("".into());
    ui.set_lot_modal_qty_str("1".into());
    ui.set_lot_modal_date_str("".into());
    ui.set_lot_modal_active(true);
}

pub fn setup(
    ui: &AppWindow,
    app_state: &Arc<Mutex<App>>,
    rs: &RefreshFn,
    rc: &RefreshFn,
    re: &RefreshFn,
    toast: &ToastFn,
) {
    ui.on_expiry_search_changed({
        let re = re.clone();
        move |_| re()
    });

    ui.on_expiry_opened({
        let re = re.clone();
        move || re()
    });

    ui.on_expiry_add({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            {
                let mut st = lock_app(&app);
                st.expiry_edit_key.clear();
                st.expiry_edit_lot = -1;
            }
            ui.set_lot_modal_is_edit(false);
            ui.set_lot_modal_show_name(true);
            ui.set_lot_modal_title("".into());
            ui.set_lot_modal_name("".into());
            ui.set_lot_modal_cat("".into());
            ui.set_lot_modal_qty_str("1".into());
            ui.set_lot_modal_date_str("".into());
            ui.set_lot_modal_active(true);
        }
    });

    ui.on_expiry_add_lot({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move |key| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = lock_app(&app);
            open_add_lot(&ui, &mut st, &key);
        }
    });

    ui.on_expiry_lot_clicked({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move |key, lot_idx| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = lock_app(&app);
            let Some(i) = st.state.expiry_find_key(&key) else {
                return;
            };
            let Some(lot) = st.state.expiry[i].lots.get(lot_idx as usize) else {
                return;
            };
            let qty = lot.qty;
            let date_str = lot
                .date
                .as_deref()
                .and_then(expiry::from_iso)
                .map(expiry::to_display)
                .unwrap_or_default();
            let name = st.state.expiry[i].name.clone();
            st.expiry_edit_key = key.to_string();
            st.expiry_edit_lot = lot_idx;
            drop(st);
            ui.set_lot_modal_is_edit(true);
            ui.set_lot_modal_show_name(false);
            ui.set_lot_modal_title(name.into());
            ui.set_lot_modal_qty_str(qty.to_string().into());
            ui.set_lot_modal_date_str(date_str.into());
            ui.set_lot_modal_active(true);
        }
    });

    ui.on_expiry_retire({
        let app = app_state.clone();
        let rs = rs.clone();
        let rc = rc.clone();
        let re = re.clone();
        let toast = toast.clone();
        move |key, lot_idx| {
            let mut st = lock_app(&app);
            let name = st
                .state
                .expiry_find_key(&key)
                .map(|i| st.state.expiry[i].name.clone())
                .unwrap_or_default();
            let taken = st
                .state
                .expiry_retire(&key, lot_idx as usize, 1)
                .unwrap_or(0);
            if taken > 0 {
                st.save();
            }
            drop(st);
            if taken > 0 {
                toast(&format!("✓ −{} {}", taken, name));
            }
            rs();
            rc();
            re();
        }
    });

    ui.on_expiry_ctx({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move |key| {
            lock_app(&app).expiry_ctx_key = key.to_string();
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_expiry_ctx_active(true);
            }
        }
    });

    ui.on_expiry_ctx_close({
        let ui_weak = ui.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_expiry_ctx_active(false);
            }
        }
    });

    ui.on_expiry_ctx_add_lot({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = lock_app(&app);
            let key = st.expiry_ctx_key.clone();
            open_add_lot(&ui, &mut st, &key);
        }
    });

    ui.on_expiry_ctx_excl_item({
        let app = app_state.clone();
        let re = re.clone();
        let toast = toast.clone();
        move || {
            let mut st = lock_app(&app);
            let key = st.expiry_ctx_key.clone();
            let lang = st.lang_en;
            if let Some((cat, name)) = st.state.expiry_resolve_key(&key) {
                st.state.expiry_exclude_item(&cat, &name);
                st.save();
                drop(st);
                toast(&format!("🚫 {}", name));
                re();
            } else {
                drop(st);
                toast(if lang { "⚠️ Item not found" } else { "⚠️ Produit introuvable" });
            }
        }
    });

    ui.on_expiry_ctx_excl_cat({
        let app = app_state.clone();
        let re = re.clone();
        let toast = toast.clone();
        move || {
            let mut st = lock_app(&app);
            let key = st.expiry_ctx_key.clone();
            let lang = st.lang_en;
            if let Some((cat, _)) = st.state.expiry_resolve_key(&key) {
                st.state.expiry_exclude_cat(&cat);
                st.save();
                drop(st);
                toast(&format!("🚫 {}", cat));
                re();
            } else {
                drop(st);
                toast(if lang { "⚠️ Item not found" } else { "⚠️ Produit introuvable" });
            }
        }
    });

    ui.on_expiry_include({
        let app = app_state.clone();
        let re = re.clone();
        move |key, is_cat| {
            let mut st = lock_app(&app);
            st.state.expiry_include(&key, is_cat);
            st.save();
            drop(st);
            re();
        }
    });

    ui.on_lot_modal_cancel({
        let ui_weak = ui.as_weak();
        move || {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_lot_modal_active(false);
            }
        }
    });

    ui.on_lot_modal_quick({
        let ui_weak = ui.as_weak();
        move |kind| {
            let Some(ui) = ui_weak.upgrade() else { return };
            if let Some(d) = expiry::quick_date(kind, expiry::today()) {
                ui.set_lot_modal_date_str(expiry::to_display(d).into());
            }
        }
    });

    ui.on_lot_modal_confirm({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let re = re.clone();
        let toast = toast.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let lang = lock_app(&app).lang_en;

            let qty: i32 = ui.get_lot_modal_qty_str().trim().parse().unwrap_or(0);
            if qty < 1 {
                toast(if lang { "⚠️ Quantity must be at least 1" } else { "⚠️ Quantité minimum : 1" });
                return;
            }

            let date_raw = ui.get_lot_modal_date_str().to_string();
            let date = if date_raw.trim().is_empty() {
                None
            } else {
                match expiry::parse_date(&date_raw, expiry::today()) {
                    Some(d) => Some(d),
                    None => {
                        toast(if lang { "⚠️ Invalid date" } else { "⚠️ Date invalide" });
                        return;
                    }
                }
            };

            let is_edit = ui.get_lot_modal_is_edit();
            let show_name = ui.get_lot_modal_show_name();
            let mut st = lock_app(&app);
            let mut shown_name = ui.get_lot_modal_title().to_string();
            let mut hidden = false;

            if is_edit {
                let key = st.expiry_edit_key.clone();
                let lot = st.expiry_edit_lot;
                if lot >= 0 {
                    st.state.expiry_set_lot(&key, lot as usize, qty, date);
                }
            } else if show_name {
                let name = ui.get_lot_modal_name().trim().to_string();
                if name.is_empty() {
                    return;
                }
                let cat_in = ui.get_lot_modal_cat().trim().to_string();
                let cat = if cat_in.is_empty() { "Nourriture".to_string() } else { cat_in };
                st.state.expiry_add_lot(&cat, &name, qty, date);
                hidden = st.state.expiry_is_excluded(&cat, &name);
                shown_name = name;
            } else {
                let key = st.expiry_edit_key.clone();
                if let Some((cat, name)) = st.state.expiry_resolve_key(&key) {
                    st.state.expiry_add_lot(&cat, &name, qty, date);
                    hidden = st.state.expiry_is_excluded(&cat, &name);
                }
            }

            st.save();
            drop(st);
            ui.set_lot_modal_active(false);
            re();
            if hidden {
                toast(if lang {
                    "⚠️ Added, but its category is excluded"
                } else {
                    "⚠️ Ajouté, mais sa catégorie est exclue"
                });
            } else {
                toast(&format!("✓ {}", shown_name));
            }
        }
    });

    ui.on_lot_modal_retire({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let rs = rs.clone();
        let rc = rc.clone();
        let re = re.clone();
        let toast = toast.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = lock_app(&app);
            let key = st.expiry_edit_key.clone();
            let lot = st.expiry_edit_lot;
            let name = ui.get_lot_modal_title().to_string();
            let taken = if lot >= 0 {
                st.state.expiry_retire(&key, lot as usize, i32::MAX).unwrap_or(0)
            } else {
                0
            };
            if taken > 0 {
                st.save();
            }
            drop(st);
            ui.set_lot_modal_active(false);
            if taken > 0 {
                toast(&format!("✓ −{} {}", taken, name));
            }
            rs();
            rc();
            re();
        }
    });

    ui.on_lot_modal_delete_lot({
        let app = app_state.clone();
        let ui_weak = ui.as_weak();
        let re = re.clone();
        let toast = toast.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = lock_app(&app);
            let key = st.expiry_edit_key.clone();
            let lot = st.expiry_edit_lot;
            let lang = st.lang_en;
            let removed = lot >= 0 && st.state.expiry_delete_lot(&key, lot as usize);
            if removed {
                st.save();
            }
            drop(st);
            ui.set_lot_modal_active(false);
            if removed {
                toast(if lang { "🗑️ Lot deleted" } else { "🗑️ Lot supprimé" });
            }
            re();
        }
    });
}
