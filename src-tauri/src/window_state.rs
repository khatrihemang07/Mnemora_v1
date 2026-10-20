//! Desktop window geometry: restore on launch, save (debounced) on change.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mnemora_core::device::{DeviceSettings, WindowState};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

use crate::state::{DeviceStore, lock};

/// Quiet period after the last move/resize before writing `device.toml`.
const DEBOUNCE: Duration = Duration::from_millis(500);

/// Applies saved size, position and maximized state to a window builder.
pub fn restore<'a>(
    handle: &AppHandle,
    mut builder: WebviewWindowBuilder<'a, tauri::Wry, AppHandle>,
    saved: &DeviceSettings,
) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
    let w = &saved.window;
    builder = builder.inner_size(f64::from(w.w), f64::from(w.h));
    match (w.x, w.y) {
        (Some(x), Some(y)) if is_on_screen(handle, x, y) => {
            builder = builder.position(f64::from(x), f64::from(y));
        }
        _ => builder = builder.center(),
    }
    builder.maximized(w.maximized)
}

/// True if the top-left area of the saved window lands on a connected monitor, so a
/// window saved on a since-unplugged display never opens off-screen.
fn is_on_screen(handle: &AppHandle, x: i32, y: i32) -> bool {
    let Ok(monitors) = handle.available_monitors() else {
        return false;
    };
    monitors.iter().any(|m| {
        let scale = m.scale_factor();
        let pos: LogicalPosition<f64> = m.position().to_logical(scale);
        let size: LogicalSize<f64> = m.size().to_logical(scale);
        let (px, py) = (f64::from(x) + 40.0, f64::from(y) + 10.0);
        px >= pos.x && px < pos.x + size.width && py >= pos.y && py < pos.y + size.height
    })
}

/// Logical size to whole pixels (at least 1). Window sizes are far below `u32::MAX`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_px(v: f64) -> u32 {
    v.round().clamp(1.0, f64::from(u32::MAX)) as u32
}

/// Logical position to whole pixels; may be negative on multi-monitor setups.
#[allow(clippy::cast_possible_truncation)]
fn to_coord(v: f64) -> i32 {
    v.round().clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

/// Reads the window's geometry; size and position are kept from `previous` while the
/// window is maximized, minimized or fullscreen so those states don't overwrite them.
fn capture(window: &WebviewWindow, previous: &WindowState) -> WindowState {
    let maximized = window.is_maximized().unwrap_or(false);
    let normal = !maximized
        && !window.is_minimized().unwrap_or(false)
        && !window.is_fullscreen().unwrap_or(false);
    if !normal {
        return WindowState {
            maximized,
            ..previous.clone()
        };
    }
    let scale = window.scale_factor().unwrap_or(1.0);
    let mut next = previous.clone();
    next.maximized = false;
    if let Ok(size) = window.inner_size() {
        let size: LogicalSize<f64> = size.to_logical(scale);
        next.w = to_px(size.width);
        next.h = to_px(size.height);
    }
    if let Ok(pos) = window.outer_position() {
        let pos: LogicalPosition<f64> = pos.to_logical(scale);
        next.x = Some(to_coord(pos.x));
        next.y = Some(to_coord(pos.y));
    }
    next
}

/// Starts saving window geometry whenever it changes (debounced) and on close (immediately).
pub fn track(window: &WebviewWindow, device: Arc<DeviceStore>) {
    let latest: Arc<Mutex<Option<WindowState>>> = Arc::new(Mutex::new(None));
    let (tx, rx) = mpsc::channel::<()>();

    let flush = {
        let latest = Arc::clone(&latest);
        let device = Arc::clone(&device);
        move || {
            if let Some(state) = lock(&latest).take()
                && let Err(e) = device.update(|d| d.window = state)
            {
                tracing::warn!(error = %e, "could not save window state");
            }
        }
    };

    // One long-lived thread coalesces bursts of events into a single write.
    let debounced = flush.clone();
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            loop {
                match rx.recv_timeout(DEBOUNCE) {
                    Ok(()) => {}
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => {
                        debounced();
                        return;
                    }
                }
            }
            debounced();
        }
    });

    let handle = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Resized(_) | WindowEvent::Moved(_) => {
            let previous = lock(&latest).clone().unwrap_or_else(|| device.get().window);
            *lock(&latest) = Some(capture(&handle, &previous));
            let _ = tx.send(());
        }
        WindowEvent::CloseRequested { .. } => {
            let previous = lock(&latest).clone().unwrap_or_else(|| device.get().window);
            *lock(&latest) = Some(capture(&handle, &previous));
            flush();
        }
        _ => {}
    });
}
