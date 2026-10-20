//! Recently used colors, shared by every project and kept in `recent_colors.txt`.

use std::cell::RefCell;

use dv_core::Color;

/// How many colors are remembered.
pub const MAX: usize = 18;

/// Puts `c` first; an equal color already in the list moves up instead of repeating.
pub fn push_into(list: &mut Vec<Color>, c: Color) {
    list.retain(|x| *x != c);
    list.insert(0, c);
    list.truncate(MAX);
}

pub fn format(list: &[Color]) -> String {
    list.iter()
        .map(|c| format!("{:02x}{:02x}{:02x}{:02x}\n", c.r, c.g, c.b, c.a))
        .collect()
}

pub fn parse(text: &str) -> Vec<Color> {
    let one = |l: &str| {
        let l = l.trim();
        let base = Color::from_hex(l.get(..6)?)?;
        let a = u8::from_str_radix(l.get(6..8)?, 16).ok()?;
        (l.len() == 8).then_some(Color { a, ..base })
    };
    text.lines().filter_map(one).take(MAX).collect()
}

thread_local! {
    static RECENT: RefCell<Option<Vec<Color>>> = const { RefCell::new(None) };
}

fn file() -> std::path::PathBuf {
    crate::store::default_dir().join("recent_colors.txt")
}

/// The remembered colors, newest first (read from disk on first use).
pub fn get() -> Vec<Color> {
    RECENT.with(|r| {
        r.borrow_mut()
            .get_or_insert_with(|| {
                if cfg!(test) {
                    return Vec::new();
                }
                parse(&std::fs::read_to_string(file()).unwrap_or_default())
            })
            .clone()
    })
}

/// Remembers a color the user has picked or typed.
pub fn push(c: Color) {
    let mut list = get();
    push_into(&mut list, c);
    if !cfg!(test) {
        let _ = std::fs::create_dir_all(crate::store::default_dir());
        let _ = crate::store::write_atomic(&file(), &format(&list));
    }
    RECENT.with(|r| *r.borrow_mut() = Some(list));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_first_without_repeats_and_capped() {
        let mut l = Vec::new();
        let (a, b) = (Color::rgb(1, 0, 0), Color::rgb(2, 0, 0));
        push_into(&mut l, a);
        push_into(&mut l, b);
        push_into(&mut l, a);
        assert_eq!(l, [a, b]);
        for i in 0..40 {
            push_into(&mut l, Color::rgb(i, 9, 9));
        }
        assert_eq!(l.len(), MAX);
        assert_eq!(l[0], Color::rgb(39, 9, 9));
    }

    #[test]
    fn text_roundtrip_keeps_alpha_and_skips_junk() {
        let l = [
            Color {
                a: 10,
                ..Color::rgb(255, 0, 128)
            },
            Color::rgb(0, 1, 2),
        ];
        assert_eq!(parse(&format(&l)), l);
        assert_eq!(
            parse("zz\n12\n\nff00ff00\n"),
            [Color {
                a: 0,
                ..Color::rgb(255, 0, 255)
            }]
        );
    }
}
