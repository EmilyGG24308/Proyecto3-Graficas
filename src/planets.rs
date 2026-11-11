//! Catálogo del sistema solar. AQUÍ se agregan planetas y se cambian sus
//! parámetros (radio, órbita, velocidad, semilla, relieve, shader).
//!
//! Para agregar un planeta nuevo:
//!  1. Crear su shader en `src/shaders/<nombre>.rs` (ver `forest.rs`) y
//!     registrarlo en `src/shaders/mod.rs`.
//!  2. Agregar un `Planet::new(PlanetDef { .. }, Box::new(MiShader { .. }))`
//!     a la lista de `solar_system()`.
//! La órbita, la rotación, la colisión, el LOD y el culling ya funcionan
//! solos para cualquier planeta de esta lista.

use crate::config::WORLD_SEED;
use crate::noise::Perlin;
use crate::planet::{Orbit, Planet, PlanetDef, Terrain, TerrainKind};
use crate::shaders::ForestShader;

pub fn solar_system() -> Vec<Planet> {
    vec![forest()]
}

/// Planeta bosque: pradera con lagos, flores y montañas nevadas.
fn forest() -> Planet {
    let terrain = Terrain {
        kind: TerrainKind::Smooth,
        amplitude: 0.14,
        frequency: 1.6,
        octaves: 5,
        sea_level: Some(-0.12),
    };
    let def = PlanetDef {
        name: "Bosque",
        radius: 8.0,
        orbit: Orbit { radius: 60.0, speed: 0.025, phase: 0.6, inclination: 0.0 },
        spin_speed: 0.10,
        axial_tilt: 0.35,
        seed: WORLD_SEED + 1,
        terrain,
    };
    let shader = ForestShader {
        noise: Perlin::new(def.seed + 100),
        amplitude: terrain.amplitude,
        sea_level: terrain.sea_level.unwrap_or(-1.0),
        flower_density: 0.12,
    };
    Planet::new(def, Box::new(shader))
}

// Plantilla para los demás planetas (ciudad, lava, hielo, desierto):
//
// fn lava() -> Planet {
//     let terrain = Terrain { kind: TerrainKind::Ridged, amplitude: 0.12, frequency: 2.0, octaves: 5, sea_level: None };
//     let def = PlanetDef {
//         name: "Lava",
//         radius: 6.0,
//         orbit: Orbit { radius: 35.0, speed: 0.04, phase: 2.5, inclination: 0.05 },
//         spin_speed: 0.2,
//         axial_tilt: 0.1,
//         seed: WORLD_SEED + 2,
//         terrain,
//     };
//     Planet::new(def, Box::new(LavaShader { .. }))
// }
