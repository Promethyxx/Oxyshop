use crate::dates::{self, Candidate};
use chrono::Local;
use image::{imageops, Rgb, RgbImage};
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::{Dimension, Model};

pub struct Scanner {
    engine: OcrEngine,
    aspect: f32,
}

fn edge_color(img: &RgbImage) -> Rgb<u8> {
    let (w, h) = img.dimensions();
    let mut sum = [0u64; 3];
    let mut n = 0u64;
    for x in (0..w).step_by(8) {
        for y in [0, h - 1] {
            let p = img.get_pixel(x, y).0;
            for c in 0..3 {
                sum[c] += p[c] as u64;
            }
            n += 1;
        }
    }
    let n = n.max(1);
    Rgb([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8])
}

pub fn letterbox(img: &RgbImage, aspect: f32) -> RgbImage {
    let (w, h) = img.dimensions();
    let cur = w as f32 / h as f32;
    if (cur / aspect - 1.0).abs() < 0.2 {
        return img.clone();
    }
    let (cw, ch) = if cur > aspect {
        (w, (w as f32 / aspect).round() as u32)
    } else {
        ((h as f32 * aspect).round() as u32, h)
    };
    let mut canvas = RgbImage::from_pixel(cw, ch, edge_color(img));
    imageops::overlay(&mut canvas, img, ((cw - w) / 2) as i64, ((ch - h) / 2) as i64);
    canvas
}

fn tiles(img: &RgbImage, n: u32) -> Vec<(String, RgbImage)> {
    let (w, h) = img.dimensions();
    let step_w = w / (n + 1);
    let step_h = h / (n + 1);
    let (tw, th) = (step_w * 2, step_h * 2);
    let mut out = Vec::new();
    for r in 0..n {
        for c in 0..n {
            let crop = imageops::crop_imm(img, c * step_w, r * step_h, tw, th).to_image();
            let scale = (1400 / tw.max(th).max(1)).clamp(1, 4);
            let tile = if scale > 1 {
                imageops::resize(&crop, tw * scale, th * scale, imageops::FilterType::Lanczos3)
            } else {
                crop
            };
            out.push((format!("tile {r},{c}"), tile));
        }
    }
    out
}

fn luma(p: &[u8; 3]) -> u32 {
    (p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000
}

pub fn autocontrast(img: &RgbImage) -> RgbImage {
    let mut hist = [0u32; 256];
    for p in img.pixels() {
        hist[luma(&p.0) as usize] += 1;
    }
    let total: u32 = hist.iter().sum();
    let (mut lo, mut hi, mut acc) = (0usize, 255usize, 0u32);
    for (i, c) in hist.iter().enumerate() {
        acc += c;
        if acc * 50 >= total {
            lo = i;
            break;
        }
    }
    acc = 0;
    for (i, c) in hist.iter().enumerate().rev() {
        acc += c;
        if acc * 50 >= total {
            hi = i;
            break;
        }
    }
    if hi <= lo + 20 {
        return img.clone();
    }
    let scale = 255.0 / (hi - lo) as f32;
    let mut out = img.clone();
    for p in out.pixels_mut() {
        for c in 0..3 {
            p.0[c] = ((p.0[c] as f32 - lo as f32) * scale).clamp(0.0, 255.0) as u8;
        }
    }
    out
}

pub fn sharpness(img: &RgbImage) -> f64 {
    let (w, h) = img.dimensions();
    if w < 3 || h < 3 {
        return 0.0;
    }
    let (mut sum, mut n) = (0f64, 0f64);
    for y in (1..h - 1).step_by(2) {
        for x in (1..w - 1).step_by(2) {
            let c = luma(&img.get_pixel(x, y).0) as i32;
            let l = luma(&img.get_pixel(x - 1, y).0) as i32;
            let r = luma(&img.get_pixel(x + 1, y).0) as i32;
            let u = luma(&img.get_pixel(x, y - 1).0) as i32;
            let d = luma(&img.get_pixel(x, y + 1).0) as i32;
            let lap = (4 * c - l - r - u - d) as f64;
            sum += lap * lap;
            n += 1.0;
        }
    }
    sum / n.max(1.0)
}

fn halves(img: &RgbImage, tag: &str, v: &mut Vec<(String, RgbImage)>) {
    let (w, h) = img.dimensions();
    if w as f32 / h as f32 > 1.8 {
        let part = (w as f32 * 0.56) as u32;
        v.push((format!("{tag}-left"), imageops::crop_imm(img, 0, 0, part, h).to_image()));
        v.push((format!("{tag}-right"), imageops::crop_imm(img, w - part, 0, part, h).to_image()));
    }
}

fn variants(img: &RgbImage, tile_grid: u32) -> Vec<(String, RgbImage)> {
    let (w, h) = img.dimensions();
    let mut v = vec![("original".to_string(), img.clone())];
    let sigma = (w as f32 / 900.0).max(1.2);
    let blurred = autocontrast(&imageops::blur(img, sigma));
    v.push(("blur".to_string(), blurred.clone()));
    if w > 1000 {
        let nh = ((h as f32) * 1000.0 / (w as f32)).max(1.0) as u32;
        v.push(("small".to_string(), autocontrast(&imageops::resize(img, 1000, nh, imageops::FilterType::Triangle))));
    }
    halves(img, "original", &mut v);
    halves(&blurred, "blur", &mut v);
    let mut inv = img.clone();
    imageops::invert(&mut inv);
    v.push(("inverted".to_string(), inv));
    if tile_grid > 0 {
        v.extend(tiles(img, tile_grid));
    }
    v
}

impl Scanner {
    pub fn from_bytes(detection: Vec<u8>, recognition: Vec<u8>) -> Result<Self, String> {
        let det = Model::load(detection).map_err(|e| e.to_string())?;
        let rec = Model::load(recognition).map_err(|e| e.to_string())?;
        let aspect = match det.input_shape(0).as_deref() {
            Some([_, _, Dimension::Fixed(h), Dimension::Fixed(w)]) if *h > 0 => *w as f32 / *h as f32,
            _ => 1.0,
        };
        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(det),
            recognition_model: Some(rec),
            ..Default::default()
        })
        .map_err(|e| e.to_string())?;
        Ok(Self { engine, aspect })
    }

    pub fn text(&self, img: &RgbImage) -> Result<String, String> {
        let img = &letterbox(img, self.aspect);
        let src = ImageSource::from_bytes(img.as_raw(), img.dimensions()).map_err(|e| e.to_string())?;
        let input = self.engine.prepare_input(src).map_err(|e| e.to_string())?;
        let words = self.engine.detect_words(&input).map_err(|e| e.to_string())?;
        let lines = self.engine.find_text_lines(&input, &words);
        let texts = self.engine.recognize_text(&input, &lines).map_err(|e| e.to_string())?;
        Ok(texts
            .into_iter()
            .flatten()
            .map(|l| l.to_string())
            .filter(|l| l.trim().len() > 1)
            .collect::<Vec<_>>()
            .join("\n"))
    }

    pub fn scan(&self, img: &RgbImage, tile_grid: u32, log: &mut Vec<String>) -> Result<Vec<Candidate>, String> {
        let today = Local::now().date_naive();
        let mut all: Vec<Candidate> = Vec::new();
        for (name, v) in variants(img, tile_grid) {
            let text = self.text(&v)?;
            log.push(format!("variant {name}\n--- OCR ---\n{text}\n-----------"));
            let bonus = if name == "original" { 5 } else { 0 };
            for mut c in dates::extract(&text, today) {
                c.score += bonus;
                match all.iter_mut().find(|x| x.date == c.date) {
                    Some(x) => x.score += c.score,
                    None => all.push(c),
                }
            }
        }
        all.sort_by(|a, b| b.score.cmp(&a.score).then(b.date.cmp(&a.date)));
        Ok(all)
    }

    pub fn scan_with_fallback(&self, img: &RgbImage, log: &mut Vec<String>) -> Result<Vec<Candidate>, String> {
        let first = self.scan(img, 0, log)?;
        if !first.is_empty() {
            return Ok(first);
        }
        self.scan(img, 3, log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_grid_covers_image() {
        let img = RgbImage::new(1500, 2000);
        let t = tiles(&img, 3);
        assert_eq!(t.len(), 9);
        assert!(t.iter().all(|(_, i)| i.width().max(i.height()) >= 1000));
        assert_eq!(tiles(&img, 5).len(), 25);
    }
}

#[cfg(test)]
mod letterbox_tests {
    use super::*;

    #[test]
    fn pads_wide_images_to_aspect() {
        let img = RgbImage::from_pixel(3000, 1000, Rgb([200, 200, 200]));
        let out = letterbox(&img, 0.75);
        assert_eq!(out.dimensions(), (3000, 4000));
        assert_eq!(out.get_pixel(1500, 2000).0, [200, 200, 200]);
        assert_eq!(out.get_pixel(1500, 10).0, [200, 200, 200]);
        let same = RgbImage::new(1500, 2000);
        assert_eq!(letterbox(&same, 0.75).dimensions(), (1500, 2000));
    }
}
