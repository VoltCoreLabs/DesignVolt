//! Text file format of a document. Plain text, tab separated, so it is easy to inspect and to
//! migrate; nodes are written parents first and siblings in z-order, which is also the order
//! they are read back in.
//!
//! ```text
//! designvolt-document<TAB>1
//! name<TAB><document name>
//! page<TAB><page name>
//! node<TAB>id<TAB>parent(0=none)<TAB>kind<TAB>x<TAB>y<TAB>w<TAB>h<TAB>rotation<TAB>radius<TAB>
//!      flags(1=visible,2=locked)<TAB>fill<TAB>stroke<TAB>name
//! ```
//!
//! `fill` is `-` or `rrggbbaa`; `stroke` is `-` or `rrggbbaa:width`. Names escape `\`, tab and
//! newline. Unknown line types are ignored so newer files stay readable.

use std::fmt;

use crate::paint::Paint;

use crate::document::{
    Color, Document, Effect, EffectKind, Geometry, LineCap, LineJoin, Node, NodeId, NodeKind, Page,
    Stroke, StrokePos, StrokeStyle, UserGuide,
};

const MAGIC: &str = "designvolt-document";
const VERSION: u32 = 1;

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

fn kind_code(k: NodeKind) -> &'static str {
    match k {
        NodeKind::Frame => "frame",
        NodeKind::Rectangle => "rect",
        NodeKind::Ellipse => "ellipse",
        NodeKind::Group => "group",
    }
}

fn kind_from(s: &str) -> Option<NodeKind> {
    match s {
        "frame" => Some(NodeKind::Frame),
        "rect" => Some(NodeKind::Rectangle),
        "ellipse" => Some(NodeKind::Ellipse),
        "group" => Some(NodeKind::Group),
        _ => None,
    }
}

fn color_code(c: Color) -> String {
    format!("{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
}

fn color_from(s: &str) -> Option<Color> {
    if s.len() != 8 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    Some(Color {
        r: byte(0)?,
        g: byte(2)?,
        b: byte(4)?,
        a: byte(6)?,
    })
}

/// Serializes the whole document.
pub fn to_text(doc: &Document) -> String {
    let mut out = format!("{MAGIC}\t{VERSION}\nname\t{}\n", escape(&doc.name));
    for page in doc.pages() {
        out.push_str(&format!("page\t{}\n", escape(&page.name)));
        for g in page.guides() {
            out.push_str(&format!(
                "guide\t{}\t{}\n",
                if g.vertical { 'v' } else { 'h' },
                g.pos
            ));
        }
        for id in page.draw_order() {
            let Some(n) = page.node(id) else { continue };
            let g = n.geom;
            let flags = u8::from(n.visible) | (u8::from(n.locked) << 1);
            let fill = n.fill.as_ref().map_or("-".to_string(), Paint::to_code);
            let stroke = n.stroke.map_or("-".to_string(), |s| {
                format!(
                    "{}:{}:{}:{}:{}:{}:{}:{}:{}",
                    color_code(s.color),
                    s.width,
                    s.style.code(),
                    s.dash,
                    s.gap,
                    s.join.code(),
                    s.miter_angle,
                    s.cap.code(),
                    s.position.code()
                )
            });
            let effects = if n.effects.is_empty() {
                "-".to_string()
            } else {
                n.effects
                    .iter()
                    .map(|e| {
                        format!(
                            "{}:{}:{}:{}:{}",
                            e.kind.code(),
                            color_code(e.color),
                            e.x,
                            e.y,
                            e.blur
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("|")
            };
            let extras = if n.extra_fills.is_empty() {
                "-".to_string()
            } else {
                n.extra_fills
                    .iter()
                    .map(Paint::to_code)
                    .collect::<Vec<_>>()
                    .join("~")
            };
            out.push_str(&format!(
                "node\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                id.0,
                n.parent.map_or(0, |p| p.0),
                kind_code(n.kind),
                g.x,
                g.y,
                g.w,
                g.h,
                g.rotation,
                n.radius,
                flags,
                fill,
                stroke,
                escape(&n.name),
                n.opacity,
                effects,
                extras
            ));
        }
    }
    out
}

fn err(line: usize, message: impl Into<String>) -> ParseError {
    ParseError {
        line,
        message: message.into(),
    }
}

/// Reads a document written by `to_text`.
pub fn from_text(text: &str) -> Result<Document, ParseError> {
    let mut lines = text.lines().enumerate();
    let Some((_, header)) = lines.next() else {
        return Err(err(1, "empty file"));
    };
    let mut head = header.split('\t');
    if head.next() != Some(MAGIC) {
        return Err(err(1, "not a DesignVolt document"));
    }
    match head.next().and_then(|v| v.parse::<u32>().ok()) {
        Some(v) if v <= VERSION => {}
        Some(v) => return Err(err(1, format!("unsupported version {v}"))),
        None => return Err(err(1, "missing version")),
    }

    let mut name = String::new();
    let mut pages: Vec<Page> = Vec::new();
    let mut max_id = 0u64;
    for (i, line) in lines {
        let n = i + 1;
        let mut f = line.split('\t');
        match f.next() {
            Some("name") => name = unescape(f.next().unwrap_or("")),
            Some("page") => pages.push(Page::new(unescape(f.next().unwrap_or("")))),
            Some("guide") => {
                let page = pages
                    .last_mut()
                    .ok_or_else(|| err(n, "guide before any page"))?;
                let vertical = f.next() == Some("v");
                let pos = f.next().and_then(|v| v.parse().ok());
                page.guides
                    .extend(pos.map(|pos| UserGuide { vertical, pos }));
            }
            Some("node") => {
                let page = pages
                    .last_mut()
                    .ok_or_else(|| err(n, "node before any page"))?;
                let (id, node) = parse_node(&mut f, page, n)?;
                max_id = max_id.max(id.0);
                attach(page, id, node, n)?;
            }
            _ => {}
        }
    }
    Ok(Document::from_parts(name, pages, max_id + 1))
}

fn attach(page: &mut Page, id: NodeId, node: Node, line: usize) -> Result<(), ParseError> {
    if page.nodes.contains_key(&id) {
        return Err(err(line, format!("duplicate node id {}", id.0)));
    }
    match node.parent {
        None => page.roots.push(id),
        Some(p) => page
            .nodes
            .get_mut(&p)
            .ok_or_else(|| err(line, format!("parent {} comes after its child", p.0)))?
            .children
            .push(id),
    }
    page.nodes.insert(id, node);
    Ok(())
}

/// One effect: `kind:color:x:y:blur`; older files had a lone drop shadow as `color:x:y:blur`.
fn parse_effect(s: &str) -> Option<Effect> {
    let mut p: Vec<&str> = s.split(':').collect();
    let kind = match EffectKind::from_code(p[0]) {
        Some(k) => {
            p.remove(0);
            k
        }
        None => EffectKind::DropShadow,
    };
    let mut it = p.into_iter();
    let color = color_from(it.next()?)?;
    let mut num = || it.next()?.parse::<f64>().ok().filter(|v| v.is_finite());
    Some(Effect {
        kind,
        color,
        x: num()?,
        y: num()?,
        blur: num()?.max(0.0),
    })
}

fn parse_effects(s: &str) -> Vec<Effect> {
    if s == "-" {
        return Vec::new();
    }
    s.split('|').filter_map(parse_effect).collect()
}

fn parse_node<'a>(
    f: &mut impl Iterator<Item = &'a str>,
    page: &Page,
    line: usize,
) -> Result<(NodeId, Node), ParseError> {
    let mut next = |what: &str| f.next().ok_or_else(|| err(line, format!("missing {what}")));
    let num = |s: &str, what: &str| {
        s.parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| err(line, format!("bad {what}")))
    };
    let id = next("id")?
        .parse::<u64>()
        .map_err(|_| err(line, "bad id"))?;
    let parent = next("parent")?
        .parse::<u64>()
        .map_err(|_| err(line, "bad parent"))?;
    let kind = kind_from(next("kind")?).ok_or_else(|| err(line, "unknown node kind"))?;
    let geom = Geometry {
        x: num(next("x")?, "x")?,
        y: num(next("y")?, "y")?,
        w: num(next("w")?, "w")?,
        h: num(next("h")?, "h")?,
        rotation: num(next("rotation")?, "rotation")?,
    };
    let radius = num(next("radius")?, "radius")?;
    let flags = next("flags")?
        .parse::<u8>()
        .map_err(|_| err(line, "bad flags"))?;
    let fill = match next("fill")? {
        "-" => None,
        s => Some(Paint::from_code(s).ok_or_else(|| err(line, "bad fill"))?),
    };
    let stroke = match next("stroke")? {
        "-" => None,
        s => {
            let mut p = s.split(':');
            let (c, w) = (
                p.next().unwrap_or(""),
                p.next().ok_or_else(|| err(line, "bad stroke"))?,
            );
            let mut st = Stroke::new(
                color_from(c).ok_or_else(|| err(line, "bad stroke color"))?,
                num(w, "stroke width")?,
            );
            // Later columns are optional: older files stop after the style.
            let rest: Vec<&str> = p.collect();
            let at = |i: usize| rest.get(i).copied().unwrap_or("");
            st.style = StrokeStyle::from_code(at(0)).unwrap_or_default();
            let n = |i: usize, d: f64| {
                at(i)
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .unwrap_or(d)
            };
            st.dash = n(1, st.dash);
            st.gap = n(2, st.gap);
            st.join = LineJoin::from_code(at(3)).unwrap_or_default();
            st.miter_angle = n(4, st.miter_angle);
            st.cap = LineCap::from_code(at(5)).unwrap_or_default();
            st.position = StrokePos::from_code(at(6)).unwrap_or_default();
            Some(st)
        }
    };
    let name = unescape(f.next().unwrap_or(""));
    let _ = page;
    // Columns added later come after the name; older files simply end there.
    let opacity = f
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .map_or(1.0, |v| v.clamp(0.0, 1.0));
    let effects = f.next().map(parse_effects).unwrap_or_default();
    let extra_fills = f
        .next()
        .map(|s| s.split('~').filter_map(Paint::from_code).collect())
        .unwrap_or_default();
    let mut node = Node::new(kind, name, geom);
    node.opacity = opacity;
    node.effects = effects;
    node.extra_fills = extra_fills;
    node.fill = fill;
    node.stroke = stroke;
    node.radius = radius;
    node.visible = flags & 1 != 0;
    node.locked = flags & 2 != 0;
    node.parent = (parent != 0).then_some(NodeId(parent));
    Ok((NodeId(id), node))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, Props};

    fn add(doc: &mut Document, kind: NodeKind, name: &str, parent: Option<NodeId>) -> NodeId {
        let id = doc.alloc_id();
        let index = doc.page().siblings(parent).len();
        let geom = Geometry {
            x: 1.5,
            y: -2.25,
            w: 100.0,
            h: 40.5,
            rotation: 12.0,
        };
        doc.apply(&Command::Create {
            id,
            parent,
            index,
            node: Node::new(kind, name, geom),
        })
        .unwrap();
        id
    }

    fn same(a: &Document, b: &Document) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.page().draw_order(), b.page().draw_order());
        for id in a.page().draw_order() {
            assert_eq!(a.page().node(id), b.page().node(id), "node {id:?}");
        }
    }

    #[test]
    fn roundtrip_keeps_tree_order_and_styles() {
        let mut doc = Document::new("Мой документ\tс табом");
        let f = add(&mut doc, NodeKind::Frame, "Фрейм \"1\"", None);
        let a = add(&mut doc, NodeKind::Rectangle, "a", Some(f));
        let b = add(&mut doc, NodeKind::Ellipse, "b\\n", Some(f));
        let top = add(&mut doc, NodeKind::Rectangle, "top", None);
        let props = Props {
            fill: Some(None),
            stroke: Some(Some(Stroke {
                color: Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 4,
                },
                width: 2.5,
                style: StrokeStyle::Dashed,
                dash: 7.5,
                gap: 3.0,
                join: LineJoin::Round,
                miter_angle: 40.0,
                cap: LineCap::Square,
                position: StrokePos::Outside,
            })),
            opacity: Some(0.4),
            effects: Some(vec![
                Effect {
                    color: Color::rgb(9, 8, 7),
                    x: -2.5,
                    y: 6.0,
                    blur: 12.0,
                    ..Effect::new(EffectKind::InnerShadow)
                },
                Effect::new(EffectKind::LayerBlur),
            ]),
            radius: Some(8.25),
            locked: Some(true),
            visible: Some(false),
            ..Props::default()
        };
        doc.apply(&Command::SetProps { id: a, props }).unwrap();
        doc.apply(&Command::Reorder { id: a, index: 1 }).unwrap();
        let back = from_text(&to_text(&doc)).unwrap();
        same(&doc, &back);
        assert_eq!(back.page().siblings(Some(f)), [b, a], "z-order is kept");
        assert_eq!(back.page().roots(), [f, top]);
        let n = back.page().node(a).unwrap();
        assert!(n.locked && !n.visible && n.fill.is_none());
        assert_eq!(n.stroke.unwrap().width, 2.5);
        let st = n.stroke.unwrap();
        assert_eq!(st.style, StrokeStyle::Dashed);
        assert_eq!((st.dash, st.gap, st.miter_angle), (7.5, 3.0, 40.0));
        assert_eq!(
            (st.join, st.cap, st.position),
            (LineJoin::Round, LineCap::Square, StrokePos::Outside)
        );
        assert_eq!(
            (n.opacity, n.effects.len(), n.effects[0].blur),
            (0.4, 2, 12.0)
        );
        assert_eq!(n.effects[0].kind, EffectKind::InnerShadow);
    }

    #[test]
    fn a_lone_shadow_of_the_previous_format_loads_as_a_drop_shadow() {
        let text = format!(
            "{MAGIC}	{VERSION}
name	x
page	p
node	1	0	rect	0	0	10	10	0	0	3	ff0000ff	-	n	0.5	000000ff:1:2:3
"
        );
        let doc = from_text(&text).unwrap();
        let n = doc.page().node(NodeId(1)).unwrap();
        assert_eq!(n.effects.len(), 1);
        assert_eq!(
            (n.effects[0].kind, n.effects[0].y, n.effects[0].blur),
            (EffectKind::DropShadow, 2.0, 3.0)
        );
    }

    #[test]
    fn old_files_without_effects_still_load() {
        let text = format!(
            "{MAGIC}	{VERSION}
name	x
page	p
node	1	0	rect	0	0	10	10	0	0	3	ff0000ff	00ff00ff:2	old
"
        );
        let doc = from_text(&text).unwrap();
        let n = doc.page().node(NodeId(1)).unwrap();
        assert_eq!((n.opacity, n.effects.len()), (1.0, 0));
        assert_eq!(n.stroke.unwrap().style, StrokeStyle::Solid);
        assert_eq!(n.name, "old");
    }

    #[test]
    fn ids_continue_after_the_highest_loaded_id() {
        let mut doc = Document::default();
        add(&mut doc, NodeKind::Rectangle, "a", None);
        let b = add(&mut doc, NodeKind::Rectangle, "b", None);
        let mut back = from_text(&to_text(&doc)).unwrap();
        assert!(back.alloc_id().0 > b.0);
    }

    #[test]
    fn user_guides_roundtrip() {
        let mut doc = Document::default();
        let guides = vec![
            UserGuide {
                vertical: true,
                pos: 12.5,
            },
            UserGuide {
                vertical: false,
                pos: -40.0,
            },
        ];
        doc.apply(&Command::SetGuides(guides.clone())).unwrap();
        let back = from_text(&to_text(&doc)).unwrap();
        assert_eq!(back.page().guides(), &guides[..]);
    }

    #[test]
    fn empty_documents_roundtrip() {
        let doc = Document::new("Empty");
        let back = from_text(&to_text(&doc)).unwrap();
        assert_eq!(back.name, "Empty");
        assert!(back.page().is_empty());
    }

    #[test]
    fn floats_survive_exactly() {
        let mut doc = Document::default();
        let id = doc.alloc_id();
        let geom = Geometry {
            x: 0.1 + 0.2,
            y: 1e-7,
            w: 123456789.123456,
            h: 1e15,
            rotation: -0.0,
        };
        doc.apply(&Command::Create {
            id,
            parent: None,
            index: 0,
            node: Node::new(NodeKind::Rectangle, "f", geom),
        })
        .unwrap();
        let back = from_text(&to_text(&doc)).unwrap();
        assert_eq!(back.page().node(id).unwrap().geom, geom);
    }

    #[test]
    fn rejects_foreign_or_future_files() {
        assert_eq!(from_text("").unwrap_err().line, 1);
        assert!(from_text("hello\n").is_err());
        assert!(
            from_text("designvolt-document\t99\n")
                .unwrap_err()
                .message
                .contains("version")
        );
        assert!(from_text("designvolt-document\tx\n").is_err());
    }

    #[test]
    fn reports_the_line_of_a_broken_node() {
        let text = "designvolt-document\t1\nname\tx\npage\tp\nnode\t1\t0\trect\t0\t0\tNaN\t1\t0\t0\t1\t-\t-\tn\n";
        let e = from_text(text).unwrap_err();
        assert_eq!(e.line, 4);
        assert!(e.message.contains("bad w"), "{e}");
        let orphan =
            "designvolt-document\t1\npage\tp\nnode\t2\t1\trect\t0\t0\t1\t1\t0\t0\t1\t-\t-\tn\n";
        assert!(from_text(orphan).unwrap_err().message.contains("parent"));
        let dup = "designvolt-document\t1\npage\tp\nnode\t1\t0\trect\t0\t0\t1\t1\t0\t0\t1\t-\t-\ta\nnode\t1\t0\trect\t0\t0\t1\t1\t0\t0\t1\t-\t-\tb\n";
        assert!(from_text(dup).unwrap_err().message.contains("duplicate"));
        assert!(
            from_text("designvolt-document\t1\nnode\t1\t0\trect\t0\t0\t1\t1\t0\t0\t1\t-\t-\ta\n")
                .is_err()
        );
    }

    #[test]
    fn unknown_lines_are_ignored_for_forward_compatibility() {
        let text = "designvolt-document\t1\nname\tDoc\nfuture\tsomething\npage\tP\nnode\t1\t0\trect\t0\t0\t5\t5\t0\t0\t1\tff0000ff\t-\tn\n";
        let doc = from_text(text).unwrap();
        assert_eq!(doc.page().len(), 1);
    }

    #[test]
    fn a_big_document_roundtrips() {
        let mut doc = Document::default();
        let mut cmds = Vec::new();
        for i in 0..10_000u64 {
            let id = doc.alloc_id();
            let geom = Geometry {
                x: i as f64,
                y: (i * 7 % 1000) as f64,
                w: 20.0,
                h: 20.0,
                rotation: 0.0,
            };
            cmds.push(Command::Create {
                id,
                parent: None,
                index: i as usize,
                node: Node::new(NodeKind::Rectangle, format!("Shape {i}"), geom),
            });
        }
        doc.apply(&Command::Batch(cmds)).unwrap();
        let text = to_text(&doc);
        let back = from_text(&text).unwrap();
        assert_eq!(back.page().len(), 10_000);
        same(&doc, &back);
    }

    #[test]
    fn extra_fills_round_trip() {
        let mut doc = Document::default();
        let a = doc.alloc_id();
        let node = Node::new(NodeKind::Rectangle, "a", Geometry::default());
        doc.apply(&Command::Create {
            id: a,
            parent: None,
            index: 0,
            node,
        })
        .unwrap();
        let grad = Paint::Gradient(crate::paint::Gradient::from_color(
            crate::paint::GradientKind::Radial,
            Color::rgb(1, 2, 3),
        ));
        let props = Props {
            fill: Some(Some(Color::rgb(9, 9, 9).into())),
            extra_fills: Some(vec![grad.clone(), Color::rgb(7, 8, 9).into()]),
            ..Props::default()
        };
        doc.apply(&Command::SetProps { id: a, props }).unwrap();
        let back = from_text(&to_text(&doc)).unwrap();
        let n = back.page().node(a).unwrap();
        assert_eq!(n.extra_fills, [grad, Color::rgb(7, 8, 9).into()]);
    }
}
