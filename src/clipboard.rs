//! System clipboard (text only) via `arboard`. Failures are ignored: copy/paste just does nothing.

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
