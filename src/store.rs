//! Local profile and project list, saved as small text files (no serde: not in the stack).
//!
//! `profile.txt`: one line, the display name. `projects.txt`: one project per line,
//! tab-separated `id, created, modified, name`, with `\\`, `\t`, `\n` escaped in the name.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::i18n::Lang;

pub const NAME_MAX_CHARS: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: u64,
    pub name: String,
    pub created: u64,
    pub modified: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Modified,
    Created,
}

pub struct Store {
    dir: PathBuf,
    pub name: Option<String>,
    pub projects: Vec<Project>,
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

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

fn parse_project(line: &str) -> Option<Project> {
    let mut parts = line.splitn(4, '\t');
    Some(Project {
        id: parts.next()?.parse().ok()?,
        created: parts.next()?.parse().ok()?,
        modified: parts.next()?.parse().ok()?,
        name: unescape(parts.next()?),
    })
}

impl Store {
    pub fn load(dir: &Path) -> Self {
        let name = fs::read_to_string(dir.join("profile.txt"))
            .ok()
            .map(|s| sanitize_name(s.trim_end_matches(['\r', '\n'])))
            .filter(|s| !s.trim().is_empty());
        let projects = fs::read_to_string(dir.join("projects.txt"))
            .map(|s| s.lines().filter_map(parse_project).collect())
            .unwrap_or_default();
        let mut store = Self::blank(dir);
        store.name = name;
        store.projects = projects;
        if let Ok(s) = fs::read_to_string(dir.join("settings.txt")) {
            store.apply_settings(&s);
        }
        store
    }

    /// Empty store with default settings (light theme, Russian) that was not loaded from disk.
    pub fn blank(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            name: None,
            projects: Vec::new(),
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
        let name = self.name.as_deref().unwrap_or("");
        write_atomic(&self.dir.join("profile.txt"), name)?;
        let body: String = self
            .projects
            .iter()
            .map(|p| {
                format!(
                    "{}\t{}\t{}\t{}\n",
                    p.id,
                    p.created,
                    p.modified,
                    escape(&p.name)
                )
            })
            .collect();
        write_atomic(&self.dir.join("projects.txt"), &body)
    }

    pub fn set_name(&mut self, raw: &str) {
        self.name = Some(sanitize_name(raw));
    }

    /// Adds "<base> N" with the lowest free N and returns its id.
    pub fn create_project(&mut self, now: u64, base: &str) -> u64 {
        let id = self.projects.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let mut n = 1;
        while self
            .projects
            .iter()
            .any(|p| p.name == format!("{base} {n}"))
        {
            n += 1;
        }
        self.projects.push(Project {
            id,
            name: format!("{base} {n}"),
            created: now,
            modified: now,
        });
        id
    }

    pub fn project(&self, id: u64) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }

    /// Projects whose name contains `filter` (case-insensitive), sorted.
    pub fn view(&self, key: SortKey, ascending: bool, filter: &str) -> Vec<&Project> {
        let needle = filter.trim().to_lowercase();
        let mut list: Vec<&Project> = self
            .projects
            .iter()
            .filter(|p| needle.is_empty() || p.name.to_lowercase().contains(&needle))
            .collect();
        list.sort_by(|a, b| {
            match key {
                SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                SortKey::Modified => a.modified.cmp(&b.modified),
                SortKey::Created => a.created.cmp(&b.created),
            }
            .then(a.id.cmp(&b.id))
        });
        if !ascending {
            list.reverse();
        }
        list
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
    fn roundtrip_keeps_unicode_and_escapes() {
        let dir = temp_dir("roundtrip");
        let mut s = Store::load(&dir);
        s.set_name("Вадим 🎨");
        let id = s.create_project(100, "Untitled");
        s.projects[0].name = "tab\there\\and\nnewline Ж".to_string();
        s.save().unwrap();

        let back = Store::load(&dir);
        assert_eq!(back.name.as_deref(), Some("Вадим 🎨"));
        assert_eq!(back.projects, s.projects);
        assert_eq!(back.project(id).unwrap().created, 100);
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
    fn create_picks_free_untitled_number() {
        let mut s = Store::blank(&PathBuf::new());
        s.create_project(1, "Untitled");
        s.create_project(2, "Untitled");
        s.projects.remove(0);
        s.create_project(3, "Untitled");
        let names: Vec<_> = s.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Untitled 2", "Untitled 1"]);
    }

    #[test]
    fn view_sorts_and_filters() {
        let mut s = Store::blank(&PathBuf::new());
        for (n, c, m) in [("beta", 1, 30), ("Alpha", 2, 10), ("gamma", 3, 20)] {
            let id = s.create_project(0, "Untitled");
            let p = s.projects.iter_mut().find(|p| p.id == id).unwrap();
            (p.name, p.created, p.modified) = (n.to_string(), c, m);
        }
        let names = |v: Vec<&Project>| v.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
        assert_eq!(
            names(s.view(SortKey::Name, true, "")),
            ["Alpha", "beta", "gamma"]
        );
        assert_eq!(
            names(s.view(SortKey::Modified, false, "")),
            ["beta", "gamma", "Alpha"]
        );
        assert_eq!(
            names(s.view(SortKey::Created, true, "")),
            ["beta", "Alpha", "gamma"]
        );
        assert_eq!(names(s.view(SortKey::Name, true, " AL ")), ["Alpha"]);
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
    fn new_projects_use_the_given_base_name() {
        let mut s = Store::blank(&PathBuf::new());
        s.create_project(1, "Без названия");
        s.create_project(2, "Без названия");
        let names: Vec<_> = s.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Без названия 1", "Без названия 2"]);
    }
}
