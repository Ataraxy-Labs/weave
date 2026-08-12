//! Library half of the weave CLI.
//!
//! The `weave` binary is the primary consumer, but the two *repo-scope*
//! engines here — the cross-file binding pass (`repo_scope`) and the typed
//! patch format (`patch`) — are also exposed as MCP tools, and an MCP server
//! cannot call into a binary. They therefore live in a library target rather
//! than under `src/commands/`, which stays the thin argument-parsing shell.

pub mod gitscan;
pub(crate) mod parsers;
pub mod patch;
pub mod repo_scope;
pub mod wire;
pub mod worktree;
