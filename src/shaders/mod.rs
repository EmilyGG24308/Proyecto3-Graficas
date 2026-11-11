//! Fragment shaders propios. Cada archivo es un efecto distinto.

pub mod sun;
pub mod test_planet;
pub mod thruster;
pub mod toon;

pub use sun::SunShader;
pub use test_planet::TestPlanetShader;
pub use thruster::{ThrusterDraw, ThrusterParams, ThrusterShader};
pub use toon::ToonShader;
