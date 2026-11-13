//! Shared HTML escaping for the hand-built shell and figures.

#[cfg(test)]
pub(super) use crate::escape::html_escape_into;
pub(super) use crate::escape::{attr_escape, html_escape};
