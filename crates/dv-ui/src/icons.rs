//! Phosphor icons (D-005): SVG path data parsed once with `kurbo`, drawn as Vello fills.

use vello::kurbo::BezPath;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Cursor,
    Frame,
    Square,
    Circle,
    Pen,
    Text,
    Hand,
    CaretDown,
    Eye,
    Lock,
    Search,
    Plus,
    Share,
    Home,
    Minimize,
    Restore,
    Close,
    Sun,
    Moon,
    Gear,
    Duplicate,
    Check,
    Star,
    StarFill,
    Folder,
    FolderPlus,
    Pencil,
    Trash,
    Undo,
    CaretRight,
    OpenOut,
}

const SOURCES: &[(Icon, &str)] = &[
    (
        Icon::Cursor,
        include_str!("../../../assets/icons/phosphor/cursor.svg"),
    ),
    (
        Icon::Frame,
        include_str!("../../../assets/icons/phosphor/frame-corners.svg"),
    ),
    (
        Icon::Square,
        include_str!("../../../assets/icons/phosphor/square.svg"),
    ),
    (
        Icon::Circle,
        include_str!("../../../assets/icons/phosphor/circle.svg"),
    ),
    (
        Icon::Pen,
        include_str!("../../../assets/icons/phosphor/pen-nib.svg"),
    ),
    (
        Icon::Text,
        include_str!("../../../assets/icons/phosphor/text-t.svg"),
    ),
    (
        Icon::Hand,
        include_str!("../../../assets/icons/phosphor/hand.svg"),
    ),
    (
        Icon::CaretDown,
        include_str!("../../../assets/icons/phosphor/caret-down.svg"),
    ),
    (
        Icon::Eye,
        include_str!("../../../assets/icons/phosphor/eye.svg"),
    ),
    (
        Icon::Lock,
        include_str!("../../../assets/icons/phosphor/lock.svg"),
    ),
    (
        Icon::Search,
        include_str!("../../../assets/icons/phosphor/magnifying-glass.svg"),
    ),
    (
        Icon::Plus,
        include_str!("../../../assets/icons/phosphor/plus.svg"),
    ),
    (
        Icon::Share,
        include_str!("../../../assets/icons/phosphor/share-network.svg"),
    ),
    (
        Icon::Home,
        include_str!("../../../assets/icons/phosphor/house.svg"),
    ),
    (
        Icon::Minimize,
        include_str!("../../../assets/icons/phosphor/minus.svg"),
    ),
    (
        Icon::Restore,
        include_str!("../../../assets/icons/phosphor/copy-simple.svg"),
    ),
    (
        Icon::Close,
        include_str!("../../../assets/icons/phosphor/x.svg"),
    ),
    (
        Icon::Sun,
        include_str!("../../../assets/icons/phosphor/sun.svg"),
    ),
    (
        Icon::Moon,
        include_str!("../../../assets/icons/phosphor/moon.svg"),
    ),
    (
        Icon::Gear,
        include_str!("../../../assets/icons/phosphor/gear.svg"),
    ),
    (
        Icon::Star,
        include_str!("../../../assets/icons/phosphor/star.svg"),
    ),
    (
        Icon::StarFill,
        include_str!("../../../assets/icons/phosphor/star-fill.svg"),
    ),
    (
        Icon::Folder,
        include_str!("../../../assets/icons/phosphor/folder-simple.svg"),
    ),
    (
        Icon::FolderPlus,
        include_str!("../../../assets/icons/phosphor/folder-simple-plus.svg"),
    ),
    (
        Icon::Pencil,
        include_str!("../../../assets/icons/phosphor/pencil-simple.svg"),
    ),
    (
        Icon::Trash,
        include_str!("../../../assets/icons/phosphor/trash.svg"),
    ),
    (
        Icon::Undo,
        include_str!("../../../assets/icons/phosphor/arrow-counter-clockwise.svg"),
    ),
    (
        Icon::CaretRight,
        include_str!("../../../assets/icons/phosphor/caret-right.svg"),
    ),
    (
        Icon::OpenOut,
        include_str!("../../../assets/icons/phosphor/arrow-square-out.svg"),
    ),
    (
        Icon::Duplicate,
        include_str!("../../../assets/icons/phosphor/copy.svg"),
    ),
    (
        Icon::Check,
        include_str!("../../../assets/icons/phosphor/check.svg"),
    ),
];

/// Icon viewBox is 256x256.
pub const VIEWBOX: f64 = 256.0;

pub struct Icons {
    paths: Vec<(Icon, BezPath)>,
}

impl Icons {
    pub fn new() -> Self {
        let paths = SOURCES
            .iter()
            .filter_map(|(icon, svg)| Some((*icon, BezPath::from_svg(extract_d(svg)?).ok()?)))
            .collect();
        Self { paths }
    }

    pub fn get(&self, icon: Icon) -> Option<&BezPath> {
        self.paths.iter().find(|(i, _)| *i == icon).map(|(_, p)| p)
    }
}

/// Returns the value of the first `d` attribute of the SVG.
fn extract_d(svg: &str) -> Option<&str> {
    let start = svg.find(" d=\"")? + 4;
    let end = svg[start..].find('"')? + start;
    Some(&svg[start..end])
}

impl Default for Icons {
    fn default() -> Self {
        Self::new()
    }
}
