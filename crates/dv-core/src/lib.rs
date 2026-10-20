//! DesignVolt document engine: node tree, reversible commands and undo/redo history.
//! No rendering, no UI, no I/O: everything here is plain data and is unit-tested.

#![forbid(unsafe_code)]

pub mod color;
pub mod command;
pub mod document;
pub mod history;
pub mod text;

pub use command::{Command, CommandError, Props};
pub use document::{Color, Document, Geometry, Node, NodeId, NodeKind, Page, Stroke};
pub use history::Executor;
pub use text::{ParseError, from_text, to_text};
