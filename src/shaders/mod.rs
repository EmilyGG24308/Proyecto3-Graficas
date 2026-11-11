//! Fragment shaders propios. Cada archivo es un efecto distinto.

pub mod forest;
pub mod sun;
pub mod thruster;
pub mod toon;

pub use forest::ForestShader;
pub use sun::SunShader;
pub use thruster::{ThrusterDraw, ThrusterParams, ThrusterShader};
pub use toon::ToonShader;
