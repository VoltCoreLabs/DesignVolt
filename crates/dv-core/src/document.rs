//! Document model: pages with a tree of nodes. Ids are stable (a monotonic counter), so a
//! command that refers to a node stays valid across undo/redo.

use std::collections::HashMap;

/// Stable node identifier, unique inside a document.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct NodeId(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeKind {
    Frame,
    Rectangle,
    Ellipse,
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

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Stroke {
    pub color: Color,
    pub width: f64,
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
    pub fill: Option<Color>,
    pub stroke: Option<Stroke>,
    pub radius: f64,
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
            fill: Some(Color::rgb(200, 205, 220)),
            stroke: None,
            radius: 0.0,
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
