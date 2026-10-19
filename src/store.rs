//! Local profile, settings and library, saved as small text files in the user data directory.
//!
//! `profile.txt`: one line, the display name. `settings.txt`: `theme=light|dark`, `lang=ru|en`.
//! Projects and folders: see `library_io.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::i18n::Lang;
use crate::library::Library;
use crate::library_io::{folders_text, parse_folders, parse_projects, projects_text};

pub const NAME_MAX_CHARS: usize = 20;

pub struct Store {
    dir: PathBuf,
    pub name: Option<String>,
    pub lib: Library,
    pub dark: bool,
    pub lang: Lang,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Keeps printable characters only and cuts the name to `NAME_MAX_CHARS` characters.
pub fn sanitize_name(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .take(NAME_MAX_CHARS)
        .collect()
}

/// Per-user data directory (`%APPDATA%\DesignVolt`, `~/.config/designvolt`, ...).
pub fn default_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("DesignVolt")
}

impl Store {
    pub fn load(dir: &Path) -> Self {
        let read = |file: &str| fs::read_to_string(dir.join(file)).unwrap_or_default();
        let mut store = Self::blank(dir);
        store.name = Some(read("profile.txt"))
            .map(|s| sanitize_name(s.trim_end_matches(['\r', '\n'])))
            .filter(|s| !s.trim().is_empty());
        store.lib.projects = parse_projects(&read("projects.txt"));
        store.lib.folders = parse_folders(&read("folders.txt"));
        store.apply_settings(&read("settings.txt"));
        store
    }

    /// Empty store with default settings (light theme, Russian) that was not loaded from disk.
    pub fn blank(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            name: None,
            lib: Library::default(),
            dark: false,
            lang: Lang::Ru,
        }
    }

    /// Reads `key=value` lines; unknown keys and bad values are ignored.
    fn apply_settings(&mut self, text: &str) {
        for line in text.lines() {
            match line.split_once('=') {
                Some(("theme", v)) => self.dark = v.trim() == "dark",
                Some(("lang", v)) => {
                    if let Some(l) = Lang::from_code(v) {
                        self.lang = l;
                    }
                }
                _ => {}
            }
        }
    }

    fn settings_text(&self) -> String {
        let theme = if self.dark { "dark" } else { "light" };
        format!("theme={theme}\nlang={}\n", self.lang.code())
    }

    pub fn save(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        write_atomic(&self.dir.join("settings.txt"), &self.settings_text())?;
        write_atomic(
            &self.dir.join("profile.txt"),
            self.name.as_deref().unwrap_or(""),
        )?;
        write_atomic(
            &self.dir.join("projects.txt"),
            &projects_text(&self.lib.projects),
        )?;
        write_atomic(
            &self.dir.join("folders.txt"),
            &folders_text(&self.lib.folders),
        )
    }

    /// Saves and reports failures on stderr (the UI keeps working with in-memory data).
    pub fn save_logged(&self) {
        if let Err(e) = self.save() {
            eprintln!("could not save data: {e}");
        }
    }

    pub fn set_name(&mut self, raw: &str) {
        self.name = Some(sanitize_name(raw));
    }
}

fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Item;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("designvolt-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn sanitize_limits_chars_not_bytes() {
        let long = "Привет мир, как дела? Отлично";
        assert_eq!(sanitize_name(long).chars().count(), NAME_MAX_CHARS);
        assert_eq!(sanitize_name("a\tb\nc"), "abc");
        assert_eq!(sanitize_name("日本語🎨"), "日本語🎨");
    }

    #[test]
    fn library_and_profile_roundtrip() {
        let dir = temp_dir("roundtrip");
        let mut s = Store::load(&dir);
        s.set_name("Вадим 🎨");
        let f = s.lib.create_folder(50, "Папка", None);
        let p = s.lib.create_project(100, "Проект", Some(f));
        s.lib.toggle_favorite(p);
        s.lib.trash(Item::Project(p), 7);
        s.save().unwrap();

        let back = Store::load(&dir);
        assert_eq!(back.name.as_deref(), Some("Вадим 🎨"));
        assert_eq!(back.lib.projects, s.lib.projects);
        assert_eq!(back.lib.folders, s.lib.folders);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_projects_file_is_migrated_on_load() {
        let dir = temp_dir("migrate");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("projects.txt"), "1\t10\t20\tЛегаси\n").unwrap();
        let s = Store::load(&dir);
        assert_eq!(s.lib.projects.len(), 1);
        assert_eq!(s.lib.projects[0].name, "Легаси");
        s.save().unwrap();
        assert_eq!(Store::load(&dir).lib.projects, s.lib.projects);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_or_blank_profile_means_first_run() {
        let dir = temp_dir("blank");
        assert!(Store::load(&dir).name.is_none());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("profile.txt"), "   \n").unwrap();
        assert!(Store::load(&dir).name.is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_default_to_light_and_russian() {
        let dir = temp_dir("defaults");
        let s = Store::load(&dir);
        assert!(!s.dark);
        assert_eq!(s.lang, Lang::Ru);
    }

    #[test]
    fn settings_roundtrip() {
        let dir = temp_dir("settings");
        let mut s = Store::load(&dir);
        s.dark = true;
        s.lang = Lang::En;
        s.save().unwrap();
        let back = Store::load(&dir);
        assert!(back.dark);
        assert_eq!(back.lang, Lang::En);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_settings_values_are_ignored() {
        let dir = temp_dir("badsettings");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("settings.txt"),
            "theme=dark\nlang=klingon\nnoise\n",
        )
        .unwrap();
        let s = Store::load(&dir);
        assert!(s.dark);
        assert_eq!(s.lang, Lang::Ru);
        let _ = fs::remove_dir_all(&dir);
    }
}
