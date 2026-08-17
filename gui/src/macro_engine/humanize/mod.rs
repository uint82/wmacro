//! humanized mouse movement: path generation, velocity profiles, noise and random number generation.

pub mod constants;
pub mod core;
pub mod math;
pub mod movement;
pub mod noise;
pub mod rng;

pub use core::humanize_commands;
