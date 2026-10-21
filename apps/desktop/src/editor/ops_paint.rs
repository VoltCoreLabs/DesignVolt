//! Fill and stroke edits from the right panel and the color picker.

use dv_core::{Color, Command, Effect, Executor, NodeId, Paint, Props, Stroke};

/// Fill given to a shape by the "+" button.
pub const DEFAULT_FILL: Color = Color::rgb(217, 217, 217);
/// Stroke given to a shape by the "+" button.
pub const DEFAULT_STROKE: Stroke = Stroke::new(Color::rgb(0, 0, 0), 1.0);

fn run(ex: &mut Executor, label: &'static str, key: Option<u64>, cmds: Vec<Command>) {
    if cmds.is_empty() {
        return;
    }
    let cmd = Command::Batch(cmds);
    let _ = match key {
        Some(k) => ex.execute_merged(label, k, cmd),
        None => ex.execute(label, cmd),
    };
}

/// Replaces the fill at `index` (0 is the main fill, the rest are the extra ones above it) of
/// every shape of the selection by `f(old)`. `None` removes that fill; a shape that has
/// fewer fills gets the new one at the end. Edits sharing a `key` form one undo step.
pub fn edit_fill_at(
    ex: &mut Executor,
    ids: &[NodeId],
    key: Option<u64>,
    index: usize,
    f: impl Fn(Option<Paint>) -> Option<Paint>,
) {
    let page = ex.doc().page();
    let cmds = page
        .geo_tops(ids)
        .into_iter()
        .filter_map(|id| {
            let n = page.node(id)?;
            let mut all: Vec<Paint> = n.fill.iter().chain(&n.extra_fills).cloned().collect();
            match f(all.get(index).cloned()) {
                Some(p) if index < all.len() => all[index] = p,
                Some(p) => all.push(p),
                None if index < all.len() => {
                    all.remove(index);
                }
                None => {}
            }
            let mut rest = all.into_iter();
            let props = Props {
                fill: Some(rest.next()),
                extra_fills: Some(rest.collect()),
                ..Props::default()
            };
            Some(Command::SetProps { id, props })
        })
        .collect();
    run(ex, "Fill", key, cmds);
}

/// Replaces the stroke of every shape of the selection by `f(old)`.
pub fn edit_stroke(
    ex: &mut Executor,
    ids: &[NodeId],
    key: Option<u64>,
    f: impl Fn(Option<Stroke>) -> Option<Stroke>,
) {
    let page = ex.doc().page();
    let cmds = page
        .geo_tops(ids)
        .into_iter()
        .filter_map(|id| {
            let stroke = f(page.node(id)?.stroke);
            let props = Props {
                stroke: Some(stroke),
                ..Props::default()
            };
            Some(Command::SetProps { id, props })
        })
        .collect();
    run(ex, "Stroke", key, cmds);
}

/// Lets `f` change the effect list of every shape of the selection.
pub fn edit_effects(
    ex: &mut Executor,
    ids: &[NodeId],
    key: Option<u64>,
    f: impl Fn(&mut Vec<Effect>),
) {
    let page = ex.doc().page();
    let cmds = page
        .geo_tops(ids)
        .into_iter()
        .filter_map(|id| {
            let mut list = page.node(id)?.effects.clone();
            f(&mut list);
            let props = Props {
                effects: Some(list),
                ..Props::default()
            };
            Some(Command::SetProps { id, props })
        })
        .collect();
    run(ex, "Effects", key, cmds);
}

#[cfg(test)]
mod tests {
    use super::*;
    use dv_core::{Document, Geometry, NodeKind};

    fn shape(ex: &mut Executor) -> NodeId {
        let g = Geometry {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            rotation: 0.0,
        };
        super::super::ops::create(ex, NodeKind::Rectangle, "r".into(), None, g).unwrap()
    }

    #[test]
    fn fill_edits_apply_to_the_selection_and_undo() {
        let mut ex = Executor::new(Document::default());
        let (a, b) = (shape(&mut ex), shape(&mut ex));
        let red = Color::rgb(255, 0, 0);
        edit_fill_at(&mut ex, &[a, b], None, 0, |_| Some(red.into()));
        let page = ex.doc().page();
        assert_eq!(page.node(a).unwrap().fill, Some(red.into()));
        assert_eq!(page.node(b).unwrap().fill, Some(red.into()));
        edit_fill_at(&mut ex, &[a], None, 0, |_| None);
        assert_eq!(ex.doc().page().node(a).unwrap().fill, None);
        ex.undo();
        ex.undo();
        assert_eq!(
            ex.doc().page().node(a).unwrap().fill,
            Some(DEFAULT_FILL.into())
        );
    }

    #[test]
    fn keyed_edits_merge_into_one_step() {
        let mut ex = Executor::new(Document::default());
        let a = shape(&mut ex);
        for v in [10, 20, 30] {
            edit_fill_at(&mut ex, &[a], Some(7), 0, |_| {
                Some(Color::rgb(v, 0, 0).into())
            });
        }
        assert_eq!(
            ex.doc().page().node(a).unwrap().fill,
            Some(Color::rgb(30, 0, 0).into())
        );
        ex.undo();
        assert_eq!(
            ex.doc().page().node(a).unwrap().fill,
            Some(DEFAULT_FILL.into())
        );
    }

    #[test]
    fn effects_can_be_added_changed_and_removed() {
        use dv_core::EffectKind;
        let mut ex = Executor::new(Document::default());
        let a = shape(&mut ex);
        edit_effects(&mut ex, &[a], None, |l| {
            l.push(Effect::new(EffectKind::DropShadow));
            l.push(Effect::new(EffectKind::LayerBlur));
        });
        edit_effects(&mut ex, &[a], None, |l| l[0].blur = 20.0);
        let n = ex.doc().page().node(a).unwrap();
        assert_eq!((n.effects.len(), n.effects[0].blur), (2, 20.0));
        edit_effects(&mut ex, &[a], None, |l| {
            l.remove(0);
        });
        assert_eq!(ex.doc().page().node(a).unwrap().effects.len(), 1);
        ex.undo();
        assert_eq!(ex.doc().page().node(a).unwrap().effects.len(), 2);
    }

    #[test]
    fn stroke_can_be_added_resized_and_removed() {
        let mut ex = Executor::new(Document::default());
        let a = shape(&mut ex);
        edit_stroke(&mut ex, &[a], None, |_| Some(DEFAULT_STROKE));
        edit_stroke(&mut ex, &[a], None, |s| {
            s.map(|s| Stroke { width: 4.0, ..s })
        });
        assert_eq!(ex.doc().page().node(a).unwrap().stroke.unwrap().width, 4.0);
        edit_stroke(&mut ex, &[a], None, |_| None);
        assert!(ex.doc().page().node(a).unwrap().stroke.is_none());
    }

    #[test]
    fn several_fills_stack_and_the_first_is_promoted_when_removed() {
        let mut ex = Executor::new(Document::default());
        let a = shape(&mut ex);
        let (red, green) = (Color::rgb(255, 0, 0), Color::rgb(0, 255, 0));
        edit_fill_at(&mut ex, &[a], None, 1, |old| {
            assert!(old.is_none(), "index 1 is a new fill");
            Some(red.into())
        });
        edit_fill_at(&mut ex, &[a], None, 2, |_| Some(green.into()));
        let n = ex.doc().page().node(a).unwrap();
        assert_eq!(n.fill, Some(DEFAULT_FILL.into()));
        assert_eq!(n.extra_fills, [red.into(), green.into()]);
        edit_fill_at(&mut ex, &[a], None, 0, |_| None);
        let n = ex.doc().page().node(a).unwrap();
        assert_eq!(n.fill, Some(red.into()));
        assert_eq!(n.extra_fills, [green.into()]);
        ex.undo();
        let n = ex.doc().page().node(a).unwrap();
        assert_eq!(
            (n.fill.clone(), n.extra_fills.len()),
            (Some(DEFAULT_FILL.into()), 2)
        );
    }
}
