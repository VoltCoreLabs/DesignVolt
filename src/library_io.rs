//! Text serialization of the library (no serde: not in the stack).
//!
//! `projects.txt`, tab-separated: `id, created, modified, folder, deleted, favorite, name`
//! (`0` = none for `folder` and `deleted`). Old 4-field lines `id, created, modified, name`
//! are still read. `folders.txt`: `id, created, parent, deleted, name`.
//! In names `\`, tab and newline are escaped.

use crate::library::{Folder, Project};

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

fn opt(v: u64) -> Option<u64> {
    (v != 0).then_some(v)
}

fn parse_project(line: &str) -> Option<Project> {
    let parts: Vec<&str> = line.splitn(7, '\t').collect();
    let num = |i: usize| parts.get(i)?.parse::<u64>().ok();
    match parts.len() {
        4 => Some(Project {
            id: num(0)?,
            created: num(1)?,
            modified: num(2)?,
            name: unescape(parts[3]),
            folder: None,
            favorite: false,
            deleted: None,
        }),
        7 => Some(Project {
            id: num(0)?,
            created: num(1)?,
            modified: num(2)?,
            folder: opt(num(3)?),
            deleted: opt(num(4)?),
            favorite: num(5)? != 0,
            name: unescape(parts[6]),
        }),
        _ => None,
    }
}

fn parse_folder(line: &str) -> Option<Folder> {
    let parts: Vec<&str> = line.splitn(5, '\t').collect();
    if parts.len() != 5 {
        return None;
    }
    let num = |i: usize| parts[i].parse::<u64>().ok();
    Some(Folder {
        id: num(0)?,
        created: num(1)?,
        parent: opt(num(2)?),
        deleted: opt(num(3)?),
        name: unescape(parts[4]),
    })
}

pub fn parse_projects(text: &str) -> Vec<Project> {
    text.lines().filter_map(parse_project).collect()
}

pub fn parse_folders(text: &str) -> Vec<Folder> {
    text.lines().filter_map(parse_folder).collect()
}

pub fn projects_text(projects: &[Project]) -> String {
    projects
        .iter()
        .map(|p| {
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                p.id,
                p.created,
                p.modified,
                p.folder.unwrap_or(0),
                p.deleted.unwrap_or(0),
                u8::from(p.favorite),
                escape(&p.name)
            )
        })
        .collect()
}

pub fn folders_text(folders: &[Folder]) -> String {
    folders
        .iter()
        .map(|f| {
            format!(
                "{}\t{}\t{}\t{}\t{}\n",
                f.id,
                f.created,
                f.parent.unwrap_or(0),
                f.deleted.unwrap_or(0),
                escape(&f.name)
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_roundtrip_with_unicode_and_escapes() {
        let p = vec![Project {
            id: 3,
            name: "tab\there\\and\nnewline Ж 🎨".into(),
            created: 10,
            modified: 20,
            folder: Some(2),
            favorite: true,
            deleted: Some(99),
        }];
        assert_eq!(parse_projects(&projects_text(&p)), p);
    }

    #[test]
    fn folders_roundtrip() {
        let f = vec![
            Folder {
                id: 1,
                name: "Работа".into(),
                created: 5,
                parent: None,
                deleted: None,
            },
            Folder {
                id: 2,
                name: "Sub".into(),
                created: 6,
                parent: Some(1),
                deleted: Some(7),
            },
        ];
        assert_eq!(parse_folders(&folders_text(&f)), f);
    }

    #[test]
    fn old_four_field_projects_are_still_read() {
        let p = parse_projects("4\t100\t200\tOld project\n");
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].id, p[0].created, p[0].modified), (4, 100, 200));
        assert_eq!(p[0].name, "Old project");
        assert_eq!(p[0].folder, None);
        assert!(!p[0].favorite);
    }

    #[test]
    fn broken_lines_are_skipped() {
        assert!(parse_projects("garbage\n1\t2\n").is_empty());
        assert!(parse_folders("x\ty\n").is_empty());
    }
}
