use ndk_sys::*;
use std::ffi::{CStr, CString};
use std::os::raw::{c_int, c_void};
use std::ptr;
use std::sync::{Arc, Mutex};

const FORMAT_YUV: i32 = 35;
const FORMAT_JPEG: i32 = 256;
const AF_CONTINUOUS_PICTURE: u8 = 4;
const FACING_BACK: u8 = 1;
const MAX_PREVIEW_WIDTH: i32 = 1280;
const JPEG_QUALITY: u8 = 100;
const HIGH_QUALITY_MODE: u8 = 2;
pub const BURST: usize = 3;

pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

struct Shared {
    preview: Mutex<Option<Frame>>,
    still: Mutex<Vec<Vec<u8>>>,
    rotation: i32,
}

pub struct Camera {
    shared: Arc<Shared>,
    manager: *mut ACameraManager,
    device: *mut ACameraDevice,
    session: *mut ACameraCaptureSession,
    preview_reader: *mut AImageReader,
    still_reader: *mut AImageReader,
    container: *mut ACaptureSessionOutputContainer,
    outputs: Vec<*mut ACaptureSessionOutput>,
    targets: Vec<*mut ACameraOutputTarget>,
    preview_request: *mut ACaptureRequest,
    still_request: *mut ACaptureRequest,
    _device_callbacks: Box<ACameraDevice_StateCallbacks>,
    _session_callbacks: Box<ACameraCaptureSession_stateCallbacks>,
    _preview_listener: Box<AImageReader_ImageListener>,
    _still_listener: Box<AImageReader_ImageListener>,
    _id: CString,
}

fn cam_ok(s: camera_status_t, what: &str) -> Result<(), String> {
    if s == camera_status_t::ACAMERA_OK {
        Ok(())
    } else {
        Err(format!("{what}: camera error {}", s.0))
    }
}

fn media_ok(s: media_status_t, what: &str) -> Result<(), String> {
    if s == media_status_t::AMEDIA_OK {
        Ok(())
    } else {
        Err(format!("{what}: media error {}", s.0))
    }
}

pub fn pick_sizes(configs: &[i32]) -> Option<((i32, i32), (i32, i32))> {
    let mut yuv: Vec<(i32, i32)> = Vec::new();
    let mut jpeg: Vec<(i32, i32)> = Vec::new();
    for c in configs.chunks_exact(4) {
        if c[3] != 0 {
            continue;
        }
        match c[0] {
            FORMAT_YUV => yuv.push((c[1], c[2])),
            FORMAT_JPEG => jpeg.push((c[1], c[2])),
            _ => {}
        }
    }
    let still = *jpeg
        .iter()
        .filter(|(w, h)| w * 3 == h * 4)
        .max_by_key(|(w, h)| w * h)
        .or_else(|| jpeg.iter().max_by_key(|(w, h)| w * h))?;
    let ratio = still.0 as f32 / still.1 as f32;
    let preview = *yuv
        .iter()
        .filter(|(w, h)| *w <= MAX_PREVIEW_WIDTH && ((*w as f32 / *h as f32) - ratio).abs() < 0.02)
        .max_by_key(|(w, h)| w * h)
        .or_else(|| yuv.iter().filter(|(w, _)| *w <= MAX_PREVIEW_WIDTH).max_by_key(|(w, h)| w * h))?;
    Some((preview, still))
}

pub fn rotate_rgb(src: &[u8], w: usize, h: usize, rotation: i32) -> (Vec<u8>, usize, usize) {
    match rotation.rem_euclid(360) {
        90 => {
            let mut out = vec![0u8; src.len()];
            for y in 0..h {
                for x in 0..w {
                    let d = (x * h + (h - 1 - y)) * 3;
                    let s = (y * w + x) * 3;
                    out[d..d + 3].copy_from_slice(&src[s..s + 3]);
                }
            }
            (out, h, w)
        }
        180 => {
            let mut out = vec![0u8; src.len()];
            for i in 0..w * h {
                let d = (w * h - 1 - i) * 3;
                out[d..d + 3].copy_from_slice(&src[i * 3..i * 3 + 3]);
            }
            (out, w, h)
        }
        270 => {
            let mut out = vec![0u8; src.len()];
            for y in 0..h {
                for x in 0..w {
                    let d = ((w - 1 - x) * h + y) * 3;
                    let s = (y * w + x) * 3;
                    out[d..d + 3].copy_from_slice(&src[s..s + 3]);
                }
            }
            (out, h, w)
        }
        _ => (src.to_vec(), w, h),
    }
}

fn clamp(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

unsafe fn plane(img: *mut AImage, idx: c_int) -> Option<(&'static [u8], usize, usize)> {
    unsafe {
        let mut data: *mut u8 = ptr::null_mut();
        let mut len: c_int = 0;
        let mut row: i32 = 0;
        let mut pix: i32 = 0;
        if AImage_getPlaneData(img, idx, &mut data, &mut len) != media_status_t::AMEDIA_OK || data.is_null() {
            return None;
        }
        if AImage_getPlaneRowStride(img, idx, &mut row) != media_status_t::AMEDIA_OK {
            return None;
        }
        if AImage_getPlanePixelStride(img, idx, &mut pix) != media_status_t::AMEDIA_OK {
            return None;
        }
        Some((std::slice::from_raw_parts(data, len as usize), row as usize, pix as usize))
    }
}

unsafe fn yuv_to_frame(img: *mut AImage, rotation: i32) -> Option<Frame> {
    unsafe {
        let (mut w, mut h) = (0i32, 0i32);
        AImage_getWidth(img, &mut w);
        AImage_getHeight(img, &mut h);
        let (y, yrs, yps) = plane(img, 0)?;
        let (u, urs, ups) = plane(img, 1)?;
        let (v, vrs, vps) = plane(img, 2)?;
        let sw = (w / 2) as usize;
        let sh = (h / 2) as usize;
        let mut rgb = vec![0u8; sw * sh * 3];
        for oy in 0..sh {
            let sy = oy * 2;
            for ox in 0..sw {
                let sx = ox * 2;
                let yi = sy * yrs + sx * yps;
                let ui = (sy / 2) * urs + (sx / 2) * ups;
                let vi = (sy / 2) * vrs + (sx / 2) * vps;
                if yi >= y.len() || ui >= u.len() || vi >= v.len() {
                    continue;
                }
                let yy = y[yi] as i32;
                let uu = u[ui] as i32 - 128;
                let vv = v[vi] as i32 - 128;
                let o = (oy * sw + ox) * 3;
                rgb[o] = clamp(yy + ((1436 * vv) >> 10));
                rgb[o + 1] = clamp(yy - ((352 * uu + 731 * vv) >> 10));
                rgb[o + 2] = clamp(yy + ((1815 * uu) >> 10));
            }
        }
        let (out, rw, rh) = rotate_rgb(&rgb, sw, sh, rotation);
        Some(Frame { width: rw as u32, height: rh as u32, rgb: out })
    }
}

unsafe extern "C" fn on_preview(ctx: *mut c_void, reader: *mut AImageReader) {
    unsafe {
        let shared = &*(ctx as *const Shared);
        let mut img: *mut AImage = ptr::null_mut();
        if AImageReader_acquireLatestImage(reader, &mut img) != media_status_t::AMEDIA_OK || img.is_null() {
            return;
        }
        if let Some(f) = yuv_to_frame(img, shared.rotation) {
            if let Ok(mut slot) = shared.preview.lock() {
                *slot = Some(f);
            }
        }
        AImage_delete(img);
    }
}

unsafe extern "C" fn on_still(ctx: *mut c_void, reader: *mut AImageReader) {
    unsafe {
        let shared = &*(ctx as *const Shared);
        loop {
            let mut img: *mut AImage = ptr::null_mut();
            if AImageReader_acquireNextImage(reader, &mut img) != media_status_t::AMEDIA_OK || img.is_null() {
                return;
            }
            let mut data: *mut u8 = ptr::null_mut();
            let mut len: c_int = 0;
            if AImage_getPlaneData(img, 0, &mut data, &mut len) == media_status_t::AMEDIA_OK && !data.is_null() {
                let bytes = std::slice::from_raw_parts(data, len as usize).to_vec();
                if let Ok(mut slot) = shared.still.lock() {
                    slot.push(bytes);
                }
            }
            AImage_delete(img);
        }
    }
}

unsafe extern "C" fn on_device_disconnected(_ctx: *mut c_void, _dev: *mut ACameraDevice) {}
unsafe extern "C" fn on_device_error(_ctx: *mut c_void, _dev: *mut ACameraDevice, _err: c_int) {}
unsafe extern "C" fn on_session_state(_ctx: *mut c_void, _s: *mut ACameraCaptureSession) {}

unsafe fn entry_i32(meta: *const ACameraMetadata, tag: u32) -> Option<Vec<i32>> {
    unsafe {
        let mut e: ACameraMetadata_const_entry = std::mem::zeroed();
        if ACameraMetadata_getConstEntry(meta, tag, &mut e) != camera_status_t::ACAMERA_OK || e.data.i32_.is_null() {
            return None;
        }
        Some(std::slice::from_raw_parts(e.data.i32_, e.count as usize).to_vec())
    }
}

unsafe fn entry_u8(meta: *const ACameraMetadata, tag: u32) -> Option<Vec<u8>> {
    unsafe {
        let mut e: ACameraMetadata_const_entry = std::mem::zeroed();
        if ACameraMetadata_getConstEntry(meta, tag, &mut e) != camera_status_t::ACAMERA_OK || e.data.u8_.is_null() {
            return None;
        }
        Some(std::slice::from_raw_parts(e.data.u8_, e.count as usize).to_vec())
    }
}

unsafe fn back_camera(manager: *mut ACameraManager) -> Result<CString, String> {
    unsafe {
        let mut list: *mut ACameraIdList = ptr::null_mut();
        cam_ok(ACameraManager_getCameraIdList(manager, &mut list), "getCameraIdList")?;
        let mut found: Option<CString> = None;
        let count = (*list).numCameras.max(0) as usize;
        for i in 0..count {
            let id = CStr::from_ptr(*(*list).cameraIds.add(i));
            let mut meta: *mut ACameraMetadata = ptr::null_mut();
            if ACameraManager_getCameraCharacteristics(manager, id.as_ptr(), &mut meta) != camera_status_t::ACAMERA_OK {
                continue;
            }
            let facing = entry_u8(meta, acamera_metadata_tag::ACAMERA_LENS_FACING.0);
            ACameraMetadata_free(meta);
            if facing.and_then(|f| f.first().copied()) == Some(FACING_BACK) {
                found = Some(id.to_owned());
                break;
            }
        }
        ACameraManager_deleteCameraIdList(list);
        found.ok_or_else(|| "no back camera".to_string())
    }
}

impl Camera {
    pub fn start() -> Result<Camera, String> {
        unsafe { Self::start_inner() }
    }

    unsafe fn start_inner() -> Result<Camera, String> {
        unsafe {
            let manager = ACameraManager_create();
            if manager.is_null() {
                return Err("ACameraManager_create failed".into());
            }
            let id = back_camera(manager)?;
            let mut meta: *mut ACameraMetadata = ptr::null_mut();
            cam_ok(ACameraManager_getCameraCharacteristics(manager, id.as_ptr(), &mut meta), "characteristics")?;
            let configs = entry_i32(meta, acamera_metadata_tag::ACAMERA_SCALER_AVAILABLE_STREAM_CONFIGURATIONS.0);
            let orientation = entry_i32(meta, acamera_metadata_tag::ACAMERA_SENSOR_ORIENTATION.0)
                .and_then(|v| v.first().copied())
                .unwrap_or(90);
            ACameraMetadata_free(meta);
            let configs = configs.ok_or("no stream configurations")?;
            let ((pw, ph), (sw, sh)) = pick_sizes(&configs).ok_or("no usable stream sizes")?;

            let shared = Arc::new(Shared {
                preview: Mutex::new(None),
                still: Mutex::new(Vec::new()),
                rotation: orientation,
            });
            let ctx = Arc::as_ptr(&shared) as *mut c_void;

            let mut device_callbacks = Box::new(ACameraDevice_StateCallbacks {
                context: ctx,
                onDisconnected: Some(on_device_disconnected),
                onError: Some(on_device_error),
            });
            let mut device: *mut ACameraDevice = ptr::null_mut();
            cam_ok(ACameraManager_openCamera(manager, id.as_ptr(), &mut *device_callbacks, &mut device), "openCamera")?;

            let mut preview_reader: *mut AImageReader = ptr::null_mut();
            media_ok(AImageReader_new(pw, ph, FORMAT_YUV, 3, &mut preview_reader), "preview reader")?;
            let mut still_reader: *mut AImageReader = ptr::null_mut();
            media_ok(AImageReader_new(sw, sh, FORMAT_JPEG, 4, &mut still_reader), "still reader")?;

            let mut preview_listener = Box::new(AImageReader_ImageListener { context: ctx, onImageAvailable: Some(on_preview) });
            let mut still_listener = Box::new(AImageReader_ImageListener { context: ctx, onImageAvailable: Some(on_still) });
            media_ok(AImageReader_setImageListener(preview_reader, &mut *preview_listener), "preview listener")?;
            media_ok(AImageReader_setImageListener(still_reader, &mut *still_listener), "still listener")?;

            let mut preview_window: *mut ANativeWindow = ptr::null_mut();
            let mut still_window: *mut ANativeWindow = ptr::null_mut();
            media_ok(AImageReader_getWindow(preview_reader, &mut preview_window), "preview window")?;
            media_ok(AImageReader_getWindow(still_reader, &mut still_window), "still window")?;

            let mut container: *mut ACaptureSessionOutputContainer = ptr::null_mut();
            cam_ok(ACaptureSessionOutputContainer_create(&mut container), "output container")?;
            let mut outputs = Vec::new();
            for w in [preview_window, still_window] {
                let mut out: *mut ACaptureSessionOutput = ptr::null_mut();
                cam_ok(ACaptureSessionOutput_create(w, &mut out), "session output")?;
                cam_ok(ACaptureSessionOutputContainer_add(container, out), "add output")?;
                outputs.push(out);
            }

            let session_callbacks = Box::new(ACameraCaptureSession_stateCallbacks {
                context: ctx,
                onClosed: Some(on_session_state),
                onReady: Some(on_session_state),
                onActive: Some(on_session_state),
            });
            let mut session: *mut ACameraCaptureSession = ptr::null_mut();
            cam_ok(ACameraDevice_createCaptureSession(device, container, &*session_callbacks, &mut session), "createCaptureSession")?;

            let mut targets = Vec::new();
            let mut make_request = |template: ACameraDevice_request_template, window: *mut ANativeWindow| -> Result<*mut ACaptureRequest, String> {
                let mut req: *mut ACaptureRequest = ptr::null_mut();
                cam_ok(ACameraDevice_createCaptureRequest(device, template, &mut req), "createCaptureRequest")?;
                let mut target: *mut ACameraOutputTarget = ptr::null_mut();
                cam_ok(ACameraOutputTarget_create(window, &mut target), "output target")?;
                cam_ok(ACaptureRequest_addTarget(req, target), "add target")?;
                targets.push(target);
                let af = AF_CONTINUOUS_PICTURE;
                cam_ok(ACaptureRequest_setEntry_u8(req, acamera_metadata_tag::ACAMERA_CONTROL_AF_MODE.0, 1, &af), "af mode")?;
                let fps = [24i32, 30i32];
                cam_ok(
                    ACaptureRequest_setEntry_i32(req, acamera_metadata_tag::ACAMERA_CONTROL_AE_TARGET_FPS_RANGE.0, 2, fps.as_ptr()),
                    "fps range",
                )?;
                Ok(req)
            };
            let mut preview_request = make_request(ACameraDevice_request_template::TEMPLATE_PREVIEW, preview_window)?;
            let still_request = make_request(ACameraDevice_request_template::TEMPLATE_STILL_CAPTURE, still_window)?;
            let quality = JPEG_QUALITY;
            let hq = HIGH_QUALITY_MODE;
            cam_ok(ACaptureRequest_setEntry_u8(still_request, acamera_metadata_tag::ACAMERA_JPEG_QUALITY.0, 1, &quality), "jpeg quality")?;
            cam_ok(ACaptureRequest_setEntry_u8(still_request, acamera_metadata_tag::ACAMERA_EDGE_MODE.0, 1, &hq), "edge mode")?;
            cam_ok(ACaptureRequest_setEntry_u8(still_request, acamera_metadata_tag::ACAMERA_NOISE_REDUCTION_MODE.0, 1, &hq), "noise reduction")?;

            let mut seq: c_int = 0;
            cam_ok(
                ACameraCaptureSession_setRepeatingRequest(session, ptr::null_mut(), 1, &mut preview_request, &mut seq),
                "setRepeatingRequest",
            )?;

            Ok(Camera {
                shared,
                manager,
                device,
                session,
                preview_reader,
                still_reader,
                container,
                outputs,
                targets,
                preview_request,
                still_request,
                _device_callbacks: device_callbacks,
                _session_callbacks: session_callbacks,
                _preview_listener: preview_listener,
                _still_listener: still_listener,
                _id: id,
            })
        }
    }

    pub fn rotation(&self) -> i32 {
        self.shared.rotation
    }

    pub fn take_preview(&self) -> Option<Frame> {
        self.shared.preview.lock().ok()?.take()
    }

    pub fn capture(&mut self) -> Result<(), String> {
        if let Ok(mut s) = self.shared.still.lock() {
            s.clear();
        }
        let mut seq: c_int = 0;
        let mut reqs = [self.still_request; BURST];
        unsafe {
            cam_ok(
                ACameraCaptureSession_capture(self.session, ptr::null_mut(), BURST as c_int, reqs.as_mut_ptr(), &mut seq),
                "capture",
            )
        }
    }

    pub fn take_stills(&self, min: usize) -> Option<Vec<Vec<u8>>> {
        let mut slot = self.shared.still.lock().ok()?;
        if slot.len() < min {
            return None;
        }
        Some(std::mem::take(&mut *slot))
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        unsafe {
            ACameraCaptureSession_stopRepeating(self.session);
            ACameraCaptureSession_close(self.session);
            ACameraDevice_close(self.device);
            for t in &self.targets {
                ACameraOutputTarget_free(*t);
            }
            ACaptureRequest_free(self.preview_request);
            ACaptureRequest_free(self.still_request);
            for o in &self.outputs {
                ACaptureSessionOutput_free(*o);
            }
            ACaptureSessionOutputContainer_free(self.container);
            AImageReader_delete(self.preview_reader);
            AImageReader_delete(self.still_reader);
            ACameraManager_delete(self.manager);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_matching_sizes() {
        let cfg = [
            35, 1920, 1080, 0, 35, 1280, 960, 0, 35, 640, 480, 0, 35, 4000, 3000, 0, 256, 4000, 3000, 0, 256, 1920, 1080, 0, 256, 4080, 3072, 1,
        ];
        assert_eq!(pick_sizes(&cfg), Some(((1280, 960), (4000, 3000))));
    }

    #[test]
    fn rotation_maps_pixels() {
        let src = [1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6];
        let (out, w, h) = rotate_rgb(&src, 3, 2, 90);
        assert_eq!((w, h), (2, 3));
        let px: Vec<u8> = out.chunks(3).map(|p| p[0]).collect();
        assert_eq!(px, vec![4, 1, 5, 2, 6, 3]);
        let (out, w, h) = rotate_rgb(&src, 3, 2, 270);
        assert_eq!((w, h), (2, 3));
        let px: Vec<u8> = out.chunks(3).map(|p| p[0]).collect();
        assert_eq!(px, vec![3, 6, 2, 5, 1, 4]);
        let (out, _, _) = rotate_rgb(&src, 3, 2, 180);
        let px: Vec<u8> = out.chunks(3).map(|p| p[0]).collect();
        assert_eq!(px, vec![6, 5, 4, 3, 2, 1]);
    }
}
