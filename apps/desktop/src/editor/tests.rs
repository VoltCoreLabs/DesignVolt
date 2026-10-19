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
