use jni::objects::{JObject, JValue};
use jni::{jni_sig, jni_str, Env, JavaVM};
use slint::android::AndroidApp;
use std::ffi::CString;
use std::mem::ManuallyDrop;
use std::sync::Mutex;

const CAMERA_PERMISSION: &str = "android.permission.CAMERA";

static PLATFORM: Mutex<Option<Platform>> = Mutex::new(None);

pub fn init(app: AndroidApp) {
    if let Ok(mut slot) = PLATFORM.lock() {
        *slot = Some(Platform::new(app));
    }
}

pub fn get() -> Option<Platform> {
    PLATFORM.lock().ok().and_then(|slot| slot.clone())
}

#[derive(Clone)]
pub struct Platform {
    app: AndroidApp,
}

impl Platform {
    pub fn new(app: AndroidApp) -> Self {
        Self { app }
    }

    pub fn read_asset(&self, name: &str) -> Result<Vec<u8>, String> {
        let manager = self.app.asset_manager();
        let cname = CString::new(name).map_err(|e| e.to_string())?;
        let mut asset = manager.open(&cname).ok_or_else(|| format!("asset not found: {name}"))?;
        let buf = asset.buffer().map_err(|e| e.to_string())?;
        Ok(buf.to_vec())
    }

    fn with_activity<R>(&self, f: impl FnOnce(&mut Env, &JObject) -> jni::errors::Result<R>) -> Option<R> {
        let vm = unsafe { JavaVM::from_raw(self.app.vm_as_ptr() as *mut _) };
        vm.attach_current_thread(|env| {
            let activity = ManuallyDrop::new(unsafe { JObject::from_raw(env, self.app.activity_as_ptr() as jni::sys::jobject) });
            f(env, &activity)
        })
        .ok()
    }

    pub fn has_camera_permission(&self) -> bool {
        self.with_activity(|env, activity| {
            let perm = env.new_string(CAMERA_PERMISSION)?;
            let res = env.call_method(
                activity,
                jni_str!("checkSelfPermission"),
                jni_sig!("(Ljava/lang/String;)I"),
                &[JValue::Object(perm.as_ref())],
            )?;
            res.i()
        })
        .is_some_and(|granted| granted == 0)
    }

    pub fn request_camera_permission(&self) {
        let _ = self.with_activity(|env, activity| {
            let perm = env.new_string(CAMERA_PERMISSION)?;
            let class = env.find_class(jni_str!("java/lang/String"))?;
            let array = env.new_object_array(1, &class, &perm)?;
            env.call_method(
                activity,
                jni_str!("requestPermissions"),
                jni_sig!("([Ljava/lang/String;I)V"),
                &[JValue::Object(array.as_ref()), JValue::Int(1)],
            )?;
            Ok(())
        });
    }
}
