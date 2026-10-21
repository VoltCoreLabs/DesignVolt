//! The pen tool: click adds a corner point, click and drag pulls out smooth handles, a click
//! on the first point closes the path, Enter or Esc finishes it, Backspace removes the last
//! point.

use dv_core::{Anchor, Command, Geometry, Node, NodeId, NodeKind, Stroke, Vector};
use dv_geom::{anchors_bounds, to_parent_space, vector_path};
use dv_ui::draw::Cx;
use dv_ui::theme::colors::*;
use kurbo::{BezPath, Point, Rect};

use super::interact::Interaction;
use super::ops;
use super::state::{Editor, Tool};
use crate::i18n::{Lang, lang};

/// How close (screen px) a click must be to the first point to close the path.
const CLOSE_PX: f64 = 9.0;
/// Handles shorter than this (page units) count as no handle: the point stays a corner.
const MIN_HANDLE: f64 = 2.0;
const POINT: f64 = 4.0;

/// A path being drawn, in page coordinates.
pub struct Pen {
    pub anchors: Vec<Anchor>,
    /// The frame the path will be created in (the one under the first point).
    pub parent: Option<NodeId>,
}

fn name() -> &'static str {
    if lang() == Lang::Ru {
        "Контур"
    } else {
        "Path"
    }
}

/// The node that a finished pen path becomes: the box fits the curve, the points are stored
/// relative to it. `None` for fewer than two points.
pub fn build_node(anchors: &[Anchor], closed: bool) -> Option<(Node, Rect)> {
    if anchors.len() < 2 {
        return None;
    }
    let world = Vector {
        anchors: anchors.to_vec(),
        closed,
        size: (1.0, 1.0),
    };
    let b = anchors_bounds(&world);
    let (w, h) = (b.width().max(1.0), b.height().max(1.0));
    let local: Vec<Anchor> = anchors
        .iter()
        .map(|a| Anchor {
            x: a.x - b.x0,
            y: a.y - b.y0,
            ..*a
        })
        .collect();
    let geom = Geometry {
        x: b.x0,
        y: b.y0,
        w,
        h,
        rotation: 0.0,
    };
    let mut node = Node::new(NodeKind::Path, name(), geom);
    node.fill = None;
    node.stroke = Some(Stroke::new(dv_core::Color::rgb(0, 0, 0), 2.0));
    node.vector = Some(Vector {
        anchors: local,
        closed,
        size: (w, h),
    });
    Some((node, b))
}

impl Editor {
    /// A press with the pen tool: closes the path, or adds a point and starts pulling its handle.
    pub(super) fn pen_press(&mut self, world: Point) {
        if let Some(pen) = &self.pen
            && pen.anchors.len() >= 2
        {
            let zoom = self.session.borrow().vp.zoom;
            let first = &pen.anchors[0];
            if Point::new(first.x, first.y).distance(world) * zoom <= CLOSE_PX {
                self.pen_finish(true);
                return;
            }
        }
        if self.pen.is_none() {
            let parent = {
                let s = self.session.borrow();
                self.index
                    .pick_where(s.exec.doc().page(), world, |n| n.kind == NodeKind::Frame)
            };
            self.pen = Some(Pen {
                anchors: Vec::new(),
                parent,
            });
        }
        if let Some(pen) = self.pen.as_mut() {
            pen.anchors.push(Anchor::corner(world.x, world.y));
        }
        self.interaction = Some(Interaction::PenDrag);
    }

    /// Pulls the handle of the point just added to the pointer.
    pub(super) fn pen_drag(&mut self) {
        let world = self.world_mouse();
        let Some(a) = self.pen.as_mut().and_then(|p| p.anchors.last_mut()) else {
            return;
        };
        let (dx, dy) = (world.x - a.x, world.y - a.y);
        *a = if dx.hypot(dy) < MIN_HANDLE {
            Anchor::corner(a.x, a.y)
        } else {
            Anchor::smooth(a.x, a.y, dx, dy)
        };
    }

    /// Makes the path a node (when it has at least two points) and leaves the pen tool.
    pub(super) fn pen_finish(&mut self, closed: bool) {
        let Some(pen) = self.pen.take() else { return };
        self.interaction = None;
        let Some((mut node, bounds)) = build_node(&pen.anchors, closed) else {
            return;
        };
        let id = {
            let mut s = self.session.borrow_mut();
            let page = s.exec.doc().page();
            let origin = to_parent_space(page, pen.parent, Point::new(bounds.x0, bounds.y0));
            node.geom.x = origin.x;
            node.geom.y = origin.y;
            node.name = ops::next_name(page, name());
            let index = page.siblings(pen.parent).len();
            let id = s.exec.alloc_id();
            let cmd = Command::Create {
                id,
                parent: pen.parent,
                index,
                node,
            };
            s.exec.execute("Pen", cmd).ok().map(|()| id)
        };
        if let Some(id) = id {
            self.selection = vec![id];
        }
        self.set_tool(Tool::Select);
    }

    pub(super) fn pen_backspace(&mut self) {
        if let Some(pen) = self.pen.as_mut() {
            pen.anchors.pop();
            if pen.anchors.is_empty() {
                self.pen = None;
            }
        }
    }
}

/// The path so far, the point under the pointer as a rubber band, and the points.
pub(super) fn draw(ed: &Editor, cx: &mut Cx) {
    let Some(pen) = &ed.pen else { return };
    let s = ed.session.borrow();
    let to_screen = |a: &Anchor| {
        let z = s.vp.zoom;
        let p = s.vp.to_screen(Point::new(a.x, a.y));
        Anchor {
            x: p.x,
            y: p.y,
            in_x: a.in_x * z,
            in_y: a.in_y * z,
            out_x: a.out_x * z,
            out_y: a.out_y * z,
        }
    };
    let mut pts: Vec<Anchor> = pen.anchors.iter().map(to_screen).collect();
    let cursor = ed.canvas_point();
    let dragging = matches!(ed.interaction, Some(Interaction::PenDrag));
    if !dragging {
        pts.push(Anchor::corner(cursor.x, cursor.y));
    }
    if pts.len() >= 2 {
        let v = Vector {
            anchors: pts.clone(),
            closed: false,
            size: (1.0, 1.0),
        };
        let path: BezPath = vector_path(&v, 1.0, 1.0);
        cx.stroke_path(&path, 3.0, alpha(WHITE, 0.9));
        cx.stroke_path(&path, 1.5, hex(accent()));
    }
    let n = pen.anchors.len();
    let near_first =
        n >= 2 && Point::new(pts[0].x, pts[0].y).distance(cursor) <= CLOSE_PX && !dragging;
    for (i, a) in pts.iter().take(n).enumerate() {
        for (hx, hy) in [(a.in_x, a.in_y), (a.out_x, a.out_y)] {
            if hx != 0.0 || hy != 0.0 {
                let mut line = BezPath::new();
                line.move_to((a.x, a.y));
                line.line_to((a.x + hx, a.y + hy));
                cx.stroke_path(&line, 1.0, hex(accent()));
                cx.circle(a.x + hx, a.y + hy, 3.0, hex(accent()));
            }
        }
        let r = Rect::new(a.x - POINT, a.y - POINT, a.x + POINT, a.y + POINT);
        let hot = i == 0 && near_first;
        cx.fill(r.inflate(1.0, 1.0), 2.0, hex(accent()));
        cx.fill(r, 1.5, if hot { hex(accent()) } else { hex(WHITE) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_needs_two_points_and_its_box_fits_the_curve() {
        assert!(build_node(&[Anchor::corner(1.0, 1.0)], false).is_none());
        let pts = [Anchor::corner(10.0, 20.0), Anchor::corner(110.0, 70.0)];
        let (node, b) = build_node(&pts, false).unwrap();
        assert_eq!((b.x0, b.y0, b.x1, b.y1), (10.0, 20.0, 110.0, 70.0));
        assert_eq!((node.geom.w, node.geom.h), (100.0, 50.0));
        let v = node.vector.unwrap();
        assert_eq!((v.anchors[0].x, v.anchors[0].y), (0.0, 0.0));
        assert_eq!((v.anchors[1].x, v.anchors[1].y), (100.0, 50.0));
        assert!(node.fill.is_none() && node.stroke.is_some());
    }

    #[test]
    fn a_straight_horizontal_line_still_gets_a_box_with_height() {
        let pts = [Anchor::corner(0.0, 5.0), Anchor::corner(40.0, 5.0)];
        let (node, _) = build_node(&pts, false).unwrap();
        assert_eq!((node.geom.w, node.geom.h), (40.0, 1.0));
        assert_eq!(node.vector.unwrap().size, (40.0, 1.0));
    }

    #[test]
    fn curve_bulges_enlarge_the_box() {
        let pts = [
            Anchor::smooth(0.0, 0.0, 0.0, 60.0),
            Anchor::smooth(100.0, 0.0, 0.0, -60.0),
        ];
        let (node, _) = build_node(&pts, false).unwrap();
        assert!(node.geom.h > 30.0);
    }
}
