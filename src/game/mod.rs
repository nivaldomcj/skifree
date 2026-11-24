/// The client size a game starts at (the port's default window), and the
/// one a replay scenario without its own size runs at.
pub const DEFAULT_CLIENT_SIZE: (i16, i16) = (640, 480);

pub mod actor;
pub mod camera;
pub mod cheat;
pub mod collision;
pub mod course;
pub mod draw;
pub mod mob;
pub mod monster;
pub mod physics;
pub mod populator;
pub mod rng;
pub mod scoring;
pub mod skier;
pub mod sound;
pub mod sprites;
pub mod state;
pub mod state_table;
pub mod types;
