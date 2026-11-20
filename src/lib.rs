//! CR-QISL experimental simulation platform.
//!
//! Research position: a quantum-inspired *classical* distributed-state simulation.
//! It is not a physical quantum implementation and not an established consensus
//! protocol. No safety / Byzantine-tolerance / finality guarantee is claimed.
//! Every equation that is not part of an explicit specification is tagged
//! `ASSUMPTION[Axx]` in the code and listed in `docs/ASSUMPTIONS.md`.

pub mod analysis;
pub mod config;
pub mod experiment;
pub mod metrics;
pub mod model;
pub mod network;
pub mod node;
pub mod rng;
pub mod sim;
pub mod stats;
pub mod topology;
