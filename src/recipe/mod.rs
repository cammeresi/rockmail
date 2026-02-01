//! Recipe evaluation engine for procmail-compatible mail filtering.
//!
//! Evaluates parsed recipes against messages, handling:
//! - Condition evaluation (regex, size, shell, variable)
//! - Weighted scoring
//! - Chain flags (A, a, E, e)
//! - Delivery actions

mod engine;

pub use engine::{Engine, EngineResult, Outcome};
