use reqwest::blocking::Client;
use base64::{engine::general_purpose, Engine};

pub fn upload(url: &str, user: &str, pass: &str, body: String) {
    let client = Client::new();
    let auth = general_purpose::STANDARD.encode(format!("{user}:{pass}"));

    let _ = client.put(url)
        .header("Authorization", format!("Basic {}", auth))
        .body(body)
        .send();
}