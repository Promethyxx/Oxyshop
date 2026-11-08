use crate::data::{AppState, ExpiryItem, ExpiryLot};
use chrono::{Datelike, Local, Months, NaiveDate};

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn to_iso(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub fn to_display(d: NaiveDate) -> String {
    d.format("%d.%m.%Y").to_string()
}

pub fn from_iso(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

pub fn quick_date(kind: i32, from: NaiveDate) -> Option<NaiveDate> {
    match kind {
        0 => from.checked_add_days(chrono::Days::new(3)),
        1 => from.checked_add_days(chrono::Days::new(7)),
        _ => from.checked_add_months(Months::new(1)),
    }
}

fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    if !(1..=12).contains(&month) {
        return None;
    }
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(ny, nm, 1)?.pred_opt()
}

fn expand_year(y: i32) -> i32 {
    if y < 100 { 2000 + y } else { y }
}

pub fn parse_date(input: &str, today: NaiveDate) -> Option<NaiveDate> {
    let input = input.trim();
    if !input.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    let parts: Vec<&str> = input
        .split(|c: char| matches!(c, '.' | '/' | '-' | ' ' | ','))
        .filter(|p| !p.is_empty())
        .collect();
    let nums: Vec<i32> = parts
        .iter()
        .map(|p| p.parse::<i32>().ok())
        .collect::<Option<Vec<i32>>>()?;
    match nums.as_slice() {
        [a, b, c] => {
            if parts[0].len() == 4 {
                NaiveDate::from_ymd_opt(*a, u32::try_from(*b).ok()?, u32::try_from(*c).ok()?)
            } else {
                NaiveDate::from_ymd_opt(
                    expand_year(*c),
                    u32::try_from(*b).ok()?,
                    u32::try_from(*a).ok()?,
                )
            }
        }
        [a, b] => {
            if parts[0].len() == 4 {
                last_day_of_month(*a, u32::try_from(*b).ok()?)
            } else if *b >= 100 {
                last_day_of_month(*b, u32::try_from(*a).ok()?)
            } else if *b <= 12 {
                let day = u32::try_from(*a).ok()?;
                let month = u32::try_from(*b).ok()?;
                match NaiveDate::from_ymd_opt(today.year(), month, day) {
                    Some(d) if d >= today => Some(d),
                    _ => NaiveDate::from_ymd_opt(today.year() + 1, month, day),
                }
            } else {
                last_day_of_month(expand_year(*b), u32::try_from(*a).ok()?)
            }
        }
        _ => None,
    }
}

pub fn days_until(date: NaiveDate, today: NaiveDate) -> i64 {
    date.signed_duration_since(today).num_days()
}

pub fn lot_status(days: Option<i64>) -> i32 {
    match days {
        None => 0,
        Some(d) if d < 0 => 1,
        Some(d) if d <= 7 => 2,
        Some(_) => 3,
    }
}

pub fn rel_text(days: i64, en: bool) -> String {
    match (days, en) {
        (d, false) if d < 0 => format!("expiré depuis {} j", -d),
        (d, true) if d < 0 => format!("expired {}d ago", -d),
        (0, false) => "aujourd'hui".to_string(),
        (0, true) => "today".to_string(),
        (1, false) => "demain".to_string(),
        (1, true) => "tomorrow".to_string(),
        (d, false) => format!("dans {} j", d),
        (d, true) => format!("in {}d", d),
    }
}

impl AppState {
    pub fn expiry_is_excluded(&self, cat: &str, name: &str) -> bool {
        self.expiry_excl_cats.iter().any(|c| c == cat)
            || self
                .expiry_excl_items
                .contains(&AppState::checked_key(cat, name))
    }

    pub fn expiry_find(&self, cat: &str, name: &str) -> Option<usize> {
        self.expiry
            .iter()
            .position(|e| e.cat == cat && e.name == name)
    }

    pub fn expiry_find_key(&self, key: &str) -> Option<usize> {
        self.expiry
            .iter()
            .position(|e| AppState::checked_key(&e.cat, &e.name) == key)
    }

    pub fn expiry_resolve_key(&self, key: &str) -> Option<(String, String)> {
        self.stock
            .iter()
            .map(|s| (&s.cat, &s.name))
            .chain(self.expiry.iter().map(|e| (&e.cat, &e.name)))
            .find(|(c, n)| AppState::checked_key(c, n) == key)
            .map(|(c, n)| (c.clone(), n.clone()))
    }

    fn expiry_entry_mut(&mut self, cat: &str, name: &str) -> &mut ExpiryItem {
        let idx = match self.expiry_find(cat, name) {
            Some(i) => i,
            None => {
                self.expiry.push(ExpiryItem {
                    name: name.to_string(),
                    cat: cat.to_string(),
                    lots: Vec::new(),
                });
                self.expiry.len() - 1
            }
        };
        &mut self.expiry[idx]
    }

    pub fn expiry_prune(&mut self) {
        for e in &mut self.expiry {
            e.lots.retain(|l| l.qty > 0);
        }
        self.expiry.retain(|e| !e.lots.is_empty());
    }

    pub fn expiry_seed(&mut self) {
        if self.expiry_init {
            return;
        }
        let seeds: Vec<(String, String, i32)> = self
            .stock
            .iter()
            .filter(|s| s.qty > 0 && !self.expiry_is_excluded(&s.cat, &s.name))
            .map(|s| (s.cat.clone(), s.name.clone(), s.qty))
            .collect();
        for (cat, name, qty) in seeds {
            if self.expiry_find(&cat, &name).is_none() {
                self.expiry.push(ExpiryItem {
                    name,
                    cat,
                    lots: vec![ExpiryLot { qty, date: None }],
                });
            }
        }
        self.expiry_init = true;
    }

    pub fn expiry_on_stock_increase(&mut self, cat: &str, name: &str, n: i32) {
        if n <= 0 || self.expiry_is_excluded(cat, name) {
            return;
        }
        let lots = &mut self.expiry_entry_mut(cat, name).lots;
        match lots.iter_mut().find(|l| l.date.is_none()) {
            Some(l) => l.qty += n,
            None => lots.push(ExpiryLot { qty: n, date: None }),
        }
    }

    pub fn expiry_on_stock_undo(&mut self, cat: &str, name: &str, n: i32) {
        let Some(idx) = self.expiry_find(cat, name) else {
            return;
        };
        let mut left = n;
        for l in self.expiry[idx].lots.iter_mut().rev() {
            if l.date.is_none() && left > 0 {
                let taken = left.min(l.qty);
                l.qty -= taken;
                left -= taken;
            }
        }
        self.expiry_prune();
    }

    pub fn expiry_rename(&mut self, old_cat: &str, old_name: &str, new_cat: &str, new_name: &str) {
        if old_cat == new_cat && old_name == new_name {
            return;
        }
        let old_key = AppState::checked_key(old_cat, old_name);
        let new_key = AppState::checked_key(new_cat, new_name);
        if let Some(i) = self.expiry_find(old_cat, old_name) {
            let mut moved = self.expiry.remove(i);
            match self.expiry_find(new_cat, new_name) {
                Some(j) => self.expiry[j].lots.append(&mut moved.lots),
                None => {
                    moved.cat = new_cat.to_string();
                    moved.name = new_name.to_string();
                    self.expiry.push(moved);
                }
            }
        }
        for k in &mut self.expiry_excl_items {
            if *k == old_key {
                *k = new_key.clone();
            }
        }
    }

    pub fn expiry_add_lot(&mut self, cat: &str, name: &str, qty: i32, date: Option<NaiveDate>) {
        self.expiry_entry_mut(cat, name).lots.push(ExpiryLot {
            qty,
            date: date.map(to_iso),
        });
    }

    pub fn expiry_set_lot(
        &mut self,
        key: &str,
        lot: usize,
        qty: i32,
        date: Option<NaiveDate>,
    ) -> bool {
        let Some(idx) = self.expiry_find_key(key) else {
            return false;
        };
        let Some(l) = self.expiry[idx].lots.get_mut(lot) else {
            return false;
        };
        l.qty = qty;
        l.date = date.map(to_iso);
        true
    }

    pub fn expiry_delete_lot(&mut self, key: &str, lot: usize) -> bool {
        let Some(idx) = self.expiry_find_key(key) else {
            return false;
        };
        if lot >= self.expiry[idx].lots.len() {
            return false;
        }
        self.expiry[idx].lots.remove(lot);
        self.expiry_prune();
        true
    }

    pub fn expiry_retire(&mut self, key: &str, lot: usize, n: i32) -> Option<i32> {
        let idx = self.expiry_find_key(key)?;
        let cat = self.expiry[idx].cat.clone();
        let name = self.expiry[idx].name.clone();
        let l = self.expiry[idx].lots.get_mut(lot)?;
        let taken = n.min(l.qty).max(0);
        l.qty -= taken;
        self.expiry_prune();
        if taken > 0 && !self.expiry_is_excluded(&cat, &name) {
            if let Some(s) = self
                .stock
                .iter_mut()
                .find(|s| s.cat == cat && s.name == name)
            {
                s.qty = (s.qty - taken).max(0);
            }
        }
        Some(taken)
    }

    pub fn expiry_exclude_item(&mut self, cat: &str, name: &str) {
        let key = AppState::checked_key(cat, name);
        if !self.expiry_excl_items.contains(&key) {
            self.expiry_excl_items.push(key);
        }
    }

    pub fn expiry_exclude_cat(&mut self, cat: &str) {
        if !self.expiry_excl_cats.iter().any(|c| c == cat) {
            self.expiry_excl_cats.push(cat.to_string());
        }
    }

    pub fn expiry_include(&mut self, key: &str, is_cat: bool) {
        if is_cat {
            self.expiry_excl_cats.retain(|c| c != key);
        } else {
            self.expiry_excl_items.retain(|k| k != key);
        }
        let seeds: Vec<(String, String, i32)> = self
            .stock
            .iter()
            .filter(|s| s.qty > 0 && !self.expiry_is_excluded(&s.cat, &s.name))
            .filter(|s| {
                if is_cat {
                    s.cat == key
                } else {
                    AppState::checked_key(&s.cat, &s.name) == key
                }
            })
            .filter(|s| {
                self.expiry_find(&s.cat, &s.name)
                    .is_none_or(|i| self.expiry[i].lots.is_empty())
            })
            .map(|s| (s.cat.clone(), s.name.clone(), s.qty))
            .collect();
        for (cat, name, qty) in seeds {
            self.expiry_add_lot(&cat, &name, qty, None);
        }
    }
}
