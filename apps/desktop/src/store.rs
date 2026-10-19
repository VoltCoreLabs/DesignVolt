//! Local profile, settings and library, saved as small text files in the user data directory.
//!
//! `profile.txt`: one line, the display name. `settings.txt`: `theme=light|dark`, `lang=ru|en`.
//! Projects and folders: see `library_io.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::i18n::Lang;
use crate::library::{Library, SortKey};
use crate::library_io::{folders_text, parse_folders, parse_projects, projects_text};

pub const NAME_MAX_CHARS: usize = 20;

pub struct Store {
    dir: PathBuf,
    pub name: Option<String>,
    pub lib: Library,
    pub dark: bool,
    pub lang: Lang,
    pub sort_key: SortKey,
    pub sort_asc: bool,
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
            sort_key: SortKey::Modified,
            sort_asc: false,
        }
    }

    /// Reads `key=value` lines; unknown keys and bad values are ignored.
    fn apply_settings(&mut self, text: &str) {
        for line in text.lines() {
            match line.split_once('=') {
                Some(("theme", v)) => self.dark = v.trim() == "dark",
                Some(("sort", v)) => {
                    if let Some(k) = SortKey::from_code(v) {
                        self.sort_key = k;
                    }
                }
                Some(("order", v)) => self.sort_asc = v.trim() == "asc",
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
        let order = if self.sort_asc { "asc" } else { "desc" };
        format!(
            "theme={theme}
lang={}
sort={}
order={order}
",
            self.lang.code(),
            self.sort_key.code()
        )
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

    /// Folder with the document files, one `<project id>.dvd` per project.
    pub fn documents_dir(&self) -> PathBuf {
        self.dir.join("documents")
    }

    pub fn document_path(&self, id: u64) -> PathBuf {
        self.documents_dir().join(format!("{id}.dvd"))
    }

    /// Copies the saved document of project `from` for project `to` (if `from` has one).
    pub fn copy_document(&self, from: u64, to: u64) {
        let (src, dst) = (self.document_path(from), self.document_path(to));
        if src.exists() {
            let _ = fs::create_dir_all(self.documents_dir());
            if let Err(e) = fs::copy(&src, &dst) {
                eprintln!("could not copy document {from} to {to}: {e}");
            }
        }
    }

    /// Removes the saved document of a deleted project.
    pub fn delete_document(&self, id: u64) {
        let _ = fs::remove_file(self.document_path(id));
    }

    pub fn set_name(&mut self, raw: &str) {
        self.name = Some(sanitize_name(raw));
    }
}

/// Writes a file through a temporary file, so a crash never leaves it half written.
pub fn write_atomic(path: &Path, content: &str) -> std::io::Result<()> {
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

    #[test]
    fn sort_defaults_and_roundtrip() {
        let dir = temp_dir("sort");
        let mut s = Store::load(&dir);
        assert_eq!(s.sort_key, SortKey::Modified);
        assert!(!s.sort_asc);
        s.sort_key = SortKey::Name;
        s.sort_asc = true;
        s.save().unwrap();
        let back = Store::load(&dir);
        assert_eq!(back.sort_key, SortKey::Name);
        assert!(back.sort_asc);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_sort_values_fall_back_to_defaults() {
        let dir = temp_dir("badsort");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("settings.txt"), "sort=size\norder=sideways\n").unwrap();
        let s = Store::load(&dir);
        assert_eq!(s.sort_key, SortKey::Modified);
        assert!(!s.sort_asc);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn documents_are_copied_and_deleted_by_project_id() {
        let dir = temp_dir("docs");
        let s = Store::blank(&dir);
        fs::create_dir_all(s.documents_dir()).unwrap();
        fs::write(s.document_path(1), "content").unwrap();
        s.copy_document(1, 2);
        assert_eq!(fs::read_to_string(s.document_path(2)).unwrap(), "content");
        s.copy_document(99, 3);
        assert!(
            !s.document_path(3).exists(),
            "nothing to copy from a missing document"
        );
        s.delete_document(1);
        assert!(!s.document_path(1).exists());
        assert!(s.document_path(2).exists());
        s.delete_document(1);
        let _ = fs::remove_dir_all(&dir);
    }
}
