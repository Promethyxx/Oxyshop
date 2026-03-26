use slint::slint;
use serde::{Serialize, Deserialize};
use std::{fs, path::PathBuf};
use dirs::data_dir;

slint! {
export struct Item {
    name: string,
    cat: string,
    qty: int,
    obj: int,
}

export component App inherits Window {
    width: 420px;
    height: 720px;

    in-out property <[Item]> items;
    in-out property <int> tab: 0;

    callback inc(int);
    callback dec(int);
    callback toggle(int);
    callback remove(int);
    callback add();

    VerticalLayout {

        Text {
            text: tab == 0 ? "Stock" : tab == 1 ? "Courses" : "Config";
            font-size: 20px;
            horizontal-alignment: center;
        }

        if (tab == 0) : VerticalLayout {

            ListView {
                for items[i] in root.items : Rectangle {
                    height: 50px;

                    HorizontalLayout {
                        Text { text: items[i].name; width: 1*; }

                        Button { text: "-"; clicked => root.dec(i); }

                        Text { text: items[i].qty + "/" + items[i].obj; }

                        Button { text: "+"; clicked => root.inc(i); }

                        Button { text: "X"; clicked => root.remove(i); }
                    }
                }
            }

            Button {
                text: "Ajouter item test";
                clicked => root.add();
            }
        }

        if (tab == 1) : ListView {
            for items[i] in root.items : Rectangle {
                height: 50px;

                if (items[i].qty < items[i].obj) : HorizontalLayout {
                    Text { text: items[i].name; width: 1*; }
                    Text { text: "x" + (items[i].obj - items[i].qty); }
                    Button { text: "✔"; clicked => root.toggle(i); }
                }
            }
        }

        if (tab == 2) : VerticalLayout {
            Text { text: "Config (placeholder)"; }
        }

        HorizontalLayout {
            height: 50px;

            Button { text: "Stock"; clicked => root.tab = 0; }
            Button { text: "Courses"; clicked => root.tab = 1; }
            Button { text: "Config"; clicked => root.tab = 2; }
        }
    }
}
}

#[derive(Serialize, Deserialize, Clone)]
struct ItemData {
    name: String,
    cat: String,
    qty: i32,
    obj: i32,
}

fn file_path() -> PathBuf {
    let mut p = data_dir().unwrap_or(std::env::current_dir().unwrap());
    p.push("oxyshop.json");
    p
}

fn load() -> Vec<ItemData> {
    if let Ok(c) = fs::read_to_string(file_path()) {
        serde_json::from_str(&c).unwrap_or_default()
    } else {
        vec![
            ItemData { name: "Lait".into(), cat: "Food".into(), qty: 2, obj: 6 },
            ItemData { name: "Pâtes".into(), cat: "Food".into(), qty: 1, obj: 5 },
        ]
    }
}

fn save(data: &Vec<ItemData>) {
    let _ = fs::write(file_path(), serde_json::to_string_pretty(data).unwrap());
}

fn main() {
    let app = App::new().unwrap();

    let data = load();

    let model = std::rc::Rc::new(slint::VecModel::from(
        data.into_iter().map(|i| Item {
            name: i.name.into(),
            cat: i.cat.into(),
            qty: i.qty,
            obj: i.obj,
        }).collect::<Vec<_>>()
    ));

    app.set_items(model.clone().into());

    {
        let model = model.clone();
        app.on_inc(move |i| {
            if let Some(mut it) = model.row_data(i as usize) {
                it.qty += 1;
                model.set_row_data(i as usize, it);
            }
        });
    }

    {
        let model = model.clone();
        app.on_dec(move |i| {
            if let Some(mut it) = model.row_data(i as usize) {
                if it.qty > 0 { it.qty -= 1; }
                model.set_row_data(i as usize, it);
            }
        });
    }

    {
        let model = model.clone();
        app.on_toggle(move |i| {
            if let Some(mut it) = model.row_data(i as usize) {
                if it.qty < it.obj {
                    it.qty = it.obj;
                } else {
                    it.qty = 0;
                }
                model.set_row_data(i as usize, it);
            }
        });
    }

    {
        let model = model.clone();
        app.on_remove(move |i| {
            model.remove(i as usize);
        });
    }

    {
        let model = model.clone();
        app.on_add(move || {
            model.push(Item {
                name: "Nouvel item".into(),
                cat: "Misc".into(),
                qty: 0,
                obj: 5,
            });
        });
    }

    {
        let model = model.clone();
        app.on_close_requested(move || {
            let mut v = vec![];

            for i in 0..model.row_count() {
                if let Some(it) = model.row_data(i) {
                    v.push(ItemData {
                        name: it.name.to_string(),
                        cat: it.cat.to_string(),
                        qty: it.qty,
                        obj: it.obj,
                    });
                }
            }

            save(&v);
            CloseRequestResponse::Accept
        });
    }

    app.run().unwrap();
}