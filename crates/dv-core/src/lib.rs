//! DesignVolt document engine: node tree, reversible commands and undo/redo history.
//! No rendering, no UI, no I/O: everything here is plain data and is unit-tested.

#![forbid(unsafe_code)]

pub mod color;
pub mod command;
pub mod document;
pub mod history;
pub mod paint;
pub mod text;

pub use command::{Command, CommandError, Props};
pub use document::{
    Color, Document, Effect, EffectKind, Geometry, Node, NodeId, NodeKind, Page, Stroke,
    StrokeStyle,
};
pub use history::Executor;
pub use paint::{Gradient, GradientKind, Paint, Pattern, Stop, Tiling};
pub use text::{ParseError, from_text, to_text};
