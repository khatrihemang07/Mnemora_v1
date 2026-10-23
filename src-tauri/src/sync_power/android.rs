//! The JNI side of Sync's lifecycle (#74): what `SyncBridge.kt` calls when the activity comes to
//! the front or leaves, and when the network changes. Called on the Android UI or a connectivity
//! thread: nothing here blocks, and release builds abort on panic, so nothing may unwrap or index.

use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use jni::JNIEnv;
use jni::objects::JClass;
use jni::sys::{jboolean, jint};
use tauri::AppHandle;

use super::Link;

static APP: OnceLock<AppHandle<tauri::Wry>> = OnceLock::new();

/// What was reported, kept even before the app exists (Kotlin reports from `onCreate`, before
/// Tauri's setup).
#[derive(Clone, Copy)]
pub struct Early {
    pub front: bool,
    pub link: Link,
}

static EARLY: Mutex<Early> = Mutex::new(Early {
    front: true,
    link: Link::Unknown,
});

fn early_mut() -> MutexGuard<'static, Early> {
    EARLY.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the Android side has said so far.
pub fn early() -> Early {
    *early_mut()
}

/// From now on reports act on the running app.
pub fn register_app(app: AppHandle<tauri::Wry>) {
    let _ = APP.set(app);
}

/// `SyncBridge.nativeForeground(front)`: `onResume` is true, `onPause` false.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_hemang_mnemora_SyncBridge_nativeForeground(
    _env: JNIEnv,
    _class: JClass,
    front: jboolean,
) {
    let front = front != 0;
    early_mut().front = front;
    if let Some(app) = APP.get() {
        super::foreground(app, front);
    }
}

/// `SyncBridge.nativeLink(code)`: 0 offline, 1 Wi-Fi or other, 2 mobile data. Sent whenever the
/// network changes, also when the kind stays the same.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_hemang_mnemora_SyncBridge_nativeLink(
    _env: JNIEnv,
    _class: JClass,
    code: jint,
) {
    let link = Link::from_code(code);
    early_mut().link = link;
    if let Some(app) = APP.get() {
        super::link(app, link);
    }
}
