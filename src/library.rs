//! Projects and folders: pure data model with nesting, trash, favorites, sorting and search.
//! No I/O and no rendering, so every rule is unit-tested.

pub const NAME_MAX: usize = 40;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Item {
    Folder(u64),
    Project(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub id: u64,
    pub name: String,
    pub created: u64,
    pub modified: u64,
    pub folder: Option<u64>,
    pub favorite: bool,
    /// Time of moving to the trash.
    pub deleted: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    pub id: u64,
    pub name: String,
    pub created: u64,
    pub parent: Option<u64>,
    pub deleted: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortKey {
    Name,
    Modified,
    Created,
}

impl SortKey {
    pub fn code(self) -> &'static str {
        match self {
            SortKey::Name => "name",
            SortKey::Modified => "modified",
            SortKey::Created => "created",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        match s.trim() {
            "name" => Some(SortKey::Name),
            "modified" => Some(SortKey::Modified),
            "created" => Some(SortKey::Created),
            _ => None,
        }
    }
}

/// What the user is looking at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Location {
    /// All projects, inside the given folder (`None` = root).
    All(Option<u64>),
    Recent,
    Favorites,
    Trash,
}

pub enum Entry<'a> {
    Folder(&'a Folder),
    Project(&'a Project),
}

impl Entry<'_> {
    pub fn item(&self) -> Item {
        match self {
            Entry::Folder(f) => Item::Folder(f.id),
            Entry::Project(p) => Item::Project(p.id),
        }
    }

    #[cfg(test)]
    pub fn name(&self) -> &str {
        match self {
            Entry::Folder(f) => &f.name,
            Entry::Project(p) => &p.name,
        }
    }
}

#[derive(Default, Clone)]
pub struct Library {
    pub projects: Vec<Project>,
    pub folders: Vec<Folder>,
}

/// Trims, drops control characters and cuts to `NAME_MAX` characters; `None` when blank.
pub fn clean_name(raw: &str) -> Option<String> {
    let s: String = raw.chars().filter(|c| !c.is_control()).collect();
    let s: String = s.trim().chars().take(NAME_MAX).collect();
    let s = s.trim().to_string();
    (!s.is_empty()).then_some(s)
}

impl Library {
    fn next_id(&self) -> u64 {
        let p = self.projects.iter().map(|p| p.id).max().unwrap_or(0);
        let f = self.folders.iter().map(|f| f.id).max().unwrap_or(0);
        p.max(f) + 1
    }

    pub fn project(&self, id: u64) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }

    pub fn folder(&self, id: u64) -> Option<&Folder> {
        self.folders.iter().find(|f| f.id == id)
    }

    fn project_mut(&mut self, id: u64) -> Option<&mut Project> {
        self.projects.iter_mut().find(|p| p.id == id)
    }

    fn folder_mut(&mut self, id: u64) -> Option<&mut Folder> {
        self.folders.iter_mut().find(|f| f.id == id)
    }

    pub fn exists(&self, item: Item) -> bool {
        match item {
            Item::Folder(id) => self.folder(id).is_some(),
            Item::Project(id) => self.project(id).is_some(),
        }
    }

    pub fn name_of(&self, item: Item) -> Option<&str> {
        match item {
            Item::Folder(id) => self.folder(id).map(|f| f.name.as_str()),
            Item::Project(id) => self.project(id).map(|p| p.name.as_str()),
        }
    }

    fn unique_name(&self, base: &str, taken: impl Fn(&str) -> bool) -> String {
        let mut n = 1;
        while taken(&format!("{base} {n}")) {
            n += 1;
        }
        format!("{base} {n}")
    }

    pub fn create_project(&mut self, now: u64, base: &str, folder: Option<u64>) -> u64 {
        let id = self.next_id();
        let name = self.unique_name(base, |s| self.projects.iter().any(|p| p.name == s));
        self.projects.push(Project {
            id,
            name,
            created: now,
            modified: now,
            folder,
            favorite: false,
            deleted: None,
        });
        id
    }

    pub fn create_folder(&mut self, now: u64, base: &str, parent: Option<u64>) -> u64 {
        let id = self.next_id();
        let name = self.unique_name(base, |s| self.folders.iter().any(|f| f.name == s));
        self.folders.push(Folder {
            id,
            name,
            created: now,
            parent,
            deleted: None,
        });
        id
    }

    /// Renames an item; returns false for blank names or unknown items.
    pub fn rename(&mut self, item: Item, raw: &str) -> bool {
        let Some(name) = clean_name(raw) else {
            return false;
        };
        match item {
            Item::Folder(id) => self.folder_mut(id).map(|f| f.name = name).is_some(),
            Item::Project(id) => self.project_mut(id).map(|p| p.name = name).is_some(),
        }
    }

    pub fn touch(&mut self, id: u64, now: u64) {
        if let Some(p) = self.project_mut(id) {
            p.modified = now;
        }
    }

    pub fn set_favorite(&mut self, id: u64, favorite: bool) {
        if let Some(p) = self.project_mut(id) {
            p.favorite = favorite;
        }
    }

    #[cfg(test)]
    pub fn toggle_favorite(&mut self, id: u64) {
        if let Some(p) = self.project_mut(id) {
            p.favorite = !p.favorite;
        }
    }

    fn parent_of(&self, item: Item) -> Option<u64> {
        match item {
            Item::Folder(id) => self.folder(id).and_then(|f| f.parent),
            Item::Project(id) => self.project(id).and_then(|p| p.folder),
        }
    }

    /// `ancestor` is `folder` itself or one of its parents.
    fn is_inside(&self, folder: u64, ancestor: u64) -> bool {
        let mut cur = Some(folder);
        for _ in 0..=self.folders.len() {
            match cur {
                Some(id) if id == ancestor => return true,
                Some(id) => cur = self.folder(id).and_then(|f| f.parent),
                None => return false,
            }
        }
        false
    }

    /// Own `deleted` flag or a deleted ancestor folder.
    pub fn is_hidden(&self, item: Item) -> bool {
        let own = match item {
            Item::Folder(id) => self.folder(id).is_some_and(|f| f.deleted.is_some()),
            Item::Project(id) => self.project(id).is_some_and(|p| p.deleted.is_some()),
        };
        if own {
            return true;
        }
        let mut cur = self.parent_of(item);
        for _ in 0..=self.folders.len() {
            match cur {
                Some(id) => {
                    let Some(f) = self.folder(id) else {
                        return false;
                    };
                    if f.deleted.is_some() {
                        return true;
                    }
                    cur = f.parent;
                }
                None => return false,
            }
        }
        false
    }

    /// Whether `item` may be dropped into `target` (a folder or the root).
    pub fn can_move(&self, item: Item, target: Option<u64>) -> bool {
        if let Some(t) = target {
            if self.folder(t).is_none() || self.is_hidden(Item::Folder(t)) {
                return false;
            }
            if let Item::Folder(id) = item
                && self.is_inside(t, id)
            {
                return false;
            }
        }
        self.exists(item) && self.parent_of(item) != target
    }

    pub fn move_to(&mut self, item: Item, target: Option<u64>) -> bool {
        if !self.can_move(item, target) {
            return false;
        }
        match item {
            Item::Folder(id) => self.folder_mut(id).map(|f| f.parent = target).is_some(),
            Item::Project(id) => self.project_mut(id).map(|p| p.folder = target).is_some(),
        }
    }

    pub fn trash(&mut self, item: Item, now: u64) {
        match item {
            Item::Folder(id) => {
                if let Some(f) = self.folder_mut(id) {
                    f.deleted = Some(now);
                }
            }
            Item::Project(id) => {
                if let Some(p) = self.project_mut(id) {
                    p.deleted = Some(now);
                }
            }
        }
    }

    /// Restores from the trash; an item whose folder is still deleted goes to the root.
    pub fn restore(&mut self, item: Item) {
        match item {
            Item::Folder(id) => {
                if let Some(f) = self.folder_mut(id) {
                    f.deleted = None;
                }
            }
            Item::Project(id) => {
                if let Some(p) = self.project_mut(id) {
                    p.deleted = None;
                }
            }
        }
        if self.is_hidden(item) {
            match item {
                Item::Folder(id) => self.folder_mut(id).map(|f| f.parent = None),
                Item::Project(id) => self.project_mut(id).map(|p| p.folder = None),
            };
        }
    }

    /// Deletes forever; a folder takes everything inside it along.
    pub fn purge(&mut self, item: Item) {
        match item {
            Item::Project(id) => self.projects.retain(|p| p.id != id),
            Item::Folder(id) => {
                let doomed: Vec<u64> = self
                    .folders
                    .iter()
                    .map(|f| f.id)
                    .filter(|f| self.is_inside(*f, id))
                    .collect();
                self.projects
                    .retain(|p| !p.folder.is_some_and(|f| doomed.contains(&f)));
                self.folders.retain(|f| !doomed.contains(&f.id));
            }
        }
    }

    /// Copies a project next to the original; the copy is named "<name> (<suffix>)".
    pub fn duplicate_project(&mut self, id: u64, now: u64, suffix: &str) -> Option<u64> {
        let src = self.project(id)?.clone();
        let new_id = self.next_id();
        let name = self.copy_name(&src.name, suffix, |s| {
            self.projects.iter().any(|p| p.name == s)
        });
        self.projects.push(Project {
            id: new_id,
            name,
            created: now,
            modified: now,
            folder: src.folder,
            favorite: false,
            deleted: None,
        });
        Some(new_id)
    }

    /// Copies a folder with everything inside it (trashed content is not copied).
    pub fn duplicate_folder(&mut self, id: u64, now: u64, suffix: &str) -> Option<u64> {
        let src = self.folder(id)?.clone();
        let name = self.copy_name(&src.name, suffix, |s| {
            self.folders.iter().any(|f| f.name == s)
        });
        let new_id = self.create_folder_named(name, now, src.parent);
        self.copy_children(id, new_id, now);
        Some(new_id)
    }

    fn create_folder_named(&mut self, name: String, now: u64, parent: Option<u64>) -> u64 {
        let id = self.next_id();
        self.folders.push(Folder {
            id,
            name,
            created: now,
            parent,
            deleted: None,
        });
        id
    }

    fn copy_children(&mut self, from: u64, to: u64, now: u64) {
        let projects: Vec<Project> = self
            .projects
            .iter()
            .filter(|p| p.folder == Some(from) && p.deleted.is_none())
            .cloned()
            .collect();
        for p in projects {
            let id = self.next_id();
            self.projects.push(Project {
                id,
                folder: Some(to),
                created: now,
                modified: now,
                ..p
            });
        }
        let folders: Vec<Folder> = self
            .folders
            .iter()
            .filter(|f| f.parent == Some(from) && f.deleted.is_none())
            .cloned()
            .collect();
        for f in folders {
            let old = f.id;
            let new = self.create_folder_named(f.name, now, Some(to));
            self.copy_children(old, new, now);
        }
    }

    fn copy_name(&self, base: &str, suffix: &str, taken: impl Fn(&str) -> bool) -> String {
        let room = NAME_MAX.saturating_sub(suffix.chars().count() + 6);
        let base: String = base.chars().take(room.max(1)).collect();
        let first = format!("{base} ({suffix})");
        if !taken(&first) {
            return first;
        }
        let mut n = 2;
        while taken(&format!("{base} ({suffix} {n})")) {
            n += 1;
        }
        format!("{base} ({suffix} {n})")
    }

    /// Deletes everything that sits in the trash.
    pub fn empty_trash(&mut self) {
        let items: Vec<Item> = self
            .view(Location::Trash, SortKey::Name, true, "")
            .iter()
            .map(|e| e.item())
            .collect();
        for item in items {
            self.purge(item);
        }
    }

    /// Drops items that lie inside another folder of the same list (group operations
    /// on a folder already include its content).
    pub fn prune_nested(&self, items: &[Item]) -> Vec<Item> {
        let folders: Vec<u64> = items
            .iter()
            .filter_map(|i| match i {
                Item::Folder(id) => Some(*id),
                Item::Project(_) => None,
            })
            .collect();
        items
            .iter()
            .copied()
            .filter(|i| {
                let parent = self.parent_of(*i);
                !folders.iter().any(|f| {
                    parent.is_some_and(|p| self.is_inside(p, *f)) && Item::Folder(*f) != *i
                })
            })
            .collect()
    }

    /// Visible direct children of a folder.
    pub fn child_count(&self, folder: u64) -> usize {
        let folders = self
            .folders
            .iter()
            .filter(|f| f.parent == Some(folder) && f.deleted.is_none());
        let projects = self
            .projects
            .iter()
            .filter(|p| p.folder == Some(folder) && p.deleted.is_none());
        folders.count() + projects.count()
    }

    /// Folder chain from the root down to `folder`.
    pub fn breadcrumb(&self, folder: Option<u64>) -> Vec<&Folder> {
        let mut path = Vec::new();
        let mut cur = folder;
        for _ in 0..=self.folders.len() {
            let Some(f) = cur.and_then(|id| self.folder(id)) else {
                break;
            };
            path.push(f);
            cur = f.parent;
        }
        path.reverse();
        path
    }

    pub fn trash_count(&self) -> usize {
        self.view(Location::Trash, SortKey::Name, true, "").len()
    }

    /// Entries to show: folders first (in `All`), sorted by `key`; a non-blank `filter`
    /// searches names across the whole library instead of the current folder.
    pub fn view(&self, loc: Location, key: SortKey, asc: bool, filter: &str) -> Vec<Entry<'_>> {
        let needle = filter.trim().to_lowercase();
        let matches = |n: &str| needle.is_empty() || n.to_lowercase().contains(&needle);
        let live = |i: Item| !self.is_hidden(i);
        let mut out: Vec<Entry> = Vec::new();
        match loc {
            Location::All(folder) => {
                let searching = !needle.is_empty();
                let here = |parent: Option<u64>| searching || parent == folder;
                out.extend(
                    self.folders
                        .iter()
                        .filter(|f| here(f.parent) && live(Item::Folder(f.id)) && matches(&f.name))
                        .map(Entry::Folder),
                );
                out.extend(
                    self.projects
                        .iter()
                        .filter(|p| here(p.folder) && live(Item::Project(p.id)) && matches(&p.name))
                        .map(Entry::Project),
                );
            }
            Location::Recent => out.extend(
                self.projects
                    .iter()
                    .filter(|p| live(Item::Project(p.id)) && matches(&p.name))
                    .map(Entry::Project),
            ),
            Location::Favorites => out.extend(
                self.projects
                    .iter()
                    .filter(|p| p.favorite && live(Item::Project(p.id)) && matches(&p.name))
                    .map(Entry::Project),
            ),
            Location::Trash => {
                let top = |parent: Option<u64>| {
                    parent.is_none_or(|id| self.folder(id).is_none_or(|f| f.deleted.is_none()))
                };
                out.extend(
                    self.folders
                        .iter()
                        .filter(|f| f.deleted.is_some() && top(f.parent) && matches(&f.name))
                        .map(Entry::Folder),
                );
                out.extend(
                    self.projects
                        .iter()
                        .filter(|p| p.deleted.is_some() && top(p.folder) && matches(&p.name))
                        .map(Entry::Project),
                );
            }
        }
        let (key, asc) = match loc {
            Location::Recent => (SortKey::Modified, false),
            Location::Trash => (SortKey::Modified, false),
            _ => (key, asc),
        };
        out.sort_by(|a, b| {
            let rank = |e: &Entry| u8::from(matches!(e, Entry::Project(_)));
            let order = match (a, b) {
                (Entry::Project(x), Entry::Project(y)) => by_key(key, x, y),
                (Entry::Folder(x), Entry::Folder(y)) => match key {
                    SortKey::Name => x.name.to_lowercase().cmp(&y.name.to_lowercase()),
                    _ => x.created.cmp(&y.created),
                },
                _ => std::cmp::Ordering::Equal,
            };
            let order = if asc { order } else { order.reverse() };
            rank(a)
                .cmp(&rank(b))
                .then(order)
                .then_with(|| item_id(a).cmp(&item_id(b)))
        });
        out
    }
}

fn item_id(e: &Entry) -> u64 {
    match e {
        Entry::Folder(f) => f.id,
        Entry::Project(p) => p.id,
    }
}

fn by_key(key: SortKey, a: &Project, b: &Project) -> std::cmp::Ordering {
    match key {
        SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        SortKey::Modified => a.modified.cmp(&b.modified),
        SortKey::Created => a.created.cmp(&b.created),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: Vec<Entry>) -> Vec<String> {
        v.iter().map(|e| e.name().to_string()).collect()
    }

    fn lib() -> Library {
        Library::default()
    }

    #[test]
    fn clean_name_rules() {
        assert_eq!(clean_name("  hi  "), Some("hi".into()));
        assert_eq!(clean_name("a\tb\n"), Some("ab".into()));
        assert_eq!(clean_name("   "), None);
        assert_eq!(
            clean_name(&"ж".repeat(60)).unwrap().chars().count(),
            NAME_MAX
        );
    }

    #[test]
    fn unique_names_pick_lowest_free_number() {
        let mut l = lib();
        l.create_project(1, "P", None);
        l.create_project(2, "P", None);
        l.projects.remove(0);
        l.create_project(3, "P", None);
        let n: Vec<_> = l.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(n, ["P 2", "P 1"]);
        l.create_folder(1, "F", None);
        assert_eq!(l.folders[0].name, "F 1");
    }

    #[test]
    fn rename_rejects_blank() {
        let mut l = lib();
        let id = l.create_project(1, "P", None);
        assert!(l.rename(Item::Project(id), "  New  "));
        assert_eq!(l.project(id).unwrap().name, "New");
        assert!(!l.rename(Item::Project(id), "   "));
        assert_eq!(l.project(id).unwrap().name, "New");
        assert!(!l.rename(Item::Project(999), "x"));
    }

    #[test]
    fn folders_come_first_and_view_is_per_folder() {
        let mut l = lib();
        let f = l.create_folder(1, "Work", None);
        let a = l.create_project(2, "a", None);
        let b = l.create_project(3, "b", Some(f));
        l.rename(Item::Project(a), "alpha");
        l.rename(Item::Project(b), "beta");
        assert_eq!(
            names(l.view(Location::All(None), SortKey::Name, true, "")),
            ["Work 1", "alpha"]
        );
        assert_eq!(
            names(l.view(Location::All(Some(f)), SortKey::Name, true, "")),
            ["beta"]
        );
    }

    #[test]
    fn sorting_keeps_folders_first() {
        let mut l = lib();
        let f1 = l.create_folder(5, "zzz", None);
        let f2 = l.create_folder(6, "aaa", None);
        l.rename(Item::Folder(f1), "zzz");
        l.rename(Item::Folder(f2), "aaa");
        let p = l.create_project(1, "p", None);
        l.touch(p, 50);
        let v = names(l.view(Location::All(None), SortKey::Name, false, ""));
        assert_eq!(v, ["zzz", "aaa", "p 1"]);
    }

    #[test]
    fn search_is_global_and_case_insensitive() {
        let mut l = lib();
        let f = l.create_folder(1, "Deep", None);
        let p = l.create_project(2, "x", Some(f));
        l.rename(Item::Project(p), "Logo Design");
        assert_eq!(
            names(l.view(Location::All(None), SortKey::Name, true, " LOGO ")),
            ["Logo Design"]
        );
        assert!(
            l.view(Location::All(None), SortKey::Name, true, "zzz")
                .is_empty()
        );
    }

    #[test]
    fn move_rules_and_cycles() {
        let mut l = lib();
        let a = l.create_folder(1, "A", None);
        let b = l.create_folder(2, "B", Some(a));
        let p = l.create_project(3, "p", None);
        assert!(l.move_to(Item::Project(p), Some(b)));
        assert!(!l.move_to(Item::Project(p), Some(b)), "already there");
        assert!(
            !l.can_move(Item::Folder(a), Some(b)),
            "folder into its child"
        );
        assert!(!l.can_move(Item::Folder(a), Some(a)), "folder into itself");
        assert!(l.move_to(Item::Folder(b), None));
        assert!(l.move_to(Item::Project(p), None));
        assert!(!l.can_move(Item::Project(p), Some(999)));
    }

    #[test]
    fn trash_hides_items_and_children_of_folders() {
        let mut l = lib();
        let f = l.create_folder(1, "F", None);
        let p = l.create_project(2, "p", Some(f));
        l.trash(Item::Folder(f), 10);
        assert!(l.is_hidden(Item::Project(p)));
        assert!(
            l.view(Location::All(None), SortKey::Name, true, "")
                .is_empty()
        );
        // Only the folder is listed in the trash, not its content.
        assert_eq!(
            names(l.view(Location::Trash, SortKey::Name, true, "")),
            ["F 1"]
        );
        assert_eq!(l.trash_count(), 1);
        l.restore(Item::Folder(f));
        assert!(!l.is_hidden(Item::Project(p)));
        assert_eq!(l.trash_count(), 0);
    }

    #[test]
    fn restore_moves_to_root_when_folder_is_still_deleted() {
        let mut l = lib();
        let f = l.create_folder(1, "F", None);
        let p = l.create_project(2, "p", Some(f));
        l.trash(Item::Project(p), 5);
        l.trash(Item::Folder(f), 6);
        l.restore(Item::Project(p));
        assert_eq!(l.project(p).unwrap().folder, None);
        assert!(!l.is_hidden(Item::Project(p)));
    }

    #[test]
    fn purge_folder_removes_everything_inside() {
        let mut l = lib();
        let a = l.create_folder(1, "A", None);
        let b = l.create_folder(2, "B", Some(a));
        l.create_project(3, "p", Some(b));
        let keep = l.create_project(4, "keep", None);
        l.purge(Item::Folder(a));
        assert!(l.folders.is_empty());
        assert_eq!(l.projects.len(), 1);
        assert_eq!(l.projects[0].id, keep);
    }

    #[test]
    fn favorites_and_recent() {
        let mut l = lib();
        let a = l.create_project(1, "a", None);
        let b = l.create_project(2, "b", None);
        l.touch(a, 100);
        l.toggle_favorite(b);
        assert_eq!(
            names(l.view(Location::Favorites, SortKey::Name, true, "")),
            ["b 1"]
        );
        assert_eq!(
            names(l.view(Location::Recent, SortKey::Name, true, "")),
            ["a 1", "b 1"]
        );
        l.trash(Item::Project(b), 9);
        assert!(
            l.view(Location::Favorites, SortKey::Name, true, "")
                .is_empty()
        );
    }

    #[test]
    fn breadcrumb_and_child_count() {
        let mut l = lib();
        let a = l.create_folder(1, "A", None);
        let b = l.create_folder(2, "B", Some(a));
        l.create_project(3, "p", Some(b));
        let path: Vec<_> = l
            .breadcrumb(Some(b))
            .iter()
            .map(|f| f.name.clone())
            .collect();
        assert_eq!(path, ["A 1", "B 1"]);
        assert!(l.breadcrumb(None).is_empty());
        assert_eq!(l.child_count(a), 1);
        assert_eq!(l.child_count(b), 1);
    }

    #[test]
    fn duplicate_project_makes_an_independent_copy() {
        let mut l = lib();
        let f = l.create_folder(1, "F", None);
        let p = l.create_project(2, "Logo", Some(f));
        l.toggle_favorite(p);
        l.touch(p, 50);
        let c = l.duplicate_project(p, 100, "copy").unwrap();
        let copy = l.project(c).unwrap();
        assert_eq!(copy.name, "Logo 1 (copy)");
        assert_eq!(copy.folder, Some(f));
        assert!(!copy.favorite);
        assert_eq!((copy.created, copy.modified), (100, 100));
        let c2 = l.duplicate_project(p, 101, "copy").unwrap();
        assert_eq!(l.project(c2).unwrap().name, "Logo 1 (copy 2)");
        assert!(l.duplicate_project(999, 1, "copy").is_none());
    }

    #[test]
    fn duplicate_folder_copies_nested_content_but_not_trash() {
        let mut l = lib();
        let a = l.create_folder(1, "A", None);
        let b = l.create_folder(2, "B", Some(a));
        l.create_project(3, "p", Some(a));
        let q = l.create_project(4, "q", Some(b));
        let gone = l.create_project(5, "gone", Some(a));
        l.trash(Item::Project(gone), 9);
        let copy = l.duplicate_folder(a, 100, "copy").unwrap();
        assert_ne!(copy, a);
        assert_eq!(l.folder(copy).unwrap().name, "A 1 (copy)");
        // Copy has one project and one sub-folder with one project; the trashed one stays out.
        assert_eq!(l.child_count(copy), 2);
        let sub = l
            .folders
            .iter()
            .find(|f| f.parent == Some(copy))
            .unwrap()
            .id;
        assert_ne!(sub, b);
        assert_eq!(l.child_count(sub), 1);
        assert_eq!(l.project(q).unwrap().folder, Some(b), "original untouched");
    }

    #[test]
    fn copy_name_is_cut_to_fit_the_limit() {
        let mut l = lib();
        let p = l.create_project(1, &"x".repeat(60), None);
        l.rename(Item::Project(p), &"y".repeat(NAME_MAX));
        let c = l.duplicate_project(p, 2, "copy").unwrap();
        assert!(l.project(c).unwrap().name.chars().count() <= NAME_MAX);
    }

    #[test]
    fn empty_trash_removes_only_deleted_items() {
        let mut l = lib();
        let f = l.create_folder(1, "F", None);
        l.create_project(2, "inside", Some(f));
        let keep = l.create_project(3, "keep", None);
        let gone = l.create_project(4, "gone", None);
        l.trash(Item::Folder(f), 5);
        l.trash(Item::Project(gone), 6);
        l.empty_trash();
        assert!(l.folders.is_empty());
        assert_eq!(l.projects.len(), 1);
        assert_eq!(l.projects[0].id, keep);
        assert_eq!(l.trash_count(), 0);
    }

    #[test]
    fn prune_nested_drops_items_inside_selected_folders() {
        let mut l = lib();
        let a = l.create_folder(1, "A", None);
        let b = l.create_folder(2, "B", Some(a));
        let p = l.create_project(3, "p", Some(b));
        let q = l.create_project(4, "q", None);
        let all = [
            Item::Folder(a),
            Item::Folder(b),
            Item::Project(p),
            Item::Project(q),
        ];
        assert_eq!(
            l.prune_nested(&all),
            vec![Item::Folder(a), Item::Project(q)]
        );
        assert_eq!(
            l.prune_nested(&[Item::Folder(b), Item::Project(p)]),
            vec![Item::Folder(b)]
        );
    }

    #[test]
    fn sort_key_codes_roundtrip() {
        for k in [SortKey::Name, SortKey::Modified, SortKey::Created] {
            assert_eq!(SortKey::from_code(k.code()), Some(k));
        }
        assert_eq!(SortKey::from_code("size"), None);
    }
}
