//! Turning the selection by a fixed angle and mirroring it (the buttons of the Position section).

use dv_core::{Command, Executor, Geometry, NodeId};
use kurbo::Point;

use super::handles::{normalize_angle, rotate_point};

/// The direction of a mirror.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flip {
    Horizontal,
    Vertical,
}

/// The four buttons of the Position section, in the order they are shown.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    TurnLeft,
    TurnRight,
    FlipHorizontal,
    FlipVertical,
}

impl Action {
    pub const ALL: [Action; 4] = [
        Action::TurnLeft,
        Action::TurnRight,
        Action::FlipHorizontal,
        Action::FlipVertical,
    ];

    pub fn run(self, ex: &mut Executor, ids: &[NodeId]) {
        match self {
            Action::TurnLeft => rotate_by(ex, ids, -90.0),
            Action::TurnRight => rotate_by(ex, ids, 90.0),
            Action::FlipHorizontal => flip(ex, ids, Flip::Horizontal),
            Action::FlipVertical => flip(ex, ids, Flip::Vertical),
        }
    }
}

impl super::state::Editor {
    /// The turn or mirror button under the pointer (only when something is selected).
    pub(super) fn action_hit(&self) -> Option<Action> {
        let right = self.layout().right;
        if self.selection.is_empty() || !right.contains(self.mouse) {
            return None;
        }
        Action::ALL
            .into_iter()
            .enumerate()
            .find(|(k, _)| super::panels::action_rect(right, *k).contains(self.mouse))
            .map(|(_, a)| a)
    }

    pub(super) fn run_action(&mut self, action: Action) {
        action.run(&mut self.session.borrow_mut().exec, &self.selection);
    }
}

/// Turns the selection by `deg` (clockwise on screen) around the middle of its bounds.
pub fn rotate_by(ex: &mut Executor, ids: &[NodeId], deg: f64) {
    let page = ex.doc().page();
    let tops = page.geo_tops(ids);
    let Some(bounds) = dv_geom::selection_bounds(page, &tops) else {
        return;
    };
    let center = bounds.center();
    let cmds: Vec<Command> = tops
        .iter()
        .filter_map(|id| {
            let n = page.node(*id)?;
            let g = n.geom;
            let c = dv_geom::world_affine(page, *id) * Point::new(g.w / 2.0, g.h / 2.0);
            let p = dv_geom::to_parent_space(page, n.parent, rotate_point(center, c, deg));
            let geom = Geometry {
                x: p.x - g.w / 2.0,
                y: p.y - g.h / 2.0,
                rotation: normalize_angle(g.rotation + deg),
                ..g
            };
            Some(Command::SetGeometry { id: *id, geom })
        })
        .collect();
    if !cmds.is_empty() {
        let _ = ex.execute("Rotate", Command::Batch(cmds));
    }
}

/// Mirrors the selection across the middle of its bounds. A single shape stays in place; its
/// angle is negated and what is inside it is mirrored with it.
pub fn flip(ex: &mut Executor, ids: &[NodeId], dir: Flip) {
    let page = ex.doc().page();
    let tops = page.geo_tops(ids);
    let Some(bounds) = dv_geom::selection_bounds(page, &tops) else {
        return;
    };
    let mut cmds = Vec::new();
    for id in &tops {
        let Some(n) = page.node(*id) else { continue };
        let g = n.geom;
        let c = dv_geom::world_affine(page, *id) * Point::new(g.w / 2.0, g.h / 2.0);
        let mirrored = match dir {
            Flip::Horizontal => Point::new(bounds.x0 + bounds.x1 - c.x, c.y),
            Flip::Vertical => Point::new(c.x, bounds.y0 + bounds.y1 - c.y),
        };
        let p = dv_geom::to_parent_space(page, n.parent, mirrored);
        let geom = Geometry {
            x: p.x - g.w / 2.0,
            y: p.y - g.h / 2.0,
            rotation: normalize_angle(-g.rotation),
            ..g
        };
        cmds.push(Command::SetGeometry { id: *id, geom });
        // Everything inside is mirrored in the node's own axes.
        let mut stack: Vec<(NodeId, Geometry)> = n.children.iter().map(|c| (*c, g)).collect();
        while let Some((child, parent)) = stack.pop() {
            let Some(cn) = page.node(child) else { continue };
            let cg = cn.geom;
            let mut geom = Geometry {
                rotation: normalize_angle(-cg.rotation),
                ..cg
            };
            match dir {
                Flip::Horizontal => geom.x = parent.w - cg.x - cg.w,
                Flip::Vertical => geom.y = parent.h - cg.y - cg.h,
            }
            cmds.push(Command::SetGeometry { id: child, geom });
            stack.extend(cn.children.iter().map(|c| (*c, cg)));
        }
    }
    if !cmds.is_empty() {
        let label = match dir {
            Flip::Horizontal => "Flip horizontal",
            Flip::Vertical => "Flip vertical",
        };
        let _ = ex.execute(label, Command::Batch(cmds));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Document, NodeKind};

    fn add(ex: &mut Executor, parent: Option<NodeId>, x: f64, y: f64, w: f64, h: f64) -> NodeId {
        let node = super::super::ops::new_node(
            NodeKind::Rectangle,
            "r".into(),
            Geometry {
                x,
                y,
                w,
                h,
                rotation: 0.0,
            },
        );
        let id = ex.alloc_id();
        let index = ex.doc().page().siblings(parent).len();
        let cmd = Command::Create {
            id,
            parent,
            index,
            node,
        };
        ex.execute("Create", cmd).unwrap();
        id
    }

    fn geom(ex: &Executor, id: NodeId) -> Geometry {
        ex.doc().page().node(id).unwrap().geom
    }

    #[test]
    fn a_quarter_turn_swaps_the_look_of_a_group_around_its_middle() {
        let mut ex = Executor::new(Document::default());
        let a = add(&mut ex, None, 0.0, 0.0, 10.0, 10.0);
        let b = add(&mut ex, None, 90.0, 0.0, 10.0, 10.0);
        rotate_by(&mut ex, &[a, b], 90.0);
        // The group's middle is (50, 5): a at the left goes to the top, b to the bottom.
        let (ga, gb) = (geom(&ex, a), geom(&ex, b));
        assert!(
            (ga.x - 45.0).abs() < 1e-9 && (ga.y - (-45.0)).abs() < 1e-9,
            "{ga:?}"
        );
        assert!(
            (gb.x - 45.0).abs() < 1e-9 && (gb.y - 45.0).abs() < 1e-9,
            "{gb:?}"
        );
        assert_eq!((ga.rotation, gb.rotation), (90.0, 90.0));
        ex.undo();
        assert_eq!(geom(&ex, a).rotation, 0.0, "one undo step");
    }

    #[test]
    fn four_quarter_turns_come_back() {
        let mut ex = Executor::new(Document::default());
        let a = add(&mut ex, None, 3.0, 7.0, 20.0, 10.0);
        for _ in 0..4 {
            rotate_by(&mut ex, &[a], 90.0);
        }
        let g = geom(&ex, a);
        assert!((g.x - 3.0).abs() < 1e-9 && (g.y - 7.0).abs() < 1e-9 && g.rotation == 0.0);
    }

    #[test]
    fn a_single_shape_flips_in_place_and_negates_its_angle() {
        let mut ex = Executor::new(Document::default());
        let a = add(&mut ex, None, 10.0, 20.0, 30.0, 40.0);
        rotate_by(&mut ex, &[a], 30.0);
        flip(&mut ex, &[a], Flip::Horizontal);
        let g = geom(&ex, a);
        assert_eq!(g.rotation, -30.0);
        assert!((g.x - 10.0).abs() < 1e-9 && (g.y - 20.0).abs() < 1e-9);
    }

    #[test]
    fn a_group_mirrors_across_the_middle_of_its_bounds() {
        let mut ex = Executor::new(Document::default());
        let a = add(&mut ex, None, 0.0, 0.0, 10.0, 10.0);
        let b = add(&mut ex, None, 90.0, 50.0, 10.0, 10.0);
        flip(&mut ex, &[a, b], Flip::Horizontal);
        assert_eq!((geom(&ex, a).x, geom(&ex, a).y), (90.0, 0.0));
        assert_eq!((geom(&ex, b).x, geom(&ex, b).y), (0.0, 50.0));
        flip(&mut ex, &[a, b], Flip::Vertical);
        assert_eq!((geom(&ex, a).x, geom(&ex, a).y), (90.0, 50.0));
        assert_eq!((geom(&ex, b).x, geom(&ex, b).y), (0.0, 0.0));
    }

    #[test]
    fn what_is_inside_a_flipped_frame_is_mirrored_with_it() {
        let mut ex = Executor::new(Document::default());
        let frame = add(&mut ex, None, 0.0, 0.0, 100.0, 100.0);
        let child = add(&mut ex, Some(frame), 10.0, 20.0, 30.0, 10.0);
        flip(&mut ex, &[frame], Flip::Horizontal);
        let c = geom(&ex, child);
        assert_eq!((c.x, c.y), (60.0, 20.0));
        flip(&mut ex, &[frame], Flip::Vertical);
        assert_eq!((geom(&ex, child).x, geom(&ex, child).y), (60.0, 70.0));
        flip(&mut ex, &[frame], Flip::Horizontal);
        flip(&mut ex, &[frame], Flip::Vertical);
        assert_eq!((geom(&ex, child).x, geom(&ex, child).y), (10.0, 20.0));
    }
}
