//! Document operations used by the editor. Each one is a single undo step.

use std::collections::HashMap;

use dv_core::{Color, Command, Executor, Geometry, Node, NodeId, NodeKind, Page, Props};
use dv_geom::to_parent_space;
use kurbo::Rect;

use super::handles::{MIN_SIZE, map_point, scale};

pub const DEFAULT_SIZE: f64 = 100.0;
/// Offset of a duplicate from its original, in page units.
const DUPLICATE_OFFSET: f64 = 20.0;

/// A node with the default look of its kind.
pub fn new_node(kind: NodeKind, name: String, geom: Geometry) -> Node {
    let mut node = Node::new(kind, name, geom);
    node.fill = match kind {
        NodeKind::Frame => Some(Color::rgb(255, 255, 255).into()),
        NodeKind::Rectangle | NodeKind::Ellipse => Some(Color::rgb(217, 217, 217).into()),
        NodeKind::Group | NodeKind::Path => None,
    };
    node
}

/// "Base 1", "Base 2", ...: one more than the highest number already used with `base`.
pub fn next_name(page: &Page, base: &str) -> String {
    let prefix = format!("{base} ");
    let used = page
        .draw_order()
        .iter()
        .filter_map(|id| page.node(*id))
        .filter_map(|n| n.name.strip_prefix(&prefix)?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("{prefix}{}", used + 1)
}

/// Creates a node on top of its siblings. Returns the new id.
pub fn create(
    ex: &mut Executor,
    kind: NodeKind,
    name: String,
    parent: Option<NodeId>,
    geom: Geometry,
) -> Option<NodeId> {
    let id = ex.alloc_id();
    let index = ex.doc().page().siblings(parent).len();
    let node = new_node(kind, name, geom);
    ex.execute(
        "Create",
        Command::Create {
            id,
            parent,
            index,
            node,
        },
    )
    .ok()
    .map(|()| id)
}

/// Deletes the nodes (and what is inside them) as one step.
pub fn delete(ex: &mut Executor, ids: &[NodeId]) {
    let top = ex.doc().page().without_descendants(ids);
    let cmds: Vec<Command> = top
        .into_iter()
        .filter(|id| ex.doc().page().contains(*id))
        .map(|id| Command::Delete { id })
        .collect();
    if !cmds.is_empty() {
        let _ = ex.execute("Delete", Command::Batch(cmds));
    }
}

/// Sets the geometry of several nodes; edits with the same `key` merge into one step.
pub fn set_geometry(ex: &mut Executor, key: u64, label: &'static str, list: &[(NodeId, Geometry)]) {
    let cmds: Vec<Command> = list
        .iter()
        .map(|(id, geom)| Command::SetGeometry {
            id: *id,
            geom: *geom,
        })
        .collect();
    if !cmds.is_empty() {
        let _ = ex.execute_merged(label, key, Command::Batch(cmds));
    }
}

/// Moves nodes by a page-space offset as one undo step.
pub fn nudge(ex: &mut Executor, ids: &[NodeId], dx: f64, dy: f64) {
    let page = ex.doc().page();
    let list: Vec<(NodeId, Geometry)> = page
        .geo_tops(ids)
        .into_iter()
        .filter_map(|id| {
            let g = page.node(id)?.geom;
            let d = dv_geom::parent_delta(page, id, kurbo_vec(dx, dy));
            Some((
                id,
                Geometry {
                    x: g.x + d.x,
                    y: g.y + d.y,
                    ..g
                },
            ))
        })
        .collect();
    let cmds: Vec<Command> = list
        .into_iter()
        .map(|(id, geom)| Command::SetGeometry { id, geom })
        .collect();
    if !cmds.is_empty() {
        let _ = ex.execute("Move", Command::Batch(cmds));
    }
}

fn kurbo_vec(x: f64, y: f64) -> kurbo::Vec2 {
    kurbo::Vec2::new(x, y)
}

/// Copies the nodes with their subtrees, slightly offset, directly above the originals.
/// Returns the ids of the copies of the given nodes.
pub fn duplicate(ex: &mut Executor, ids: &[NodeId]) -> Vec<NodeId> {
    let tops = ex.doc().page().without_descendants(ids);
    let mut cmds = Vec::new();
    let mut copies = Vec::new();
    for top in tops {
        let page = ex.doc().page();
        let Some(root) = page.node(top) else { continue };
        let (parent, index) = (
            root.parent,
            page.siblings(root.parent)
                .iter()
                .position(|x| *x == top)
                .map_or(0, |i| i + 1),
        );
        let mut map: HashMap<NodeId, NodeId> = HashMap::new();
        for old in page.subtree(top) {
            let new = ex.alloc_id();
            map.insert(old, new);
        }
        let page = ex.doc().page();
        for old in page.subtree(top) {
            let Some(src) = page.node(old) else { continue };
            let mut node = src.clone();
            if old == top {
                node.geom.x += DUPLICATE_OFFSET;
                node.geom.y += DUPLICATE_OFFSET;
            }
            let new_parent = if old == top {
                parent
            } else {
                src.parent.and_then(|p| map.get(&p).copied())
            };
            let index = if old == top { index } else { usize::MAX };
            cmds.push(Command::Create {
                id: map[&old],
                parent: new_parent,
                index,
                node,
            });
        }
        copies.push(map[&top]);
    }
    if !cmds.is_empty() {
        let _ = ex.execute("Duplicate", Command::Batch(cmds));
    }
    copies
}

/// Shows or hides a node.
pub fn set_visible(ex: &mut Executor, id: NodeId, visible: bool) {
    let props = Props {
        visible: Some(visible),
        ..Props::default()
    };
    let _ = ex.execute("Visibility", Command::SetProps { id, props });
}

/// Adds `n` shapes spread over a large area, as one undo step (a performance test page).
pub fn stress(ex: &mut Executor, n: usize) -> Vec<NodeId> {
    let (world_w, world_h) = (20_000.0, 12_000.0);
    let mut seed = 0x2545_F491_4F6C_DD1D_u64 ^ ex.doc().revision();
    let mut next = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as f64) / ((1u64 << 31) as f64)
    };
    let base = ex.doc().page().roots().len();
    let mut ids = Vec::with_capacity(n);
    let mut cmds = Vec::with_capacity(n);
    for i in 0..n {
        let (w, h) = (20.0 + next() * 180.0, 20.0 + next() * 180.0);
        let kind = if i % 3 == 0 {
            NodeKind::Ellipse
        } else {
            NodeKind::Rectangle
        };
        let geom = Geometry {
            x: next() * (world_w - w),
            y: next() * (world_h - h),
            w,
            h,
            rotation: if i % 5 == 0 { next() * 90.0 } else { 0.0 },
        };
        let mut node = Node::new(kind, format!("Shape {}", i + 1), geom);
        node.fill = Some(
            Color::rgb(
                (next() * 255.0) as u8,
                (next() * 255.0) as u8,
                (next() * 255.0) as u8,
            )
            .into(),
        );
        node.radius = if i % 4 == 0 { 12.0 } else { 0.0 };
        let id = ex.alloc_id();
        cmds.push(Command::Create {
            id,
            parent: None,
            index: base + i,
            node,
        });
        ids.push(id);
    }
    let _ = ex.execute("Stress test", Command::Batch(cmds));
    ids
}

/// Geometry of nodes after their selection box changes from `orig` to `new` (page space).
/// `originals` are the nodes with their geometry and world bounds before the change.
pub fn resized(
    page: &Page,
    originals: &[(NodeId, Geometry, Rect)],
    orig: Rect,
    new: Rect,
) -> Vec<(NodeId, Geometry)> {
    let (sx, sy) = scale(orig, new);
    originals
        .iter()
        .map(|(id, g, wb)| {
            let (w, h) = ((g.w * sx).max(MIN_SIZE), (g.h * sy).max(MIN_SIZE));
            let center = map_point(orig, new, wb.center());
            let parent = page.node(*id).and_then(|n| n.parent);
            let p = to_parent_space(page, parent, center);
            let geom = Geometry {
                x: p.x - w / 2.0,
                y: p.y - h / 2.0,
                w,
                h,
                rotation: g.rotation,
            };
            (*id, geom)
        })
        .collect()
}

/// The numeric properties that can be typed into the right panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    X,
    Y,
    W,
    H,
    Rotation,
    Radius,
    /// Opacity of the shape in percent.
    Opacity,
}

impl Field {
    /// In the order of the panel: Position (X, Y, angle), Layout (W, H), Appearance (radius).
    pub const ALL: [Field; 7] = [
        Field::X,
        Field::Y,
        Field::Rotation,
        Field::W,
        Field::H,
        Field::Radius,
        Field::Opacity,
    ];

    pub fn from_index(i: usize) -> Option<Field> {
        Field::ALL.get(i).copied()
    }

    pub fn index(self) -> usize {
        Field::ALL.iter().position(|f| *f == self).unwrap_or(0)
    }
}

/// Applies a typed property value to the nodes as one undo step. Returns false when nothing
/// changed (bad value, unknown nodes).
pub fn set_field(ex: &mut Executor, ids: &[NodeId], field: Field, value: f64) -> bool {
    if !value.is_finite() {
        return false;
    }
    let page = ex.doc().page();
    let tops: Vec<NodeId> = page
        .geo_tops(ids)
        .into_iter()
        .filter(|id| page.contains(*id))
        .collect();
    let mut cmds: Vec<Command> = Vec::new();
    let geoms = |tops: &[NodeId]| -> Vec<(NodeId, Geometry)> {
        tops.iter()
            .filter_map(|id| Some((*id, page.node(*id)?.geom)))
            .collect()
    };
    match field {
        Field::X | Field::Y => {
            let Some(b) = dv_geom::selection_bounds(page, &tops) else {
                return false;
            };
            let single = tops.len() == 1;
            for (id, g) in geoms(&tops) {
                let mut geom = g;
                if single {
                    // One shape: the typed value is its position in its parent.
                    if field == Field::X {
                        geom.x = value
                    } else {
                        geom.y = value
                    }
                } else {
                    let delta = if field == Field::X {
                        kurbo_vec(value - b.x0, 0.0)
                    } else {
                        kurbo_vec(0.0, value - b.y0)
                    };
                    let d = dv_geom::parent_delta(page, id, delta);
                    geom.x += d.x;
                    geom.y += d.y;
                }
                cmds.push(Command::SetGeometry { id, geom });
            }
        }
        Field::W | Field::H => {
            let value = value.max(MIN_SIZE);
            if let [id] = tops[..] {
                let Some(n) = page.node(id) else { return false };
                let mut geom = n.geom;
                if field == Field::W {
                    geom.w = value
                } else {
                    geom.h = value
                }
                cmds.push(Command::SetGeometry { id, geom });
            } else if let Some(orig) = dv_geom::selection_bounds(page, &tops) {
                let new = if field == Field::W {
                    Rect::new(orig.x0, orig.y0, orig.x0 + value, orig.y1)
                } else {
                    Rect::new(orig.x0, orig.y0, orig.x1, orig.y0 + value)
                };
                let originals: Vec<(NodeId, Geometry, Rect)> = tops
                    .iter()
                    .filter_map(|id| {
                        Some((*id, page.node(*id)?.geom, dv_geom::world_bounds(page, *id)?))
                    })
                    .collect();
                for (id, geom) in resized(page, &originals, orig, new) {
                    cmds.push(Command::SetGeometry { id, geom });
                }
            }
        }
        Field::Rotation => {
            let angle = super::handles::normalize_angle(value);
            for (id, g) in geoms(&tops) {
                cmds.push(Command::SetGeometry {
                    id,
                    geom: Geometry {
                        rotation: angle,
                        ..g
                    },
                });
            }
        }
        Field::Opacity => {
            for id in ids.iter().filter(|id| page.contains(**id)) {
                let props = Props {
                    opacity: Some((value / 100.0).clamp(0.0, 1.0)),
                    ..Props::default()
                };
                cmds.push(Command::SetProps { id: *id, props });
            }
        }
        Field::Radius => {
            for id in ids {
                let Some(n) = page.node(*id) else { continue };
                if n.kind != NodeKind::Ellipse {
                    let props = Props {
                        radius: Some(value.max(0.0)),
                        ..Props::default()
                    };
                    cmds.push(Command::SetProps { id: *id, props });
                }
            }
        }
    }
    if cmds.is_empty() {
        return false;
    }
    ex.execute("Edit property", Command::Batch(cmds)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::Document;

    fn exec() -> Executor {
        Executor::new(Document::default())
    }

    fn g(x: f64, y: f64) -> Geometry {
        Geometry {
            x,
            y,
            w: 50.0,
            h: 40.0,
            rotation: 0.0,
        }
    }

    #[test]
    fn names_count_up_per_base() {
        let mut ex = exec();
        assert_eq!(next_name(ex.doc().page(), "Rectangle"), "Rectangle 1");
        create(
            &mut ex,
            NodeKind::Rectangle,
            "Rectangle 1".into(),
            None,
            g(0.0, 0.0),
        );
        create(
            &mut ex,
            NodeKind::Rectangle,
            "Rectangle 5".into(),
            None,
            g(0.0, 0.0),
        );
        assert_eq!(next_name(ex.doc().page(), "Rectangle"), "Rectangle 6");
        assert_eq!(next_name(ex.doc().page(), "Ellipse"), "Ellipse 1");
    }

    #[test]
    fn create_puts_the_node_on_top_with_a_default_fill() {
        let mut ex = exec();
        let a = create(&mut ex, NodeKind::Rectangle, "a".into(), None, g(0.0, 0.0)).unwrap();
        let b = create(&mut ex, NodeKind::Ellipse, "b".into(), None, g(1.0, 1.0)).unwrap();
        assert_eq!(ex.doc().page().roots(), [a, b]);
        assert!(ex.doc().page().node(a).unwrap().fill.is_some());
        let f = create(&mut ex, NodeKind::Frame, "f".into(), None, g(0.0, 0.0)).unwrap();
        assert_eq!(
            ex.doc().page().node(f).unwrap().fill,
            Some(Color::rgb(255, 255, 255).into())
        );
    }

    #[test]
    fn delete_is_one_undoable_step_and_skips_nested_duplicates() {
        let mut ex = exec();
        let f = create(&mut ex, NodeKind::Frame, "f".into(), None, g(0.0, 0.0)).unwrap();
        let k = create(
            &mut ex,
            NodeKind::Rectangle,
            "k".into(),
            Some(f),
            g(1.0, 1.0),
        )
        .unwrap();
        let o = create(&mut ex, NodeKind::Rectangle, "o".into(), None, g(5.0, 5.0)).unwrap();
        delete(&mut ex, &[f, k, o]);
        assert!(ex.doc().page().is_empty());
        ex.undo();
        assert_eq!(ex.doc().page().len(), 3);
        assert_eq!(ex.doc().page().node(k).unwrap().parent, Some(f));
    }

    #[test]
    fn nudge_moves_and_undoes_in_one_step() {
        let mut ex = exec();
        let a = create(
            &mut ex,
            NodeKind::Rectangle,
            "a".into(),
            None,
            g(10.0, 10.0),
        )
        .unwrap();
        let b = create(
            &mut ex,
            NodeKind::Rectangle,
            "b".into(),
            None,
            g(20.0, 20.0),
        )
        .unwrap();
        nudge(&mut ex, &[a, b], 5.0, -3.0);
        let x = |id| ex.doc().page().node(id).map(|n| (n.geom.x, n.geom.y));
        assert_eq!((x(a), x(b)), (Some((15.0, 7.0)), Some((25.0, 17.0))));
        ex.undo();
        let x = |id| ex.doc().page().node(id).map(|n| (n.geom.x, n.geom.y));
        assert_eq!((x(a), x(b)), (Some((10.0, 10.0)), Some((20.0, 20.0))));
    }

    #[test]
    fn nudging_a_frame_and_its_child_moves_the_child_only_once() {
        let mut ex = exec();
        let f = create(&mut ex, NodeKind::Frame, "f".into(), None, g(0.0, 0.0)).unwrap();
        let k = create(
            &mut ex,
            NodeKind::Rectangle,
            "k".into(),
            Some(f),
            g(3.0, 3.0),
        )
        .unwrap();
        nudge(&mut ex, &[f, k], 10.0, 0.0);
        assert_eq!(ex.doc().page().node(f).unwrap().geom.x, 10.0);
        assert_eq!(
            ex.doc().page().node(k).unwrap().geom.x,
            3.0,
            "child is relative to the frame"
        );
    }

    #[test]
    fn duplicate_copies_subtrees_with_new_ids_above_the_original() {
        let mut ex = exec();
        let f = create(&mut ex, NodeKind::Frame, "f".into(), None, g(0.0, 0.0)).unwrap();
        let k = create(
            &mut ex,
            NodeKind::Rectangle,
            "k".into(),
            Some(f),
            g(3.0, 3.0),
        )
        .unwrap();
        let top = create(
            &mut ex,
            NodeKind::Rectangle,
            "top".into(),
            None,
            g(50.0, 50.0),
        )
        .unwrap();
        let copies = duplicate(&mut ex, &[f]);
        assert_eq!(copies.len(), 1);
        let page = ex.doc().page();
        assert_eq!(page.len(), 5);
        assert_eq!(
            page.roots(),
            [f, copies[0], top],
            "copy sits right above its original"
        );
        let copy = page.node(copies[0]).unwrap();
        assert_eq!(
            (copy.geom.x, copy.geom.y),
            (DUPLICATE_OFFSET, DUPLICATE_OFFSET)
        );
        assert_eq!(copy.children.len(), 1);
        assert_ne!(copy.children[0], k);
        let child = page.node(copy.children[0]).unwrap();
        assert_eq!(child.parent, Some(copies[0]));
        assert_eq!(
            (child.geom.x, child.geom.y),
            (3.0, 3.0),
            "children keep their local position"
        );
        ex.undo();
        assert_eq!(ex.doc().page().len(), 3, "one undo removes the whole copy");
    }

    #[test]
    fn stress_adds_n_shapes_in_one_undo_step() {
        let mut ex = exec();
        let ids = stress(&mut ex, 1000);
        assert_eq!(ids.len(), 1000);
        assert_eq!(ex.doc().page().len(), 1000);
        ex.undo();
        assert!(ex.doc().page().is_empty());
        ex.redo();
        assert_eq!(ex.doc().page().len(), 1000);
    }

    #[test]
    fn merged_geometry_edits_form_one_step() {
        let mut ex = exec();
        let a = create(&mut ex, NodeKind::Rectangle, "a".into(), None, g(0.0, 0.0)).unwrap();
        for x in [1.0, 2.0, 3.0] {
            set_geometry(&mut ex, 9, "Move", &[(a, g(x, 0.0))]);
        }
        ex.undo();
        assert_eq!(ex.doc().page().node(a).unwrap().geom.x, 0.0);
    }

    #[test]
    fn visibility_toggle_is_undoable() {
        let mut ex = exec();
        let a = create(&mut ex, NodeKind::Rectangle, "a".into(), None, g(0.0, 0.0)).unwrap();
        set_visible(&mut ex, a, false);
        assert!(!ex.doc().page().node(a).unwrap().visible);
        ex.undo();
        assert!(ex.doc().page().node(a).unwrap().visible);
    }

    fn sample() -> (Executor, NodeId, NodeId) {
        let mut ex = exec();
        let a = create(
            &mut ex,
            NodeKind::Rectangle,
            "a".into(),
            None,
            g(10.0, 20.0),
        )
        .unwrap();
        let b = create(&mut ex, NodeKind::Ellipse, "b".into(), None, g(100.0, 20.0)).unwrap();
        (ex, a, b)
    }

    fn node_geom(ex: &Executor, id: NodeId) -> Geometry {
        ex.doc().page().node(id).unwrap().geom
    }

    #[test]
    fn typed_position_and_size_apply_to_one_shape() {
        let (mut ex, a, _) = sample();
        assert!(set_field(&mut ex, &[a], Field::X, -40.0));
        assert!(set_field(&mut ex, &[a], Field::Y, 7.5));
        assert!(set_field(&mut ex, &[a], Field::W, 300.0));
        assert!(set_field(&mut ex, &[a], Field::H, 0.0));
        let g = node_geom(&ex, a);
        assert_eq!(
            (g.x, g.y, g.w, g.h),
            (-40.0, 7.5, 300.0, MIN_SIZE),
            "size never drops below the minimum"
        );
        ex.undo();
        assert_eq!(
            node_geom(&ex, a).h,
            40.0,
            "each field edit is its own undo step"
        );
    }

    #[test]
    fn typed_rotation_is_normalized_and_keeps_the_center() {
        let (mut ex, a, _) = sample();
        let before = node_geom(&ex, a);
        assert!(set_field(&mut ex, &[a], Field::Rotation, 270.0));
        let g = node_geom(&ex, a);
        assert!((g.rotation + 90.0).abs() < 1e-9);
        assert_eq!(
            (g.x + g.w / 2.0, g.y + g.h / 2.0),
            (before.x + before.w / 2.0, before.y + before.h / 2.0)
        );
    }

    #[test]
    fn radius_applies_to_rectangles_only_and_never_goes_negative() {
        let (mut ex, a, b) = sample();
        assert!(set_field(&mut ex, &[a, b], Field::Radius, -5.0));
        assert_eq!(ex.doc().page().node(a).unwrap().radius, 0.0);
        assert!(set_field(&mut ex, &[a, b], Field::Radius, 12.0));
        assert_eq!(ex.doc().page().node(a).unwrap().radius, 12.0);
        assert_eq!(
            ex.doc().page().node(b).unwrap().radius,
            0.0,
            "ellipses have no radius"
        );
        assert!(
            !set_field(&mut ex, &[b], Field::Radius, 3.0),
            "nothing to change for an ellipse"
        );
    }

    #[test]
    fn typed_x_of_a_group_moves_the_whole_group() {
        let (mut ex, a, b) = sample();
        // The group spans x 10 .. 150; put its left edge at 0.
        assert!(set_field(&mut ex, &[a, b], Field::X, 0.0));
        assert_eq!((node_geom(&ex, a).x, node_geom(&ex, b).x), (0.0, 90.0));
        ex.undo();
        assert_eq!((node_geom(&ex, a).x, node_geom(&ex, b).x), (10.0, 100.0));
    }

    #[test]
    fn typed_width_of_a_group_scales_it() {
        let (mut ex, a, b) = sample();
        // The group is 140 wide (10 .. 150); make it 280 wide: everything doubles horizontally.
        assert!(set_field(&mut ex, &[a, b], Field::W, 280.0));
        let (ga, gb) = (node_geom(&ex, a), node_geom(&ex, b));
        assert_eq!((ga.x, ga.w), (10.0, 100.0));
        assert_eq!((gb.x, gb.w), (190.0, 100.0));
    }

    #[test]
    fn bad_values_and_unknown_nodes_change_nothing() {
        let (mut ex, a, _) = sample();
        assert!(!set_field(&mut ex, &[a], Field::X, f64::NAN));
        assert!(!set_field(&mut ex, &[a], Field::W, f64::INFINITY));
        assert!(!set_field(&mut ex, &[NodeId(9999)], Field::X, 1.0));
        assert!(!set_field(&mut ex, &[], Field::X, 1.0));
        assert_eq!(node_geom(&ex, a).x, 10.0);
    }

    #[test]
    fn typed_opacity_is_clamped_and_undoable() {
        let (mut ex, a, _) = sample();
        assert!(set_field(&mut ex, &[a], Field::Opacity, 40.0));
        assert_eq!(ex.doc().page().node(a).unwrap().opacity, 0.4);
        assert!(set_field(&mut ex, &[a], Field::Opacity, 400.0));
        assert_eq!(ex.doc().page().node(a).unwrap().opacity, 1.0);
        ex.undo();
        assert_eq!(ex.doc().page().node(a).unwrap().opacity, 0.4);
    }

    #[test]
    fn fields_map_to_panel_slots() {
        for (i, f) in Field::ALL.iter().enumerate() {
            assert_eq!(f.index(), i);
            assert_eq!(Field::from_index(i), Some(*f));
        }
        assert_eq!(Field::from_index(7), None);
    }
}
