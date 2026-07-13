//! Pluggable player-agent boundary.
//!
//! The current debug AI still uses the channel adapter directly. This trait
//! reserves a stable seam for future heuristic, search-based, or external
//! Mortal-like agents without allowing an agent to mutate `GameState`.

use crate::channel::{PlayerAction, ServerEvent};
use riichi_core::player::PlayerId;
use std::future::Future;
use std::pin::Pin;

pub type AgentFuture<'a> = Pin<Box<dyn Future<Output = PlayerAction> + Send + 'a>>;

pub trait PlayerAgent: Send {
    fn player_id(&self) -> PlayerId;

    fn decide<'a>(&'a mut self, observation: ServerEvent) -> AgentFuture<'a>;
}
