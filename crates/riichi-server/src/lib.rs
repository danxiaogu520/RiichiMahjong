pub mod ai_client;
pub mod protocol;

// Temporary compatibility re-exports for existing server adapters. New code
// should depend on `riichi-session` directly.
pub use riichi_session::{channel, game};
