// Microphone permission. On macOS the system asks the user once; we ask
// explicitly (AVFoundation) before any audio-device call so the prompt
// comes from a well-defined place and cpal never triggers it implicitly
// from a background thread. Other platforms need nothing.

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::c_void;
    use tokio::sync::oneshot;

    extern "C" {
        fn diddle_mic_status() -> i32;
        fn diddle_mic_request(cb: extern "C" fn(i32, *mut c_void), ctx: *mut c_void);
    }

    extern "C" fn on_answer(granted: i32, ctx: *mut c_void) {
        // Reclaim the boxed sender handed to the request.
        let tx: Box<oneshot::Sender<bool>> = unsafe { Box::from_raw(ctx as *mut _) };
        let _ = tx.send(granted != 0);
    }

    /// Ok(true) when capture is allowed, Ok(false) when the user denied
    /// it (now or earlier). Prompts only when undetermined.
    pub async fn ensure_access() -> bool {
        match unsafe { diddle_mic_status() } {
            3 => true,
            1 | 2 => false,
            _ => {
                let (tx, rx) = oneshot::channel::<bool>();
                let ctx = Box::into_raw(Box::new(tx)) as *mut c_void;
                unsafe { diddle_mic_request(on_answer, ctx) };
                rx.await.unwrap_or(false)
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub async fn ensure_access() -> bool {
        true
    }
}

pub use imp::ensure_access;

pub const DENIED_MESSAGE: &str = "Microphone access is off for Diddle. Enable it in System Settings → Privacy & Security → Microphone, then try again.";

#[cfg(all(test, target_os = "macos"))]
mod tests {
    #[test]
    fn shim_links_and_reports_a_status() {
        extern "C" {
            fn diddle_mic_status() -> i32;
        }
        let s = unsafe { diddle_mic_status() };
        assert!((0..=3).contains(&s), "unexpected TCC status {s}");
    }
}
