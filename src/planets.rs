//! Catálogo del sistema solar. AQUÍ se agregan planetas y se cambian sus
//! parámetros (radio, órbita, velocidad, semilla, relieve, shader).
//!
//! Para agregar un planeta nuevo:
//!  1. Crear su shader en `src/shaders/<nombre>.rs` (ver `jungle.rs`) y
//!     registrarlo en `src/shaders/mod.rs`.
//!  2. Agregar un `Planet::new(PlanetDef { .. }, Box::new(MiShader { .. }))`
//!     a la lista de `solar_system()`.
//! La órbita, la rotación, la colisión, el LOD y el culling ya funcionan
//! solos para cualquier planeta de esta lista.

use crate::config::WORLD_SEED;
use crate::mesh::{self, Mesh};
use crate::noise::Perlin;
use crate::planet::{Orbit, Planet, PlanetDef, Terrain, TerrainKind};
use crate::props::{FloaterSpec, Placement, PropSpec};
use crate::scene::asset_path;
use crate::shaders::{HauntedShader, JungleShader};

pub fn solar_system() -> Result<Vec<Planet>, String> {
    let mut planets = vec![jungle()?, haunted()?];
    // Cada integrante agrega sus mundos en su propio archivo (src/mundos/).
    planets.extend(crate::mundos::esteban::planetas()?);
    planets.extend(crate::mundos::emily::planetas()?);
    Ok(planets)
}

/// Carga `assets/models/<name>.obj` (con su `.mtl`).
pub fn model(name: &str) -> Result<Mesh, String> {
    mesh::load_obj(asset_path(&format!("assets/models/{name}.obj")))
}

/// Jungla jurásica: selva densa, ríos, pantanos, acantilados y volcanes.
fn jungle() -> Result<Planet, String> {
    let terrain = Terrain {
        kind: TerrainKind::Smooth,
        amplitude: 0.16,
        frequency: 1.8,
        octaves: 5,
        sea_level: Some(-0.14),
    };
    let def = PlanetDef {
        name: "Jungla",
        radius: 8.0,
        orbit: Orbit { radius: 60.0, speed: 0.025, phase: 0.6, inclination: 0.0 },
        spin_speed: 0.10,
        axial_tilt: 0.35,
        seed: WORLD_SEED + 1,
        terrain,
    };
    let volcano_height = 0.3;
    let shader = JungleShader {
        noise: Perlin::new(def.seed + 100),
        amplitude: terrain.amplitude,
        sea_level: terrain.sea_level.unwrap_or(-1.0),
        canopy_scale: 65.0,
        volcano_height,
    };
    let props = vec![
        PropSpec {
            name: "Palmera",
            mesh: model("palmera")?,
            count: 90,
            placement: Placement::Scatter,
            scale: (0.8, 1.3),
            height: (-0.09, volcano_height - 0.03),
            density_frequency: 2.5,
            density_threshold: -0.05,
            max_slope: 0.35,
            sink: 0.05,
            max_tilt: 0.15,
        },
        PropSpec {
            name: "Helecho",
            mesh: model("helecho")?,
            count: 110,
            placement: Placement::Scatter,
            scale: (0.6, 1.1),
            height: (-0.1, volcano_height),
            density_frequency: 4.0,
            density_threshold: -0.15,
            max_slope: 0.45,
            sink: 0.02,
            max_tilt: 0.2,
        },
        PropSpec {
            name: "Esqueleto",
            mesh: model("dino")?,
            count: 4,
            placement: Placement::Scatter,
            scale: (0.9, 1.2),
            height: (-0.05, 0.2),
            density_frequency: 1.0,
            density_threshold: -1.0,
            max_slope: 0.15,
            sink: 0.12,
            max_tilt: 0.08,
        },
    ];
    Ok(Planet::new(def, Box::new(shader)).with_props(props))
}

/// Planeta embrujado: tierra muerta, senderos, farolitos y niebla fantasmal.
fn haunted() -> Result<Planet, String> {
    let terrain = Terrain {
        kind: TerrainKind::Ridged,
        amplitude: 0.07,
        frequency: 2.4,
        octaves: 4,
        sea_level: None,
    };
    let def = PlanetDef {
        name: "Embrujado",
        radius: 6.5,
        orbit: Orbit { radius: 100.0, speed: 0.015, phase: 3.4, inclination: 0.12 },
        spin_speed: 0.06,
        axial_tilt: 0.2,
        seed: WORLD_SEED + 13,
        terrain,
    };
    let shader = HauntedShader {
        noise: Perlin::new(def.seed + 100),
        amplitude: terrain.amplitude,
        fog_density: 0.9,
        fog_speed: 0.06,
        lantern_density: 0.08,
    };
    let props = vec![
        PropSpec {
            name: "Arbol seco",
            mesh: model("arbol_seco")?,
            count: 70,
            placement: Placement::Scatter,
            scale: (0.8, 1.5),
            height: (-1.0, 0.75),
            density_frequency: 2.0,
            density_threshold: -0.1,
            max_slope: 0.5,
            sink: 0.05,
            max_tilt: 0.25,
        },
        PropSpec {
            // Umbral alto: las lápidas se agrupan en "cementerios".
            name: "Lapida",
            mesh: model("lapida")?,
            count: 140,
            placement: Placement::Scatter,
            scale: (0.8, 1.2),
            height: (-1.0, 0.5),
            density_frequency: 3.0,
            density_threshold: 0.12,
            max_slope: 0.4,
            sink: 0.04,
            max_tilt: 0.25,
        },
        PropSpec {
            // Una sola casona, en el punto más alto de 60 candidatos.
            name: "Casona",
            mesh: model("casona")?,
            count: 60,
            placement: Placement::Highest,
            scale: (0.7, 0.7),
            height: (-1.0, 2.0),
            density_frequency: 1.0,
            density_threshold: -1.0,
            max_slope: 0.8,
            sink: 0.15,
            max_tilt: 0.0,
        },
    ];
    let ghosts = FloaterSpec {
        mesh: model("fantasma")?,
        count: 7,
        altitude: (0.8, 2.0),
        scale: (0.5, 0.8),
        speed: (0.04, 0.1),
    };
    Ok(Planet::new(def, Box::new(shader)).with_props(props).with_floaters(ghosts))
}

// Plantilla para los demás planetas (ciudad, lava, hielo, desierto):
//
// fn lava() -> Result<Planet, String> {
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
//     Ok(Planet::new(def, Box::new(LavaShader { .. })).with_props(vec![ /* PropSpec… */ ]))
// }
