//! État applicatif partagé (`App`) et fonctions de conversion des données
//! métier vers les modèles Slint (make_categories, make_course_cats, ...).

use crate::data::{self, cat_color_hex, cat_icon, AppState, CAT_ORDER};
use crate::expiry;
use crate::storage::{dav_save, save_local, DavConfig};
use crate::{
    CatGroup, CourseCat, CourseItem, ExcludedRow, ExpiryEntry, ExpiryGroup, ExpiryLotRow, MealSlot,
    StockItem,
};
use slint::{Color, ModelRc, SharedString, VecModel};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};

pub struct App {
    pub state: AppState,
    pub config: DavConfig,
    pub dav_ok: bool,
    pub ctx_target: i32,
    pub meal_target: i32,
    pub expiry_ctx_key: String,
    pub expiry_edit_key: String,
    pub expiry_edit_lot: i32,
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

pub struct ExpiryModels {
    pub groups: ModelRc<ExpiryGroup>,
    pub excluded: ModelRc<ExcludedRow>,
    pub excluded_count: i32,
    pub expired: i32,
    pub soon: i32,
    pub fill: i32,
}

pub fn make_expiry(
    state: &AppState,
    search: &str,
    lang_en: bool,
) -> ExpiryModels {
    let today = expiry::today();
    let q = search.to_lowercase();

    let mut all: Vec<(&str, &str)> = state
        .stock
        .iter()
        .map(|s| (s.cat.as_str(), s.name.as_str()))
        .collect();
    for e in &state.expiry {
        if !state
            .stock
            .iter()
            .any(|s| s.cat == e.cat && s.name == e.name)
        {
            all.push((e.cat.as_str(), e.name.as_str()));
        }
    }

    let mut cat_map: BTreeMap<String, Vec<ExpiryEntry>> = BTreeMap::new();
    let mut alerts: HashMap<String, i32> = HashMap::new();
    let mut excluded_items: Vec<ExcludedRow> = Vec::new();
    let (mut expired, mut soon, mut fill) = (0, 0, 0);

    for (cat, name) in &all {
        if state.expiry_is_excluded(cat, name) {
            if !state.expiry_excl_cats.iter().any(|c| c == cat) {
                excluded_items.push(ExcludedRow {
                    key: AppState::checked_key(cat, name).into(),
                    label: format!("{} ({})", name, cat).into(),
                    is_cat: false,
                    count: 0,
                });
            }
            continue;
        }
        if !q.is_empty()
            && !name.to_lowercase().contains(&q)
            && !cat.to_lowercase().contains(&q)
        {
            continue;
        }

        let lots_src: &[data::ExpiryLot] = state
            .expiry_find(cat, name)
            .map(|i| state.expiry[i].lots.as_slice())
            .unwrap_or(&[]);

        let mut rows: Vec<(Option<chrono::NaiveDate>, ExpiryLotRow)> = Vec::new();
        for (li, lot) in lots_src.iter().enumerate() {
            let date = lot.date.as_deref().and_then(expiry::from_iso);
            let days = date.map(|d| expiry::days_until(d, today));
            let status = expiry::lot_status(days);
            match status {
                0 => fill += 1,
                1 => {
                    expired += 1;
                    *alerts.entry(cat.to_string()).or_default() += 1;
                }
                2 => {
                    soon += 1;
                    *alerts.entry(cat.to_string()).or_default() += 1;
                }
                _ => {}
            }
            let rel = match days {
                Some(d) => expiry::rel_text(d, lang_en),
                None if lang_en => "to fill".to_string(),
                None => "à renseigner".to_string(),
            };
            rows.push((
                date,
                ExpiryLotRow {
                    lot_index: li as i32,
                    qty: lot.qty,
                    date_text: date.map(expiry::to_display).unwrap_or_default().into(),
                    rel_text: rel.into(),
                    status,
                },
            ));
        }
        rows.sort_by_key(|(d, _)| *d);
        let total: i32 = rows.iter().map(|(_, r)| r.qty).sum();
        let lot_rows: Vec<ExpiryLotRow> = rows.into_iter().map(|(_, r)| r).collect();

        cat_map
            .entry(cat.to_string())
            .or_default()
            .push(ExpiryEntry {
                key: AppState::checked_key(cat, name).into(),
                name: name.to_string().into(),
                lots: ModelRc::new(VecModel::from(lot_rows)),
                total,
            });
    }

    for items in cat_map.values_mut() {
        items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }

    let mut ordered: Vec<String> = CAT_ORDER.iter().map(|s| s.to_string()).collect();
    for key in cat_map.keys() {
        if !ordered.contains(key) {
            ordered.push(key.clone());
        }
    }

    let mut groups: Vec<ExpiryGroup> = Vec::new();
    for cat_name in &ordered {
        if let Some(items) = cat_map.remove(cat_name) {
            groups.push(ExpiryGroup {
                name: cat_name.clone().into(),
                icon: cat_icon(cat_name).into(),
                color: hex_color(cat_color_hex(cat_name)),
                items: ModelRc::new(VecModel::from(items)),
                alerts: *alerts.get(cat_name).unwrap_or(&0),
            });
        }
    }

    let mut excluded_rows: Vec<ExcludedRow> = Vec::new();
    for cat in &state.expiry_excl_cats {
        let count = all.iter().filter(|(c, _)| *c == cat.as_str()).count() as i32;
        excluded_rows.push(ExcludedRow {
            key: cat.clone().into(),
            label: format!("{} {}", cat_icon(cat), cat).into(),
            is_cat: true,
            count,
        });
    }
    excluded_items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    excluded_rows.extend(excluded_items);
    let excluded_count = excluded_rows.len() as i32;

    ExpiryModels {
        groups: ModelRc::new(VecModel::from(groups)),
        excluded: ModelRc::new(VecModel::from(excluded_rows)),
        excluded_count,
        expired,
        soon,
        fill,
    }
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
