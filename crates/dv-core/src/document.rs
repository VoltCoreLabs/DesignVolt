//! Document model: pages with a tree of nodes. Ids are stable (a monotonic counter), so a
//! command that refers to a node stays valid across undo/redo.

use std::collections::HashMap;

use crate::paint::Paint;

/// Stable node identifier, unique inside a document.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct NodeId(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeKind {
    Frame,
    Rectangle,
    Ellipse,
    /// A container without a look of its own: it only holds shapes together.
    Group,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

/// How the line of a stroke is drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StrokeStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

impl StrokeStyle {
    pub const ALL: [StrokeStyle; 3] =
        [StrokeStyle::Solid, StrokeStyle::Dashed, StrokeStyle::Dotted];

    pub fn code(self) -> &'static str {
        match self {
            StrokeStyle::Solid => "solid",
            StrokeStyle::Dashed => "dashed",
            StrokeStyle::Dotted => "dotted",
        }
    }

    pub fn from_code(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.code() == s)
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|k| *k == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Stroke {
    pub color: Color,
    pub width: f64,
    pub style: StrokeStyle,
}

/// A soft shadow behind a shape.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shadow {
    pub color: Color,
    pub x: f64,
    pub y: f64,
    pub blur: f64,
}

impl Shadow {
    /// The shadow the "+" button adds.
    pub const DEFAULT: Shadow = Shadow {
        color: Color {
            r: 0,
            g: 0,
            b: 0,
            a: 64,
        },
        x: 0.0,
        y: 4.0,
        blur: 8.0,
    };
}

/// Position and size in the parent's coordinate space; rotation (degrees) is around the center.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Geometry {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub rotation: f64,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Node {
    pub kind: NodeKind,
    pub name: String,
    pub geom: Geometry,
    pub fill: Option<Paint>,
    pub stroke: Option<Stroke>,
    pub radius: f64,
    /// Opacity of the whole shape (0..1), together with its children.
    pub opacity: f64,
    pub shadow: Option<Shadow>,
    pub visible: bool,
    pub locked: bool,
    pub parent: Option<NodeId>,
    /// Children in z-order: the last one is on top.
    pub children: Vec<NodeId>,
}

impl Node {
    pub fn new(kind: NodeKind, name: impl Into<String>, geom: Geometry) -> Self {
        Self {
            kind,
            name: name.into(),
            geom,
            fill: Some(Paint::Solid(Color::rgb(200, 205, 220))),
            stroke: None,
            radius: 0.0,
            opacity: 1.0,
            shadow: None,
            visible: true,
            locked: false,
            parent: None,
            children: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Page {
    pub name: String,
    pub(crate) nodes: HashMap<NodeId, Node>,
    pub(crate) roots: Vec<NodeId>,
}

impl Page {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id)
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Top-level nodes in z-order (last on top).
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// Children of `parent` (or the roots) in z-order.
    pub fn siblings(&self, parent: Option<NodeId>) -> &[NodeId] {
        match parent {
            None => &self.roots,
            Some(p) => self.nodes.get(&p).map_or(&[], |n| n.children.as_slice()),
        }
    }

    /// Ids in drawing order: parents before their children, bottom to top.
    pub fn draw_order(&self) -> Vec<NodeId> {
        let mut out = Vec::with_capacity(self.nodes.len());
        let mut stack: Vec<NodeId> = self.roots.iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            out.push(id);
            if let Some(n) = self.nodes.get(&id) {
                stack.extend(n.children.iter().rev().copied());
            }
        }
        out
    }

    /// `id` and all its descendants, parents first.
    pub fn subtree(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            out.push(cur);
            if let Some(n) = self.nodes.get(&cur) {
                stack.extend(n.children.iter().rev().copied());
            }
        }
        out
    }

    /// Whether `ancestor` is `id` itself or one of its parents.
    pub fn is_inside(&self, id: NodeId, ancestor: NodeId) -> bool {
        let mut cur = Some(id);
        while let Some(c) = cur {
            if c == ancestor {
                return true;
            }
            cur = self.nodes.get(&c).and_then(|n| n.parent);
        }
        false
    }

    /// `ids` without the nodes that lie inside another node of the list, so that moving or
    /// deleting a folder-like node does not also treat its children separately.
    pub fn without_descendants(&self, ids: &[NodeId]) -> Vec<NodeId> {
        ids.iter()
            .copied()
            .filter(|id| {
                let parent = self.nodes.get(id).and_then(|n| n.parent);
                !ids.iter()
                    .any(|other| other != id && parent.is_some_and(|p| self.is_inside(p, *other)))
            })
            .collect()
    }

    /// The shapes that geometry edits (move, resize, flip) act on for a selection: `ids`
    /// without nested duplicates, with every group replaced by the shapes inside it.
    pub fn geo_tops(&self, ids: &[NodeId]) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack: Vec<NodeId> = self.without_descendants(ids).into_iter().rev().collect();
        while let Some(id) = stack.pop() {
            let Some(n) = self.nodes.get(&id) else {
                continue;
            };
            if n.kind == NodeKind::Group {
                stack.extend(n.children.iter().rev());
            } else {
                out.push(id);
            }
        }
        out
    }

    /// The outermost group that contains `id` (or `id` itself when it is in no group): what a
    /// click on the shape selects.
    pub fn outer_group(&self, id: NodeId) -> NodeId {
        let mut best = id;
        let mut cur = self.nodes.get(&id).and_then(|n| n.parent);
        while let Some(c) = cur {
            let Some(n) = self.nodes.get(&c) else { break };
            if n.kind == NodeKind::Group {
                best = c;
            }
            cur = n.parent;
        }
        best
    }
}

#[derive(Clone, Debug)]
pub struct Document {
    pub name: String,
    pages: Vec<Page>,
    current: usize,
    next_id: u64,
    revision: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

impl Document {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            pages: vec![Page::new("Page 1")],
            current: 0,
            next_id: 1,
            revision: 0,
        }
    }

    /// Builds a document from loaded pages (used by the text loader).
    pub(crate) fn from_parts(name: String, pages: Vec<Page>, next_id: u64) -> Self {
        let pages = if pages.is_empty() {
            vec![Page::new("Page 1")]
        } else {
            pages
        };
        Self {
            name,
            pages,
            current: 0,
            next_id,
            revision: 0,
        }
    }

    pub fn page(&self) -> &Page {
        &self.pages[self.current]
    }

    pub(crate) fn page_mut(&mut self) -> &mut Page {
        self.revision += 1;
        &mut self.pages[self.current]
    }

    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Increases on every change, so caches can tell when to rebuild.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Reserves an id for a node that is about to be created.
    pub fn alloc_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Makes sure ids from restored nodes are never handed out again.
    pub(crate) fn reserve_id(&mut self, id: NodeId) {
        self.next_id = self.next_id.max(id.0 + 1);
    }
}
