//! Keyboard events shared by widgets and screens.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Enter,
    Escape,
    SelectAll,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    Insert,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
}
