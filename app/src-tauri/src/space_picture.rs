//! A picture of the page Space has open (docs/SPACE.md 11): what a body
//! wears when its page isn't live. WebKit takes it, of the page as it is
//! drawn, and it is handed on as a JPEG.
//!
//! Only the Mac can, so far: WebKit's snapshot is asked for by name, with no
//! binding crate, since the app needs nothing else of AppKit's.

use tauri::{Runtime, Webview};

/// How wide a picture is, in points. It is seen small, on a body's screen.
const WIDE: f64 = 640.0;

/// Takes a picture of what `view` shows and gives it, or `None`, to `done`.
/// Answers false where no picture can be taken at all.
pub fn take<R: Runtime>(
    view: &Webview<R>,
    done: impl Fn(Option<Vec<u8>>) + Send + 'static,
) -> bool {
    #[cfg(target_os = "macos")]
    {
        view.with_webview(move |platform| {
            // SAFETY: `inner` is this web view's live WKWebView, and the
            // closure runs on the main thread, where AppKit must be used.
            unsafe { mac::take(platform.inner(), WIDE, done) }
        })
        .is_ok()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (view, done, WIDE);
        false
    }
}

/// The number the window server knows `window` by, for a picture of that
/// window alone (the tour's).
#[cfg(target_os = "macos")]
pub fn window_number<R: Runtime>(window: &tauri::Window<R>) -> Option<isize> {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    let ns_window = window.ns_window().ok()?.cast::<AnyObject>();
    if ns_window.is_null() {
        return None;
    }
    // SAFETY: a live NSWindow; `windowNumber` only reads.
    Some(unsafe { msg_send![ns_window, windowNumber] })
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::c_void;

    use block2::RcBlock;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {
        static NSImageCompressionFactor: *const AnyObject;
    }

    /// NSBitmapImageFileTypeJPEG.
    const JPEG: usize = 3;

    /// # Safety
    /// `web_view` is a live WKWebView and this is the main thread.
    pub unsafe fn take(web_view: *mut c_void, wide: f64, done: impl Fn(Option<Vec<u8>>) + 'static) {
        let web_view = web_view.cast::<AnyObject>();
        let config: *mut AnyObject = msg_send![class!(WKSnapshotConfiguration), new];
        let width: *mut AnyObject = msg_send![class!(NSNumber), numberWithDouble: wide];
        let _: () = msg_send![config, setSnapshotWidth: width];
        let handler = RcBlock::new(move |image: *mut AnyObject, _error: *mut AnyObject| {
            // SAFETY: WebKit hands over an NSImage, or nil with an error.
            done(unsafe { jpeg(image) });
        });
        let _: () = msg_send![
            web_view,
            takeSnapshotWithConfiguration: config,
            completionHandler: &*handler
        ];
        let _: () = msg_send![config, release];
    }

    /// # Safety
    /// `image` is an NSImage or nil.
    unsafe fn jpeg(image: *mut AnyObject) -> Option<Vec<u8>> {
        if image.is_null() {
            return None;
        }
        let tiff: *mut AnyObject = msg_send![image, TIFFRepresentation];
        if tiff.is_null() {
            return None;
        }
        let rep: *mut AnyObject = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
        if rep.is_null() {
            return None;
        }
        let quality: *mut AnyObject = msg_send![class!(NSNumber), numberWithDouble: 0.62f64];
        let properties: *mut AnyObject = msg_send![
            class!(NSDictionary),
            dictionaryWithObject: quality,
            forKey: NSImageCompressionFactor
        ];
        let data: *mut AnyObject =
            msg_send![rep, representationUsingType: JPEG, properties: properties];
        if data.is_null() {
            return None;
        }
        let bytes: *const c_void = msg_send![data, bytes];
        let length: usize = msg_send![data, length];
        (!bytes.is_null() && length > 0)
            .then(|| std::slice::from_raw_parts(bytes.cast::<u8>(), length).to_vec())
    }
}
