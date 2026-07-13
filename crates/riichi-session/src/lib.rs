//! A transport-independent game session.
//!
//! `riichi-session` owns the asynchronous game loop and the in-process
//! command/event boundary. Network transports, terminal clients, and bot
//! agents can all adapt to this boundary without owning `GameState`.

pub mod agent;
pub mod channel;
pub mod game;

pub use agent::{AgentFuture, PlayerAgent};
pub use channel::{
    create_player_pair, CallResponse, ClientHandle, PlayerAction, PlayerCommand, PlayerHandle,
    SessionEvent, TurnAction,
};
pub use game::GameSession;
