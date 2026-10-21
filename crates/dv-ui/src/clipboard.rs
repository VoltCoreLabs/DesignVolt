//! System clipboard (text and images) via `arboard`. Failures are ignored: copy/paste just does nothing.

use std::cell::RefCell;

thread_local! {
    static CLIPBOARD: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
}

fn with_clipboard<T>(f: impl FnOnce(&mut arboard::Clipboard) -> Option<T>) -> Option<T> {
    CLIPBOARD.with(|c| {
        let mut slot = c.borrow_mut();
        if slot.is_none() {
            *slot = arboard::Clipboard::new().ok();
        }
        slot.as_mut().and_then(f)
    })
}

pub fn get_text() -> Option<String> {
    with_clipboard(|c| c.get_text().ok())
}

pub fn set_text(s: &str) {
    with_clipboard(|c| c.set_text(s.to_string()).ok());
}

/// An image on the clipboard as straight RGBA pixels (`width`, `height`, bytes), if there is one.
pub fn get_image() -> Option<(u32, u32, Vec<u8>)> {
    with_clipboard(|c| {
        let img = c.get_image().ok()?;
        Some((img.width as u32, img.height as u32, img.bytes.into_owned()))
    })
}

/// Empties the clipboard (so an old picture is not pasted after shapes were copied).
pub fn clear() {
    with_clipboard(|c| c.clear().ok());
}
