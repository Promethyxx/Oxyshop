use serde::{Serialize, Deserialize};
use std::{fs, path::PathBuf};
use dirs::data_dir;

#[derive(Serialize, Deserialize, Clone)]
pub struct Item {
    pub name: String,
    pub cat: String,
    pub qty: i32,
    pub obj: i32,
}

fn path() -> PathBuf {
    let mut p = data_dir().unwrap_or(std::env::current_dir().unwrap());
    p.push("oxyshop.json");
    p
}

pub fn load_data() -> Vec<Item> {
    if let Ok(c) = fs::read_to_string(path()) {
        serde_json::from_str(&c).unwrap_or_default()
    } else {
        vec![]
    }
}

pub fn save_data(d: &Vec<Item>) {
    let _ = fs::write(path(), serde_json::to_string_pretty(d).unwrap());
}

// ⚠️ on évite dépendance directe au type Slint ici
pub fn data_to_ui(data: &Vec<Item>) -> Vec<(String, String, i32, i32)> {
    data.iter()
        .map(|i| (i.name.clone(), i.cat.clone(), i.qty, i.obj))
        .collect()
}