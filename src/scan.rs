use crate::camera::{self, Camera};
use crate::dates::Candidate;
use crate::ocr::{self, Scanner};
use crate::platform::{self, Platform};
use crate::AppWindow;
use image::{imageops, ImageFormat, RgbImage};
use slint::{ComponentHandle, Image, ModelRc, Rgb8Pixel, SharedPixelBuffer, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

const ROI_X: f32 = 0.06;
const ROI_Y: f32 = 0.40;
const ROI_W: f32 = 0.88;
const ROI_H: f32 = 0.20;
const WEAK_SCORE: i32 = 20;
const MAX_CHOICES: usize = 4;

struct ScanOutput {
    generation: u32,
    result: Result<Vec<Candidate>, String>,
}

struct State {
    generation: u32,
    plat: Option<Platform>,
    camera: Option<Camera>,
    camera_failed: bool,
    asked_permission: bool,
    scanner: Option<Arc<Scanner>>,
    models_rx: Option<mpsc::Receiver<Result<Scanner, String>>>,
    waiting_still: bool,
    scan_deadline: Option<Instant>,
    scanning: bool,
    scan_tx: mpsc::Sender<ScanOutput>,
    scan_rx: mpsc::Receiver<ScanOutput>,
}

fn tr(ui: &AppWindow, en: &str, fr: &str) -> SharedString {
    if ui.get_lang_en() { en.into() } else { fr.into() }
}

fn load_scanner(plat: &Platform) -> Result<Scanner, String> {
    let read = |stem: &str| {
        plat.read_asset(&format!("{stem}.rten"))
            .or_else(|_| plat.read_asset(&format!("{stem}.onnx")))
    };
    Scanner::from_bytes(read("text-detection")?, read("text-recognition")?)
}

fn crop_roi(jpeg: &[u8], rotation: i32) -> Result<RgbImage, String> {
    let img = image::load_from_memory_with_format(jpeg, ImageFormat::Jpeg)
        .map_err(|e| e.to_string())?
        .into_rgb8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let (data, rw, rh) = camera::rotate_rgb(img.as_raw(), w, h, rotation);
    let rgb = RgbImage::from_raw(rw as u32, rh as u32, data).ok_or("bad image buffer")?;
    let (w, h) = (rgb.width() as f32, rgb.height() as f32);
    Ok(imageops::crop_imm(
        &rgb,
        (w * ROI_X) as u32,
        (h * ROI_Y) as u32,
        (w * ROI_W) as u32,
        (h * ROI_H) as u32,
    )
    .to_image())
}

fn process_stills(jpegs: &[Vec<u8>], rotation: i32, scanner: &Scanner) -> Result<Vec<Candidate>, String> {
    let mut best: Option<(f64, RgbImage)> = None;
    let mut last_err = String::from("no image");
    for jpeg in jpegs {
        match crop_roi(jpeg, rotation) {
            Ok(roi) => {
                let s = ocr::sharpness(&roi);
                if best.as_ref().is_none_or(|(b, _)| s > *b) {
                    best = Some((s, roi));
                }
            }
            Err(e) => last_err = e,
        }
    }
    let (_, roi) = best.ok_or(last_err)?;
    let mut log = Vec::new();
    scanner.scan_with_fallback(&roi, &mut log)
}

fn to_slint(rgb: &[u8], w: u32, h: u32) -> Image {
    Image::from_rgb8(SharedPixelBuffer::<Rgb8Pixel>::clone_from_slice(rgb, w, h))
}

fn set_results(ui: &AppWindow, dates: Vec<String>) {
    let items: Vec<SharedString> = dates.into_iter().map(Into::into).collect();
    ui.set_scan_results(ModelRc::new(VecModel::from(items)));
}

pub fn setup(ui: &AppWindow) {
    ui.set_scan_available(true);
    ui.set_scan_roi_x(ROI_X);
    ui.set_scan_roi_y(ROI_Y);
    ui.set_scan_roi_w(ROI_W);
    ui.set_scan_roi_h(ROI_H);

    let (scan_tx, scan_rx) = mpsc::channel();
    let state = Rc::new(RefCell::new(State {
        generation: 0,
        plat: None,
        camera: None,
        camera_failed: false,
        asked_permission: false,
        scanner: None,
        models_rx: None,
        waiting_still: false,
        scan_deadline: None,
        scanning: false,
        scan_tx,
        scan_rx,
    }));
    let timer = Rc::new(Timer::default());

    let tick: Rc<dyn Fn()> = Rc::new({
        let ui_weak = ui.as_weak();
        let state = state.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();

            let models = st.models_rx.as_ref().and_then(|rx| rx.try_recv().ok());
            if let Some(res) = models {
                st.models_rx = None;
                match res {
                    Ok(s) => {
                        st.scanner = Some(Arc::new(s));
                        ui.set_scan_status(tr(&ui, "Aim the date inside the green frame", "Cadre la date dans le cadre vert"));
                    }
                    Err(e) => ui.set_scan_status(format!("OCR: {e}").into()),
                }
            }

            if st.camera.is_none() && !st.camera_failed {
                let plat = st.plat.clone();
                if let Some(plat) = plat {
                    if plat.has_camera_permission() {
                        match Camera::start() {
                            Ok(c) => st.camera = Some(c),
                            Err(e) => {
                                st.camera_failed = true;
                                ui.set_scan_status(format!("Camera: {e}").into());
                            }
                        }
                    } else if !st.asked_permission {
                        st.asked_permission = true;
                        plat.request_camera_permission();
                        ui.set_scan_status(tr(&ui, "Camera permission needed", "Autorisation caméra nécessaire"));
                    }
                }
            }

            if let Some(f) = st.camera.as_ref().and_then(|c| c.take_preview()) {
                ui.set_scan_preview(to_slint(&f.rgb, f.width, f.height));
            }

            if st.waiting_still {
                let expired = st.scan_deadline.is_some_and(|d| Instant::now() >= d);
                let min = if expired { 1 } else { camera::BURST };
                let still = st.camera.as_ref().and_then(|c| c.take_stills(min).map(|j| (j, c.rotation())));
                if expired && still.is_none() {
                    st.waiting_still = false;
                    st.scanning = false;
                    ui.set_scan_busy(false);
                    ui.set_scan_status(tr(&ui, "No image received from the camera", "Aucune image reçue de la caméra"));
                }
                if let (Some((jpegs, rotation)), Some(scanner)) = (still, st.scanner.clone()) {
                    st.waiting_still = false;
                    ui.set_scan_status(tr(&ui, "Reading the date…", "Lecture de la date…"));
                    let tx = st.scan_tx.clone();
                    let generation = st.generation;
                    thread::spawn(move || {
                        let result = process_stills(&jpegs, rotation, &scanner);
                        let _ = tx.send(ScanOutput { generation, result });
                    });
                }
            }

            while let Ok(out) = st.scan_rx.try_recv() {
                if out.generation != st.generation {
                    continue;
                }
                st.scanning = false;
                ui.set_scan_busy(false);
                match out.result {
                    Ok(list) if !list.is_empty() => {
                        let weak = list[0].score < WEAK_SCORE;
                        let dates = list.iter().take(MAX_CHOICES).map(|c| c.date.format("%d.%m.%Y").to_string()).collect();
                        set_results(&ui, dates);
                        ui.set_scan_status(if weak {
                            tr(&ui, "Uncertain, please check the date", "Incertain, vérifie la date")
                        } else {
                            tr(&ui, "Tap the right date to use it", "Touche la bonne date pour l'utiliser")
                        });
                    }
                    Ok(_) => {
                        set_results(&ui, Vec::new());
                        ui.set_scan_status(tr(&ui, "No date found: get closer or hold steadier", "Aucune date trouvée : rapproche-toi ou stabilise"));
                    }
                    Err(e) => ui.set_scan_status(format!("Scan: {e}").into()),
                }
            }
        }
    });

    let close: Rc<dyn Fn()> = Rc::new({
        let ui_weak = ui.as_weak();
        let state = state.clone();
        let timer = timer.clone();
        move || {
            timer.stop();
            {
                let mut st = state.borrow_mut();
                st.generation += 1;
                st.camera = None;
                st.waiting_still = false;
                st.scanning = false;
            }
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_scan_active(false);
                ui.set_scan_busy(false);
            }
        }
    });

    ui.on_lot_modal_scan({
        let ui_weak = ui.as_weak();
        let state = state.clone();
        let timer = timer.clone();
        let tick = tick.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let Some(plat) = platform::get() else { return };
            let mut st = state.borrow_mut();
            st.generation += 1;
            st.plat = Some(plat.clone());
            st.camera = None;
            st.camera_failed = false;
            st.asked_permission = false;
            st.waiting_still = false;
            st.scanning = false;
            if st.scanner.is_none() && st.models_rx.is_none() {
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || {
                    let _ = tx.send(load_scanner(&plat));
                });
                st.models_rx = Some(rx);
            }
            let ready = st.scanner.is_some();
            drop(st);
            ui.set_scan_preview(Image::default());
            set_results(&ui, Vec::new());
            ui.set_scan_busy(false);
            ui.set_scan_status(if ready {
                tr(&ui, "Aim the date inside the green frame", "Cadre la date dans le cadre vert")
            } else {
                tr(&ui, "Loading OCR models…", "Chargement des modèles OCR…")
            });
            ui.set_scan_active(true);
            let tick = tick.clone();
            timer.start(TimerMode::Repeated, Duration::from_millis(40), move || tick());
        }
    });

    ui.on_scan_shoot({
        let ui_weak = ui.as_weak();
        let state = state.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if st.scanning {
                return;
            }
            if st.scanner.is_none() {
                ui.set_scan_status(tr(&ui, "OCR models not ready yet", "Modèles OCR pas encore prêts"));
                return;
            }
            let Some(cam) = st.camera.as_mut() else {
                ui.set_scan_status(tr(&ui, "Camera not ready", "Caméra pas prête"));
                return;
            };
            match cam.capture() {
                Ok(()) => {
                    st.waiting_still = true;
                    st.scan_deadline = Some(Instant::now() + Duration::from_millis(2500));
                    st.scanning = true;
                    ui.set_scan_busy(true);
                    set_results(&ui, Vec::new());
                    ui.set_scan_status(tr(&ui, "Capturing…", "Capture…"));
                }
                Err(e) => ui.set_scan_status(format!("Capture: {e}").into()),
            }
        }
    });

    ui.on_scan_cancel({
        let close = close.clone();
        move || close()
    });

    ui.on_scan_pick({
        let ui_weak = ui.as_weak();
        let close = close.clone();
        move |date| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_lot_modal_date_str(date);
            }
            close();
        }
    });
}
