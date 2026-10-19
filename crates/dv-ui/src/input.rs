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
    Up,
    Down,
    Space,
    Tab,
    /// A layout-independent letter, digit, `=` or `-` (from the physical key), lowercase.
    Char(char),
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
}
