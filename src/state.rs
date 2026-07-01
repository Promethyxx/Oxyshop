//! État applicatif partagé (`App`) et fonctions de conversion des données
//! métier vers les modèles Slint (make_categories, make_course_cats, ...).

use crate::data::{self, cat_color_hex, cat_icon, AppState, CAT_ORDER};
use crate::storage::{dav_save, save_local, DavConfig};
use crate::{CatGroup, CourseCat, CourseItem, MealSlot, StockItem};
use slint::{Color, ModelRc, SharedString, VecModel};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

pub struct App {
    pub state: AppState,
    pub open_cats: HashMap<String, bool>,
    pub config: DavConfig,
    pub dav_ok: bool,
    pub ctx_target: i32,
    pub meal_target: i32,
    pub lang_en: bool,
}

impl App {
    pub fn sync_state_int(&self) -> i32 {
        if self.dav_ok {
            1
        } else if self.config.is_complete() || self.config.has_dav2() {
            2
        } else {
            0
        }
    }

    pub fn sync_label(&self) -> SharedString {
        if self.dav_ok {
            "☁️ WebDAV".into()
        } else if self.config.is_complete() || self.config.has_dav2() {
            if self.lang_en {
                "⚠️ Disconnected".into()
            } else {
                "⚠️ Déconnecté".into()
            }
        } else {
            "💾 Local".into()
        }
    }

    fn save_bg(state: AppState, cfg: DavConfig) {
        std::thread::spawn(move || {
            if let Err(e) = dav_save(&cfg, &state) {
                eprintln!("Sauvegarde WebDAV échouée : {e}");
            }
        });
    }

    pub fn save(&mut self) {
        if let Err(e) = save_local(&self.config, &self.state) {
            eprintln!("Sauvegarde locale échouée : {e}");
        }
        if self.config.is_complete() || self.config.has_dav2() {
            Self::save_bg(self.state.clone(), self.config.clone());
        }
    }
}

/// Verrouille le mutex applicatif partagé.
///
/// En cas de mutex "empoisonné" (un thread a paniqué pendant qu'il le
/// détenait), on récupère les données internes plutôt que de paniquer à
/// notre tour en cascade : mieux vaut continuer avec un état potentiellement
/// incohérent visible par l'utilisateur qu'un crash silencieux de toute
/// l'application.
pub fn lock_app(app: &Arc<Mutex<App>>) -> MutexGuard<'_, App> {
    app.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ── Conversion données métier → modèles Slint ─────────────────────────────

pub fn hex_color(hex: &str) -> Color {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(hex.get(0..2).unwrap_or("80"), 16).unwrap_or(128);
    let g = u8::from_str_radix(hex.get(2..4).unwrap_or("80"), 16).unwrap_or(128);
    let b = u8::from_str_radix(hex.get(4..6).unwrap_or("80"), 16).unwrap_or(128);
    Color::from_rgb_u8(r, g, b)
}

pub fn make_categories(
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

    for items in cat_map.values_mut() {
        items.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));
    }

    let mut groups: Vec<CatGroup> = Vec::new();

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
                color: hex_color(cat_color_hex(cat_name)),
                items: ModelRc::new(VecModel::from(slint_items)),
                missing,
                open,
            });
        }
    }

    ModelRc::new(VecModel::from(groups))
}

pub fn make_course_cats(
    stock: &[data::StockItem],
    checked: &HashMap<String, bool>,
    search: &str,
) -> (ModelRc<CourseCat>, i32, i32, i32) {
    let q = search.to_lowercase();
    let needed: Vec<(usize, &data::StockItem)> = stock
        .iter()
        .enumerate()
        .filter(|(_, i)| {
            i.obj > 0
                && i.qty < i.obj
                && (q.is_empty()
                    || i.name.to_lowercase().contains(&q)
                    || i.cat.to_lowercase().contains(&q))
        })
        .collect();

    let total = needed.len() as i32;
    let done = needed
        .iter()
        .filter(|(_, i)| {
            *checked
                .get(&AppState::checked_key(&i.cat, &i.name))
                .unwrap_or(&false)
        })
        .count() as i32;
    let remaining = total - done;

    let mut sorted = needed.clone();
    sorted.sort_by_key(|(_, i)| {
        let k = AppState::checked_key(&i.cat, &i.name);
        if *checked.get(&k).unwrap_or(&false) {
            1
        } else {
            0
        }
    });

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

pub fn make_meals(meals: &[data::MealEntry]) -> ModelRc<MealSlot> {
    let slots: Vec<MealSlot> = meals
        .iter()
        .enumerate()
        .map(|(i, m)| MealSlot {
            time: m.time.clone().into(),
            label: m.label.clone().into(),
            accent: hex_color(&m.accent),
            content: m.content.clone().into(),
            index: i as i32,
        })
        .collect();
    ModelRc::new(VecModel::from(slots))
}
