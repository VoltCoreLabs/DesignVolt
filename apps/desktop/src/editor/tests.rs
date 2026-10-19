//! Tests that drive the editor like a user: mouse and keyboard events in, document out.

use std::cell::RefCell;
use std::rc::Rc;

use dv_core::{NodeId, NodeKind};
use dv_ui::icons::Icons;
use dv_ui::text::Font;
use kurbo::Point;

use super::session::Session;
use super::state::Editor;
use crate::input::{Assets, Key, Mods, Screen};

const NONE: Mods = Mods {
    ctrl: false,
    shift: false,
};
const SHIFT: Mods = Mods {
    ctrl: false,
    shift: true,
};
const CTRL: Mods = Mods {
    ctrl: true,
    shift: false,
};
const CTRL_SHIFT: Mods = Mods {
    ctrl: true,
    shift: true,
};

fn editor() -> Editor {
    let font = Font::from_bytes(include_bytes!("../../../../assets/fonts/GolosText.ttf")).unwrap();
    let assets = Rc::new(Assets {
        font,
        icons: Icons::new(),
    });
    let session = Rc::new(RefCell::new(Session::new("Test")));
    let mut ed = Editor::new(assets, session, "Test".into());
    ed.set_viewport(1180, 760, 1.0);
    ed.update(0.016);
    ed
}

/// Window position of the page point (wx, wy) at the current zoom and pan.
fn at(ed: &Editor, wx: f64, wy: f64) -> (f64, f64) {
    let c = ed.layout().canvas;
    let p = ed.session.borrow().vp.to_screen(Point::new(wx, wy));
    (c.x0 + p.x, c.y0 + p.y)
}

fn mv(ed: &mut Editor, wx: f64, wy: f64) {
    let (x, y) = at(ed, wx, wy);
    ed.mouse_moved(x, y);
}

fn drag(ed: &mut Editor, from: (f64, f64), to: (f64, f64), mods: Mods) {
    mv(ed, from.0, from.1);
    ed.click(mods);
    mv(ed, (from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
    mv(ed, to.0, to.1);
    ed.mouse_up();
}

fn press_key(ed: &mut Editor, key: Key, mods: Mods) {
    ed.key(key, mods);
}

fn roots(ed: &Editor) -> Vec<NodeId> {
    ed.session.borrow().exec.doc().page().roots().to_vec()
}

fn geom(ed: &Editor, id: NodeId) -> (f64, f64, f64, f64) {
    let s = ed.session.borrow();
    let g = s.exec.doc().page().node(id).unwrap().geom;
    (g.x, g.y, g.w, g.h)
}

fn draw_rect(ed: &mut Editor, from: (f64, f64), to: (f64, f64)) -> NodeId {
    press_key(ed, Key::Char('r'), NONE);
    drag(ed, from, to, NONE);
    *roots(ed).last().expect("a node was created")
}

#[test]
fn dragging_with_the_rectangle_tool_draws_a_rectangle() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (200.0, 100.0));
    assert_eq!(geom(&ed, id), (0.0, 0.0, 200.0, 100.0));
    assert_eq!(ed.selection, vec![id], "the new shape is selected");
    assert_eq!(
        ed.tool,
        super::state::Tool::Select,
        "the tool returns to select"
    );
    let s = ed.session.borrow();
    let n = s.exec.doc().page().node(id).unwrap();
    assert_eq!(n.kind, NodeKind::Rectangle);
    assert!(n.name.ends_with(" 1"));
}

#[test]
fn dragging_backwards_still_makes_a_positive_size() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (300.0, 200.0), (100.0, 50.0));
    assert_eq!(geom(&ed, id), (100.0, 50.0, 200.0, 150.0));
}

#[test]
fn shift_draws_a_square() {
    let mut ed = editor();
    press_key(&mut ed, Key::Char('o'), NONE);
    ed.mods = SHIFT;
    drag(&mut ed, (0.0, 0.0), (120.0, 40.0), SHIFT);
    let id = *roots(&ed).last().unwrap();
    assert_eq!(geom(&ed, id), (0.0, 0.0, 120.0, 120.0));
}

#[test]
fn a_plain_click_with_a_shape_tool_makes_a_default_sized_shape() {
    let mut ed = editor();
    press_key(&mut ed, Key::Char('f'), NONE);
    mv(&mut ed, 50.0, 60.0);
    ed.click(NONE);
    ed.mouse_up();
    let id = *roots(&ed).last().unwrap();
    assert_eq!(geom(&ed, id), (50.0, 60.0, 100.0, 100.0));
    assert_eq!(
        ed.session.borrow().exec.doc().page().node(id).unwrap().kind,
        NodeKind::Frame
    );
}

#[test]
fn a_shape_drawn_inside_a_frame_becomes_its_child() {
    let mut ed = editor();
    press_key(&mut ed, Key::Char('f'), NONE);
    drag(&mut ed, (0.0, 0.0), (400.0, 300.0), NONE);
    let frame = *roots(&ed).last().unwrap();
    draw_rect(&mut ed, (100.0, 100.0), (200.0, 160.0));
    let s = ed.session.borrow();
    let page = s.exec.doc().page();
    assert_eq!(page.roots(), [frame]);
    let child = page.siblings(Some(frame))[0];
    let g = page.node(child).unwrap().geom;
    assert_eq!(
        (g.x, g.y, g.w, g.h),
        (100.0, 100.0, 100.0, 60.0),
        "position is relative to the frame"
    );
}

#[test]
fn clicking_selects_and_dragging_moves_as_one_undo_step() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (200.0, 100.0));
    ed.selection.clear();
    drag(&mut ed, (100.0, 50.0), (150.0, 80.0), NONE);
    assert_eq!(geom(&ed, id), (50.0, 30.0, 200.0, 100.0));
    assert_eq!(ed.selection, vec![id]);
    press_key(&mut ed, Key::Undo, CTRL);
    assert_eq!(
        geom(&ed, id),
        (0.0, 0.0, 200.0, 100.0),
        "one undo reverts the whole drag"
    );
    press_key(&mut ed, Key::Redo, CTRL);
    assert_eq!(geom(&ed, id), (50.0, 30.0, 200.0, 100.0));
}

#[test]
fn holding_shift_while_moving_locks_the_axis() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (100.0, 100.0));
    ed.mods = SHIFT;
    drag(&mut ed, (50.0, 50.0), (130.0, 70.0), SHIFT);
    assert_eq!(
        geom(&ed, id),
        (80.0, 0.0, 100.0, 100.0),
        "only the dominant (x) axis moves"
    );
}

#[test]
fn a_tiny_mouse_jitter_does_not_move_a_shape() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (100.0, 100.0));
    drag(&mut ed, (50.0, 50.0), (51.0, 51.0), NONE);
    assert_eq!(geom(&ed, id), (0.0, 0.0, 100.0, 100.0));
}

#[test]
fn dragging_a_corner_handle_resizes_the_selection() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (200.0, 100.0));
    drag(&mut ed, (200.0, 100.0), (260.0, 150.0), NONE);
    assert_eq!(geom(&ed, id), (0.0, 0.0, 260.0, 150.0));
    press_key(&mut ed, Key::Undo, CTRL);
    assert_eq!(geom(&ed, id), (0.0, 0.0, 200.0, 100.0));
}

#[test]
fn dragging_the_west_handle_moves_only_the_left_edge() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (200.0, 100.0));
    drag(&mut ed, (0.0, 50.0), (-40.0, 50.0), NONE);
    assert_eq!(geom(&ed, id), (-40.0, 0.0, 240.0, 100.0));
}

#[test]
fn resizing_two_shapes_scales_them_together() {
    let mut ed = editor();
    let a = draw_rect(&mut ed, (0.0, 0.0), (100.0, 100.0));
    let b = draw_rect(&mut ed, (200.0, 0.0), (300.0, 100.0));
    press_key(&mut ed, Key::SelectAll, CTRL);
    assert_eq!(ed.selection, vec![a, b]);
    drag(&mut ed, (300.0, 100.0), (600.0, 100.0), NONE);
    // The selection grows from 300 to 600 wide, so everything doubles horizontally.
    assert_eq!(geom(&ed, a), (0.0, 0.0, 200.0, 100.0));
    assert_eq!(geom(&ed, b), (400.0, 0.0, 200.0, 100.0));
}

#[test]
fn a_marquee_selects_the_shapes_it_touches() {
    let mut ed = editor();
    let a = draw_rect(&mut ed, (0.0, 0.0), (50.0, 50.0));
    let b = draw_rect(&mut ed, (100.0, 0.0), (150.0, 50.0));
    let _far = draw_rect(&mut ed, (250.0, 200.0), (300.0, 250.0));
    ed.selection.clear();
    drag(&mut ed, (-20.0, -20.0), (170.0, 70.0), NONE);
    assert_eq!(ed.selection, vec![a, b]);
    drag(&mut ed, (-300.0, -300.0), (-250.0, -250.0), NONE);
    assert!(
        ed.selection.is_empty(),
        "an empty marquee clears the selection"
    );
}

#[test]
fn shift_click_adds_and_removes_from_the_selection() {
    let mut ed = editor();
    let a = draw_rect(&mut ed, (0.0, 0.0), (50.0, 50.0));
    let b = draw_rect(&mut ed, (100.0, 0.0), (150.0, 50.0));
    mv(&mut ed, 25.0, 25.0);
    ed.click(NONE);
    ed.mouse_up();
    assert_eq!(ed.selection, vec![a]);
    mv(&mut ed, 125.0, 25.0);
    ed.click(SHIFT);
    ed.mouse_up();
    assert_eq!(ed.selection, vec![a, b]);
    mv(&mut ed, 25.0, 25.0);
    ed.click(SHIFT);
    ed.mouse_up();
    assert_eq!(ed.selection, vec![b]);
}

#[test]
fn delete_duplicate_and_nudge_work_on_the_selection() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (100.0, 100.0));
    press_key(&mut ed, Key::Right, NONE);
    press_key(&mut ed, Key::Down, SHIFT);
    assert_eq!(geom(&ed, id), (1.0, 10.0, 100.0, 100.0));
    press_key(&mut ed, Key::Char('d'), CTRL);
    assert_eq!(roots(&ed).len(), 2);
    assert_eq!(ed.selection.len(), 1);
    assert_ne!(ed.selection[0], id, "the copy is selected");
    press_key(&mut ed, Key::Delete, NONE);
    assert_eq!(roots(&ed), vec![id]);
    press_key(&mut ed, Key::Undo, CTRL);
    assert_eq!(roots(&ed).len(), 2, "undo brings the copy back");
}

#[test]
fn tool_shortcuts_switch_tools_and_escape_goes_back() {
    let mut ed = editor();
    for (c, tool) in [
        ('r', super::state::Tool::Rectangle),
        ('o', super::state::Tool::Ellipse),
        ('f', super::state::Tool::Frame),
        ('h', super::state::Tool::Hand),
        ('v', super::state::Tool::Select),
    ] {
        press_key(&mut ed, Key::Char(c), NONE);
        assert_eq!(ed.tool, tool);
    }
    press_key(&mut ed, Key::Char('r'), NONE);
    press_key(&mut ed, Key::Escape, NONE);
    assert_eq!(ed.tool, super::state::Tool::Select);
}

#[test]
fn ctrl_wheel_zooms_around_the_pointer() {
    let mut ed = editor();
    mv(&mut ed, 120.0, 80.0);
    let before = ed.world_mouse();
    ed.wheel(0.0, 200.0, CTRL);
    assert!(ed.session.borrow().vp.zoom > 1.2);
    let after = ed.world_mouse();
    assert!((before.x - after.x).abs() < 1e-6 && (before.y - after.y).abs() < 1e-6);
    ed.wheel(0.0, -200.0, CTRL);
    assert!((ed.session.borrow().vp.zoom - 1.0).abs() < 1e-9);
}

#[test]
fn plain_wheel_pans_the_canvas() {
    let mut ed = editor();
    let before = ed.session.borrow().vp.pan;
    mv(&mut ed, 0.0, 0.0);
    ed.wheel(10.0, 30.0, NONE);
    let after = ed.session.borrow().vp.pan;
    assert_eq!((after.x - before.x, after.y - before.y), (10.0, 30.0));
}

#[test]
fn space_drag_pans_without_touching_the_document() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (100.0, 100.0));
    let before = ed.session.borrow().vp.pan;
    press_key(&mut ed, Key::Space, NONE);
    let (x, y) = at(&ed, 50.0, 50.0);
    ed.mouse_moved(x, y);
    ed.click(NONE);
    ed.mouse_moved(x + 40.0, y + 25.0);
    ed.mouse_up();
    ed.key_up(Key::Space);
    let after = ed.session.borrow().vp.pan;
    assert_eq!((after.x - before.x, after.y - before.y), (40.0, 25.0));
    assert_eq!(
        geom(&ed, id),
        (0.0, 0.0, 100.0, 100.0),
        "the shape did not move"
    );
}

#[test]
fn middle_button_pans_and_survives_a_left_release() {
    let mut ed = editor();
    let before = ed.session.borrow().vp.pan;
    let (x, y) = at(&ed, 0.0, 0.0);
    ed.mouse_moved(x, y);
    ed.middle_button(true);
    ed.mouse_moved(x + 15.0, y + 5.0);
    ed.mouse_up();
    ed.mouse_moved(x + 30.0, y + 10.0);
    ed.middle_button(false);
    let after = ed.session.borrow().vp.pan;
    assert_eq!((after.x - before.x, after.y - before.y), (30.0, 10.0));
}

#[test]
fn zoom_shortcuts_fit_and_reset() {
    let mut ed = editor();
    draw_rect(&mut ed, (0.0, 0.0), (4000.0, 2000.0));
    press_key(&mut ed, Key::Char('1'), SHIFT);
    let z = ed.session.borrow().vp.zoom;
    assert!(
        z < 0.2,
        "fits a 4000 px wide shape into the canvas, zoom {z}"
    );
    press_key(&mut ed, Key::Char('0'), SHIFT);
    assert!((ed.session.borrow().vp.zoom - 1.0).abs() < 1e-9);
    press_key(&mut ed, Key::Char('='), CTRL);
    assert!(ed.session.borrow().vp.zoom > 1.2);
}

#[test]
fn the_performance_page_adds_ten_thousand_shapes_in_one_step() {
    let mut ed = editor();
    press_key(&mut ed, Key::Char('b'), CTRL_SHIFT);
    assert_eq!(roots(&ed).len(), super::keys::STRESS_COUNT);
    ed.update(0.016);
    assert_eq!(
        ed.index.len(),
        super::keys::STRESS_COUNT,
        "the spatial index follows the document"
    );
    press_key(&mut ed, Key::Undo, CTRL);
    assert!(roots(&ed).is_empty());
}

#[test]
fn hidden_layers_cannot_be_selected_and_leave_the_selection() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (100.0, 100.0));
    super::ops::set_visible(&mut ed.session.borrow_mut().exec, id, false);
    ed.prune_selection();
    ed.update(0.016);
    assert!(ed.selection.is_empty());
    mv(&mut ed, 50.0, 50.0);
    ed.click(NONE);
    ed.mouse_up();
    assert!(ed.selection.is_empty(), "a hidden shape is not hit");
}

#[test]
fn a_session_keeps_the_document_when_the_editor_is_reopened() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (100.0, 100.0));
    let session = ed.session.clone();
    drop(ed);
    let font = Font::from_bytes(include_bytes!("../../../../assets/fonts/GolosText.ttf")).unwrap();
    let assets = Rc::new(Assets {
        font,
        icons: Icons::new(),
    });
    let again = Editor::new(assets, session, "Test".into());
    assert!(again.session.borrow().exec.doc().page().contains(id));
    assert_eq!(again.index.len(), 1);
}

#[test]
fn edits_are_autosaved_to_the_documents_file() {
    let dir =
        std::env::temp_dir().join(format!("designvolt-editor-autosave-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("documents").join("7.dvd");
    let font = Font::from_bytes(include_bytes!("../../../../assets/fonts/GolosText.ttf")).unwrap();
    let assets = Rc::new(Assets {
        font,
        icons: Icons::new(),
    });
    let session = Rc::new(RefCell::new(Session::open("Doc", path.clone())));
    let mut ed = Editor::new(assets, session.clone(), "Doc".into());
    ed.set_viewport(1180, 760, 1.0);
    ed.update(0.016);
    draw_rect(&mut ed, (0.0, 0.0), (80.0, 60.0));
    ed.update(0.5);
    assert!(!path.exists(), "not yet: the change is still fresh");
    ed.update(0.6);
    assert!(path.exists(), "saved after the delay");
    assert_eq!(session.borrow().state(), super::session::SaveState::Saved);
    let reopened = Session::open("Doc", path);
    assert_eq!(reopened.exec.doc().page().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_canvas_has_no_edges_you_can_pan_and_zoom_far_away() {
    let mut ed = editor();
    mv(&mut ed, 0.0, 0.0);
    for _ in 0..200 {
        ed.wheel(-5000.0, 3000.0, NONE);
    }
    let pan = ed.session.borrow().vp.pan;
    assert!(pan.x < -900_000.0 && pan.y > 500_000.0, "{pan:?}");
    // Far from the origin you can still draw and pick.
    let c = ed.session.borrow().vp.to_world(Point::new(330.0, 360.0));
    assert!(c.x > 900_000.0 && c.y < -500_000.0, "{c:?}");
    let id = draw_rect(&mut ed, (c.x, c.y), (c.x + 100.0, c.y + 50.0));
    assert!(ed.session.borrow().exec.doc().page().contains(id));
}

/// Drags with the rotate grip: starts just outside the top-right corner of the selection.
fn rotate_drag(ed: &mut Editor, corner: (f64, f64), center: (f64, f64), degrees: f64, mods: Mods) {
    let start = (corner.0 + 12.0 / 1.0, corner.1 - 12.0);
    let v = (start.0 - center.0, start.1 - center.1);
    let (s, c) = degrees.to_radians().sin_cos();
    let end = (center.0 + v.0 * c - v.1 * s, center.1 + v.0 * s + v.1 * c);
    mv(ed, start.0, start.1);
    ed.click(mods);
    mv(ed, (start.0 + end.0) / 2.0, (start.1 + end.1) / 2.0);
    mv(ed, end.0, end.1);
    ed.mouse_up();
}

fn rotation_of(ed: &Editor, id: NodeId) -> f64 {
    ed.session
        .borrow()
        .exec
        .doc()
        .page()
        .node(id)
        .unwrap()
        .geom
        .rotation
}

#[test]
fn dragging_outside_a_corner_rotates_the_shape_around_its_center() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (200.0, 100.0));
    rotate_drag(&mut ed, (200.0, 0.0), (100.0, 50.0), 90.0, NONE);
    assert!(
        (rotation_of(&ed, id) - 90.0).abs() < 1e-6,
        "{}",
        rotation_of(&ed, id)
    );
    let (x, y, w, h) = geom(&ed, id);
    assert!(
        (x + w / 2.0 - 100.0).abs() < 1e-6 && (y + h / 2.0 - 50.0).abs() < 1e-6,
        "center stays"
    );
    press_key(&mut ed, Key::Undo, CTRL);
    assert_eq!(
        rotation_of(&ed, id),
        0.0,
        "one undo reverts the whole rotation"
    );
}

#[test]
fn shift_snaps_the_rotation_to_fifteen_degrees() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (200.0, 100.0));
    ed.mods = SHIFT;
    rotate_drag(&mut ed, (200.0, 0.0), (100.0, 50.0), 50.0, SHIFT);
    assert!(
        (rotation_of(&ed, id) - 45.0).abs() < 1e-6,
        "{}",
        rotation_of(&ed, id)
    );
}

#[test]
fn rotating_twice_adds_up_and_stays_normalized() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (100.0, 100.0));
    rotate_drag(&mut ed, (100.0, 0.0), (50.0, 50.0), 100.0, NONE);
    assert!((rotation_of(&ed, id) - 100.0).abs() < 1e-6);
    // The selection box is now the rotated shape's bounds; grab outside its top-right corner.
    let b = ed.selection_screen_bounds().unwrap();
    let z = ed.session.borrow().vp.zoom;
    let corner = ((b.x1) / z - 330.0 / z, (b.y0) / z - 360.0 / z);
    rotate_drag(&mut ed, corner, (50.0, 50.0), 100.0, NONE);
    assert!(
        (rotation_of(&ed, id) + 160.0).abs() < 1e-6,
        "200 degrees wraps to -160: {}",
        rotation_of(&ed, id)
    );
}

#[test]
fn rotating_a_group_turns_the_shapes_around_the_group_center() {
    let mut ed = editor();
    let a = draw_rect(&mut ed, (0.0, 0.0), (100.0, 100.0));
    let b = draw_rect(&mut ed, (200.0, 0.0), (300.0, 100.0));
    press_key(&mut ed, Key::SelectAll, CTRL);
    rotate_drag(&mut ed, (300.0, 0.0), (150.0, 50.0), 180.0, NONE);
    let center = |id| {
        let (x, y, w, h) = geom(&ed, id);
        (x + w / 2.0, y + h / 2.0)
    };
    let (ca, cb) = (center(a), center(b));
    assert!(
        (ca.0 - 250.0).abs() < 1e-6 && (ca.1 - 50.0).abs() < 1e-6,
        "{ca:?}"
    );
    assert!(
        (cb.0 - 50.0).abs() < 1e-6 && (cb.1 - 50.0).abs() < 1e-6,
        "{cb:?}"
    );
    for id in [a, b] {
        let turned = rotation_of(&ed, id).abs();
        assert!(
            (turned - 180.0).abs() < 1e-6,
            "a half turn, either sign: {turned}"
        );
    }
}

#[test]
fn moving_a_shape_snaps_its_edge_to_a_neighbour_and_shows_a_guide() {
    let mut ed = editor();
    let _a = draw_rect(&mut ed, (-200.0, 0.0), (-100.0, 100.0));
    let b = draw_rect(&mut ed, (100.0, 0.0), (200.0, 100.0));
    // Grab b in the middle and pull it left; its left edge would end 3 units from a's right edge.
    mv(&mut ed, 150.0, 50.0);
    ed.click(NONE);
    mv(&mut ed, 50.0, 50.0);
    mv(&mut ed, -47.0, 50.0);
    assert!(!ed.guides.is_empty(), "a guide is visible while snapped");
    ed.mouse_up();
    assert_eq!(
        geom(&ed, b),
        (-100.0, 0.0, 100.0, 100.0),
        "the left edge snapped onto x = -100"
    );
    assert!(ed.guides.is_empty(), "guides disappear when the drag ends");
}

#[test]
fn holding_ctrl_turns_snapping_off() {
    let mut ed = editor();
    let _a = draw_rect(&mut ed, (-200.0, 0.0), (-100.0, 100.0));
    let b = draw_rect(&mut ed, (100.0, 0.0), (200.0, 100.0));
    ed.mods = CTRL;
    drag(&mut ed, (150.0, 50.0), (-47.0, 50.0), CTRL);
    assert_eq!(geom(&ed, b), (-97.0, 0.0, 100.0, 100.0));
}

#[test]
fn the_snap_distance_is_measured_on_screen_not_in_page_units() {
    let mut ed = editor();
    // At 400 % zoom, 5 page units are 20 px: far too much to snap (the limit is 6 px).
    ed.session
        .borrow_mut()
        .vp
        .set_zoom_at(Point::new(330.0, 360.0), 4.0);
    let _a = draw_rect(&mut ed, (0.0, 0.0), (20.0, 20.0));
    let b = draw_rect(&mut ed, (30.0, 0.0), (50.0, 20.0));
    drag(&mut ed, (40.0, 10.0), (35.0, 10.0), NONE);
    assert_eq!(geom(&ed, b).0, 25.0, "no snap at 20 px distance");
    // Within 1.5 units (6 px) it does snap.
    drag(&mut ed, (35.0, 10.0), (31.0, 10.0), NONE);
    assert_eq!(geom(&ed, b).0, 20.0, "21 snaps to a's right edge at 20");
}

#[test]
fn resizing_snaps_the_dragged_edge_and_shift_aspect_does_not() {
    let mut ed = editor();
    let _a = draw_rect(&mut ed, (200.0, 0.0), (300.0, 100.0));
    let b = draw_rect(&mut ed, (0.0, 0.0), (100.0, 100.0));
    // Drag b's east handle to x = 197: it snaps to a's left edge at 200.
    drag(&mut ed, (100.0, 50.0), (197.0, 50.0), NONE);
    assert_eq!(geom(&ed, b), (0.0, 0.0, 200.0, 100.0));
}

#[test]
fn moving_a_shape_does_not_snap_to_itself_or_to_its_own_children() {
    let mut ed = editor();
    press_key(&mut ed, Key::Char('f'), NONE);
    drag(&mut ed, (0.0, 0.0), (400.0, 300.0), NONE);
    let frame = *roots(&ed).last().unwrap();
    draw_rect(&mut ed, (100.0, 100.0), (200.0, 160.0));
    ed.selection = vec![frame];
    // Dragging the frame by 3 units must move it by exactly 3: its child is not a snap target.
    drag(&mut ed, (300.0, 250.0), (303.0, 250.0), NONE);
    assert_eq!(geom(&ed, frame).0, 3.0);
}

/// Window position of the middle of property field `i` in the right panel.
fn field_at(ed: &Editor, i: usize) -> (f64, f64) {
    let c = super::panels::field_rect(ed.layout().right, i).center();
    (c.x, c.y)
}

fn click_field(ed: &mut Editor, i: usize) {
    let (x, y) = field_at(ed, i);
    ed.mouse_moved(x, y);
    ed.click(NONE);
    ed.mouse_up();
}

fn type_and_enter(ed: &mut Editor, text: &str) {
    ed.text(text);
    press_key(ed, Key::Enter, NONE);
}

#[test]
fn typing_a_width_resizes_the_selected_shape_in_one_undo_step() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (200.0, 100.0));
    click_field(&mut ed, 2); // W
    type_and_enter(&mut ed, "250");
    assert_eq!(geom(&ed, id), (0.0, 0.0, 250.0, 100.0));
    assert!(ed.prop_edit.is_none(), "Enter leaves the field");
    press_key(&mut ed, Key::Undo, CTRL);
    assert_eq!(geom(&ed, id).2, 200.0);
}

#[test]
fn position_rotation_and_radius_can_be_typed() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (200.0, 100.0));
    click_field(&mut ed, 0); // X
    type_and_enter(&mut ed, "-40,5");
    click_field(&mut ed, 1); // Y
    type_and_enter(&mut ed, "12");
    click_field(&mut ed, 4); // angle
    type_and_enter(&mut ed, "30");
    click_field(&mut ed, 5); // radius
    type_and_enter(&mut ed, "16");
    let s = ed.session.borrow();
    let n = s.exec.doc().page().node(id).unwrap();
    assert_eq!(
        (n.geom.x, n.geom.y, n.geom.rotation, n.radius),
        (-40.5, 12.0, 30.0, 16.0)
    );
}

#[test]
fn letters_are_ignored_and_an_empty_value_changes_nothing() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (200.0, 100.0));
    click_field(&mut ed, 2);
    ed.text("abc");
    assert_eq!(
        ed.prop_edit.as_ref().unwrap().input.text(),
        "200",
        "letters were not inserted"
    );
    press_key(&mut ed, Key::Backspace, NONE); // the whole value is selected
    press_key(&mut ed, Key::Enter, NONE);
    assert_eq!(geom(&ed, id).2, 200.0, "an empty field is not a number");
}

#[test]
fn escape_cancels_and_clicking_the_canvas_applies() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (200.0, 100.0));
    click_field(&mut ed, 3); // H
    ed.text("500");
    press_key(&mut ed, Key::Escape, NONE);
    assert_eq!(geom(&ed, id).3, 100.0);
    assert!(ed.prop_edit.is_none());

    click_field(&mut ed, 3);
    ed.text("300");
    mv(&mut ed, -250.0, -250.0); // empty canvas: leaving the field applies the value
    ed.click(NONE);
    ed.mouse_up();
    assert_eq!(geom(&ed, id).3, 300.0);
}

#[test]
fn arrow_keys_step_the_value_and_tab_moves_to_the_next_field() {
    let mut ed = editor();
    let id = draw_rect(&mut ed, (0.0, 0.0), (200.0, 100.0));
    click_field(&mut ed, 0); // X
    press_key(&mut ed, Key::Up, NONE);
    press_key(&mut ed, Key::Up, SHIFT);
    assert_eq!(geom(&ed, id).0, 11.0, "+1 then +10");
    press_key(&mut ed, Key::Down, NONE);
    assert_eq!(geom(&ed, id).0, 10.0);
    press_key(&mut ed, Key::Tab, NONE);
    assert_eq!(ed.prop_edit.as_ref().unwrap().field, super::ops::Field::Y);
    press_key(&mut ed, Key::Tab, SHIFT);
    assert_eq!(ed.prop_edit.as_ref().unwrap().field, super::ops::Field::X);
}

#[test]
fn typing_in_a_field_does_not_trigger_tool_shortcuts() {
    let mut ed = editor();
    draw_rect(&mut ed, (0.0, 0.0), (200.0, 100.0));
    click_field(&mut ed, 2);
    press_key(&mut ed, Key::Char('r'), NONE);
    press_key(&mut ed, Key::Delete, NONE);
    assert_eq!(ed.tool, super::state::Tool::Select);
    assert_eq!(
        roots(&ed).len(),
        1,
        "Delete edited the text instead of deleting the shape"
    );
}

#[test]
fn a_group_width_can_be_typed_and_fields_need_a_selection() {
    let mut ed = editor();
    let a = draw_rect(&mut ed, (0.0, 0.0), (100.0, 100.0));
    let b = draw_rect(&mut ed, (200.0, 0.0), (300.0, 100.0));
    press_key(&mut ed, Key::SelectAll, CTRL);
    click_field(&mut ed, 2);
    type_and_enter(&mut ed, "600");
    assert_eq!(geom(&ed, a), (0.0, 0.0, 200.0, 100.0));
    assert_eq!(geom(&ed, b), (400.0, 0.0, 200.0, 100.0));
    ed.selection.clear();
    click_field(&mut ed, 2);
    assert!(ed.prop_edit.is_none(), "no selection, nothing to edit");
}
