//! The JNI side of the Todo widget's bridge (ADR 0014): what `WidgetBridge.kt` calls. Loading
//! this library from a broadcast receiver does not start Tauri, so everything here works with or
//! without the app in the process. Release builds abort on panic: nothing here may panic, unwrap
//! or index, and every failure becomes an `err` reply or a null string.

use std::path::PathBuf;
use std::sync::OnceLock;

use jni::objects::{GlobalRef, JClass, JString};
use jni::sys::jstring;
use jni::{JNIEnv, JavaVM};
use mnemora_core::logging::{self, LogHandle};
use mnemora_core::paths::AppPaths;
use tauri::AppHandle;

use super::handle;
use super::source::{Live, Standalone};
use crate::ENVIRONMENT;

/// The running app, once `setup` has made its state.
static APP: OnceLock<AppHandle<tauri::Wry>> = OnceLock::new();
/// The bridge used while there is no app in this process.
static STANDALONE: OnceLock<Standalone> = OnceLock::new();
/// Keeps the log file writer alive when the widget started logging before the app did.
static LOG: OnceLock<LogHandle> = OnceLock::new();
/// How to reach `WidgetBridge.onTodoChanged` from any thread.
static NOTIFIER: OnceLock<Notifier> = OnceLock::new();

struct Notifier {
    vm: JavaVM,
    class: GlobalRef,
}

/// Remembers the app, so bridge calls use its connection instead of opening their own.
pub fn register_app(app: AppHandle<tauri::Wry>) {
    let _ = APP.set(app);
}

/// Asks the Android side to redraw the Todo widgets. Quietly does nothing if it cannot.
pub fn notify_todo_changed() {
    let Some(notifier) = NOTIFIER.get() else {
        return;
    };
    let Ok(mut env) = notifier.vm.attach_current_thread() else {
        return;
    };
    if env
        .call_static_method(&notifier.class, "onTodoChanged", "()V", &[])
        .is_err()
    {
        // A pending Java exception would poison the thread's next JNI call.
        let _ = env.exception_clear();
    }
}

/// Answers one request from the Android side.
fn answer(data_dir: &str, request: &str) -> String {
    if let Some(app) = APP.get() {
        let live = Live::new(app.clone());
        if live.is_ready() {
            return handle(&live, request);
        }
    }
    let bridge = STANDALONE.get_or_init(|| {
        start_logging(data_dir);
        Standalone::new(ENVIRONMENT, PathBuf::from(data_dir))
    });
    handle(bridge, request)
}

/// The widget may start the process before Tauri: log to the same files the app uses.
fn start_logging(data_dir: &str) {
    let paths = AppPaths::resolve(ENVIRONMENT, std::path::Path::new(data_dir), None);
    if let Ok(log) = logging::init(&paths.log_dir, "info") {
        let _ = LOG.set(log);
    }
}

fn read(env: &mut JNIEnv, text: &JString) -> Option<String> {
    env.get_string(text).ok().map(String::from)
}

/// `WidgetBridge.nativeAttach()`: lets Rust call back into Kotlin from any thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_hemang_mnemora_widget_WidgetBridge_nativeAttach(
    env: JNIEnv,
    class: JClass,
) {
    let (Ok(vm), Ok(class)) = (env.get_java_vm(), env.new_global_ref(class)) else {
        return;
    };
    let _ = NOTIFIER.set(Notifier { vm, class });
}

/// `WidgetBridge.nativeCall(dataDir, request)`: one JSON request in, one JSON reply out. Null
/// means the call could not even be read or answered; the caller shows its read-error state.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_hemang_mnemora_widget_WidgetBridge_nativeCall(
    mut env: JNIEnv,
    _class: JClass,
    data_dir: JString,
    request: JString,
) -> jstring {
    let (Some(data_dir), Some(request)) = (read(&mut env, &data_dir), read(&mut env, &request))
    else {
        return std::ptr::null_mut();
    };
    let reply = answer(&data_dir, &request);
    env.new_string(reply)
        .map_or(std::ptr::null_mut(), JString::into_raw)
}
