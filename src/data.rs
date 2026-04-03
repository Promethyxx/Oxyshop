use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockItem {
    pub name: String,
    pub cat: String,
    pub qty: i32,
    pub obj: i32,
}

// ── Meal data ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MealEntry {
    pub time: String,
    pub label: String,
    pub accent: String, // hex color e.g. "#E8A87C"
    pub content: String, // multi-line text, options separated by \n
}

pub const ACCENT_COLORS: &[&str] = &[
    "#E8A87C", "#85CDCA", "#D96459", "#C7B198", "#F2D388", "#B794D6",
];

pub fn accent_idx(hex: &str) -> i32 {
    ACCENT_COLORS.iter().position(|&c| c.eq_ignore_ascii_case(hex)).unwrap_or(0) as i32
}

pub fn default_meals() -> Vec<MealEntry> {
    let dinner = "🥩 Viande Rouge, 200g Pâtes, 150g Épinards, 1 cas H d'olive\n\
                  🐟 Thon à l'Huile, 200g Riz, 100g Poivrons. 1 cas H d'olive\n\
                  🐟 Saumon, 200g Pâtes complètes, 150g Carottes, 1 cas H de colza\n\
                  🫀 Foie de Bœuf, 200g Patates, 150g Épinards. 2 cas H d'olive\n\
                  🍳 Omelette Lard (3 œufs), 250g Haricots, Salade verte + Radis, 1 cas H de noix";
    vec![
        MealEntry { time: "08h".into(), label: "Petit-déjeuner".into(), accent: "#E8A87C".into(),
            content: "Bircher (30g avoine, 5cl lait, 1 yaourt, 1 noix Brésil, 1 cac arachide, 25g amandes)".into() },
        MealEntry { time: "10h".into(), label: "Collation".into(), accent: "#85CDCA".into(),
            content: "1 Kiwi, 2dl Lait, 2dl Jus de fruit".into() },
        MealEntry { time: "12h".into(), label: "Dîner".into(), accent: "#D96459".into(),
            content: dinner.into() },
        MealEntry { time: "16h".into(), label: "Goûter".into(), accent: "#C7B198".into(),
            content: "(Optionnel si faim) : Un peu de fruit ou noix, ou juste de l'eau".into() },
        MealEntry { time: "18h".into(), label: "Souper".into(), accent: "#D96459".into(),
            content: dinner.into() },
        MealEntry { time: "20h".into(), label: "Soir".into(), accent: "#F2D388".into(),
            content: "50g Fromage".into() },
    ]
}

// ── App state ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub stock: Vec<StockItem>,
    pub checked: HashMap<String, bool>, // "cat:name" -> bool
    #[serde(default)]
    pub meals: Vec<MealEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}

impl AppState {
    pub fn with_defaults() -> Self {
        Self {
            stock: default_stock(),
            checked: HashMap::new(),
            meals: default_meals(),
            last_modified: None,
        }
    }

    pub fn checked_key(cat: &str, name: &str) -> String {
        format!("{}:{}", cat, name)
    }
}

pub fn default_stock() -> Vec<StockItem> {
    vec![
        // Boissons
        s("Bieres pack","Boissons",1,1), s("Café","Boissons",1,1),
        s("Infusion Camomille","Boissons",0,1), s("Infusion Tilleul - Verveine","Boissons",0,1),
        s("Jus de fruits","Boissons",6,6), s("Lait","Boissons",6,6),
        s("Sirop","Boissons",0,2), s("Thé menthe, noir, vert","Boissons",0,1),
        s("Vin Rouge","Boissons",2,2), s("Yogi tea, Citron","Boissons",0,1),
        // Condiments
        s("Beurre","Condiments",0,1), s("Bouillons","Condiments",0,3),
        s("Crème","Condiments",2,2), s("Curry rouge","Condiments",0,1),
        s("Curry vert","Condiments",1,1), s("Maïzena","Condiments",0,1),
        s("Girofle","Condiments",0,1), s("Graines","Condiments",0,1),
        s("Huile Noix","Condiments",1,1), s("Huile Olive","Condiments",0,1),
        s("Huile Sésame","Condiments",1,1), s("Huile Tournesol","Condiments",0,1),
        s("Humus","Condiments",1,1), s("Jus de citron","Condiments",0,2),
        s("Ketchup","Condiments",0,1), s("Lait de coco","Condiments",0,2),
        s("Maggi","Condiments",0,1), s("Mayonnaise","Condiments",0,1),
        s("Moutarde","Condiments",1,1), s("Paprika fort","Condiments",0,1),
        s("Pesto calabrese","Condiments",0,4), s("Pesto rouge","Condiments",0,4),
        s("Pesto vert","Condiments",1,4), s("Sauce Piment","Condiments",0,1),
        s("Poivre","Condiments",0,1), s("Sauce Salade","Condiments",0,1),
        s("Sauce tomate","Condiments",0,4), s("Sel","Condiments",0,1),
        s("Soja","Condiments",0,2), s("Sucre brun","Condiments",0,1),
        s("Sucre glace","Condiments",0,2), s("Vinaigre","Condiments",0,1),
        s("Chocolat cuisine","Condiments",0,2),
        // Nourriture
        s("Ail","Nourriture",0,1), s("Cornichons","Nourriture",1,1),
        s("Flocon d'avoines","Nourriture",0,1), s("Fromage","Nourriture",1,1),
        s("Fromage rapé","Nourriture",1,1), s("Fromage tranche","Nourriture",0,1),
        s("Haricots","Nourriture",1,4), s("Lentilles","Nourriture",0,4),
        s("Œufs x10","Nourriture",0,1), s("Oignons","Nourriture",0,1),
        s("Patates","Nourriture",0,1), s("Pates","Nourriture",0,10),
        s("Petis maïs","Nourriture",0,1), s("Petis oignons","Nourriture",0,1),
        s("Pois chiches","Nourriture",0,2), s("Jambon de dinde","Nourriture",1,1),
        s("Poulpe","Nourriture",2,5), s("Purée","Nourriture",0,1),
        s("Riz","Nourriture",0,3), s("Semoule","Nourriture",0,1),
        s("Thon","Nourriture",1,5), s("Toast","Nourriture",0,1),
        s("Yaourt","Nourriture",1,1),
        // Snack
        s("Biscuits","Snack",2,2), s("Chips","Snack",2,2),
        s("Noix mélange","Snack",0,2), s("Nutella","Snack",0,1),
        s("Pistaches","Snack",1,1),
        // Menage
        s("Adoucissant","Menage",0,1), s("Chiffons","Menage",1,4),
        s("Détartrant machine","Menage",0,2), s("Eponges grandes","Menage",0,4),
        s("Eponges petites","Menage",0,2), s("Javel","Menage",0,1),
        s("Produit Lessive","Menage",0,1), s("Savon vaisselle","Menage",0,2),
        s("Twist","Menage",0,1), s("Vinaigre blanc","Menage",0,1),
        // SDB
        s("Bain de bouche","SDB",0,1), s("Brosse à dents","SDB",2,2),
        s("Cotton-tiges","SDB",0,1), s("Dentifrice","SDB",2,2),
        s("Dermovate","SDB",0,2), s("Désodorisant WC","SDB",0,1),
        s("Excipial","SDB",1,3), s("Fil dentaire","SDB",0,1),
        s("Gel douche","SDB",0,2), s("Huile corps","SDB",0,1),
        s("Papier toilettes","SDB",1,1), s("Prurimed","SDB",0,0),
        s("Rasoirs jetables","SDB",0,1), s("Savon d'Alep","SDB",1,1),
        s("Shampoing","SDB",0,1), s("WC bloc","SDB",1,1),
        // Stock
        s("Agraffes","Stock",0,1), s("Aluminium 50m","Stock",0,1),
        s("Bloc-notes","Stock",0,1), s("Briquets","Stock",0,3),
        s("Cigarettes","Stock",0,5), s("Cure-dents","Stock",0,1),
        s("Encre couleur","Stock",0,1), s("Encre noire","Stock",0,1),
        s("Etiquettes","Stock",0,1), s("Feuilles grandes","Stock",0,3),
        s("Feuilles petites","Stock",0,3), s("Filtre machine","Stock",0,2),
        s("Filtres","Stock",1,2), s("Fusibles","Stock",0,1),
        s("Gel combustible","Stock",1,1), s("Gel lubrifiant","Stock",0,1),
        s("Mouchoirs","Stock",1,1), s("Pansements","Stock",0,3),
        s("Papier film","Stock",0,1), s("Papier impression","Stock",0,1),
        s("Papier ménage","Stock",0,4), s("Papier sufurisé","Stock",0,1),
        s("Piles AA","Stock",0,1), s("Sac 20 L","Stock",0,1),
        s("Sac poubelles","Stock",0,1), s("Scotch","Stock",0,1),
        s("Scotch peau","Stock",0,2), s("Tabac","Stock",2,3),
        s("Touillettes","Stock",0,1), s("Verres carton","Stock",3,3),
    ]
}

fn s(name: &str, cat: &str, qty: i32, obj: i32) -> StockItem {
    StockItem { name: name.into(), cat: cat.into(), qty, obj }
}

// ── Category metadata ────────────────────────────────────────────────────────

pub fn cat_icon(cat: &str) -> &'static str {
    match cat {
        "Boissons"   => "🥤",
        "Condiments" => "🧂",
        "Nourriture" => "🍽️",
        "Snack"      => "🍪",
        "Menage"     => "🧹",
        "SDB"        => "🧴",
        "Stock"      => "📦",
        _            => "📁",
    }
}

/// Returns a hex color string for the category label in Courses view
pub fn cat_color_hex(cat: &str) -> &'static str {
    match cat {
        "Boissons"   => "#85CDCA",
        "Condiments" => "#E8A87C",
        "Nourriture" => "#D96459",
        "Snack"      => "#F2D388",
        "Menage"     => "#C7B198",
        "SDB"        => "#B794D6",
        _            => "rgba(232,228,222,0.3)",
    }
}

pub const CAT_ORDER: &[&str] = &[
    "Boissons", "Condiments", "Nourriture", "Snack", "Menage", "SDB", "Stock",
];
