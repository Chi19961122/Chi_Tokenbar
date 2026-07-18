//! Provider adapters. Each reads one source and yields `Limit`s.
//! Codex = local file (safest). Anthropic = undocumented API with degrade.

pub mod anthropic;
pub mod codex;
pub mod codex_live;
