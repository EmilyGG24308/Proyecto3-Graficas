//! Planetas: órbita, rotación, terreno deformado con ruido y niveles de detalle.
//!
//! Cada planeta se genera UNA vez al iniciar:
//!  1. Se crea una icosfera para cada nivel de detalle (LOD).
//!  2. Cada vértice se desplaza a lo largo de su dirección según la función
//!     de altura del terreno (deformación por ruido, antes de rasterizar).
//!  3. Se recalculan las normales suaves de la malla deformada.
//!
//! La MISMA función de altura se usa para la colisión, así que la nave choca
//! con las montañas que se ven.

use crate::config::LOD_SUBDIVISIONS;
use crate::math::{Mat4, Vec3};
use crate::mesh::{self, Mesh};
use crate::noise::Perlin;
use crate::shader::Shader;

#[derive(Clone, Copy, Debug)]
pub enum TerrainKind {
    /// fBm: colinas suaves.
    Smooth,
    /// Ruido ridged: crestas afiladas (montañas, grietas).
    Ridged,
}

/// Parámetros de la superficie de un planeta.
#[derive(Clone, Copy, Debug)]
pub struct Terrain {
    pub kind: TerrainKind,
    /// Altura máxima relativa al radio (0.08 = montañas del 8 % del radio).
    pub amplitude: f32,
    /// Frecuencia del ruido (más alto = relieve más fino).
    pub frequency: f32,
    pub octaves: u32,
    /// Nivel del mar en unidades de `amplitude` (por ejemplo -0.1). Lo que
    /// queda por debajo se aplana: océanos o lagos. None = sin mar.
    pub sea_level: Option<f32>,
}

impl Terrain {
    pub const FLAT: Terrain =
        Terrain { kind: TerrainKind::Smooth, amplitude: 0.0, frequency: 1.0, octaves: 1, sea_level: None };

    /// Altura relativa (fracción del radio) en la dirección unitaria `dir`
    /// (espacio local del planeta).
    pub fn height(&self, noise: &Perlin, dir: Vec3) -> f32 {
        if self.amplitude == 0.0 {
            return 0.0;
        }
        let p = dir * self.frequency;
        let n = match self.kind {
            TerrainKind::Smooth => noise.fbm(p, self.octaves),
            TerrainKind::Ridged => noise.ridged(p, self.octaves) * 2.0 - 1.0,
        };
        let n = match self.sea_level {
            Some(sea) => n.max(sea),
            None => n,
        };
        n * self.amplitude
    }
}

/// Órbita circular alrededor del sol (que está en el origen).
#[derive(Clone, Copy, Debug)]
pub struct Orbit {
    pub radius: f32,
    /// Radianes por segundo.
    pub speed: f32,
    /// Ángulo inicial (radianes), para que no salgan todos alineados.
    pub phase: f32,
    /// Inclinación del plano de la órbita (radianes).
    pub inclination: f32,
}

/// Todo lo que define a un planeta. Para la demo se cambian estos valores
/// en `planets.rs` (radio, velocidad orbital, semilla…).
#[derive(Clone, Copy, Debug)]
pub struct PlanetDef {
    pub name: &'static str,
    pub radius: f32,
    pub orbit: Orbit,
    /// Rotación sobre su eje (radianes por segundo).
    pub spin_speed: f32,
    /// Inclinación del eje de rotación (radianes).
    pub axial_tilt: f32,
    /// Semilla del ruido del terreno y de la distribución de objetos.
    pub seed: u64,
    pub terrain: Terrain,
}

pub struct Planet {
    pub def: PlanetDef,
    pub noise: Perlin,
    /// Una malla por nivel de detalle (índice 0 = la más simple).
    pub lods: Vec<Mesh>,
    pub shader: Box<dyn Shader + Send>,
    /// Radio de la esfera envolvente en mundo (incluye las montañas).
    pub bounding_radius: f32,
}

impl Planet {
    pub fn new(def: PlanetDef, shader: Box<dyn Shader + Send>) -> Self {
        let noise = Perlin::new(def.seed);
        let lods: Vec<Mesh> = LOD_SUBDIVISIONS
            .iter()
            .map(|&s| build_terrain_mesh(&def.terrain, &noise, s))
            .collect();
        let bounding_radius = lods.iter().map(|m| m.bounding_radius).fold(1.0, f32::max) * def.radius;
        Self { def, noise, lods, shader, bounding_radius }
    }

    /// Centro del planeta en el instante `t` (segundos). Depende solo del
    /// tiempo transcurrido, nunca de cuántos cuadros se han dibujado.
    pub fn position(&self, t: f32) -> Vec3 {
        let o = &self.def.orbit;
        let a = o.phase + t * o.speed;
        let flat = Vec3::new(a.cos(), 0.0, -a.sin()) * o.radius;
        Mat4::rotation_x(o.inclination).transform_dir(flat)
    }

    /// Velocidad del centro (derivada de `position`).
    pub fn velocity(&self, t: f32) -> Vec3 {
        let o = &self.def.orbit;
        let a = o.phase + t * o.speed;
        let flat = Vec3::new(-a.sin(), 0.0, -a.cos()) * (o.radius * o.speed);
        Mat4::rotation_x(o.inclination).transform_dir(flat)
    }

    /// Orientación: inclinación del eje * giro sobre el eje.
    pub fn rotation(&self, t: f32) -> Mat4 {
        Mat4::rotation_z(self.def.axial_tilt) * Mat4::rotation_y(t * self.def.spin_speed)
    }

    /// Matriz de modelo: traslación (órbita) * rotación * escala (radio).
    pub fn model(&self, t: f32) -> Mat4 {
        Mat4::translation(self.position(t)) * self.rotation(t) * Mat4::uniform_scale(self.def.radius)
    }

    /// Radio de la superficie (en mundo) en una dirección local unitaria.
    pub fn surface_radius_local(&self, dir_local: Vec3) -> f32 {
        self.def.radius * (1.0 + self.def.terrain.height(&self.noise, dir_local))
    }

    /// Punto de la superficie (en espacio local del modelo, radio ~1) en la
    /// dirección `dir_local`. Útil para colocar árboles, edificios, etc.
    pub fn surface_point_local(&self, dir_local: Vec3) -> Vec3 {
        let d = dir_local.normalize();
        d * (1.0 + self.def.terrain.height(&self.noise, d))
    }
}

/// Icosfera subdividida y deformada con la función de altura.
pub fn build_terrain_mesh(terrain: &Terrain, noise: &Perlin, subdivisions: u32) -> Mesh {
    let mut m = mesh::icosphere(subdivisions);
    if terrain.amplitude != 0.0 {
        for p in m.positions.iter_mut() {
            let dir = *p; // en la icosfera unitaria la posición ya es la dirección
            *p = dir * (1.0 + terrain.height(noise, dir));
        }
        m.compute_smooth_normals();
    }
    m.compute_bounding_radius();
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shaders::ToonShader;

    fn test_planet() -> Planet {
        let def = PlanetDef {
            name: "t",
            radius: 5.0,
            orbit: Orbit { radius: 40.0, speed: 0.1, phase: 0.3, inclination: 0.2 },
            spin_speed: 0.5,
            axial_tilt: 0.3,
            seed: 7,
            terrain: Terrain {
                kind: TerrainKind::Smooth,
                amplitude: 0.1,
                frequency: 2.0,
                octaves: 4,
                sea_level: Some(-0.1),
            },
        };
        Planet::new(def, Box::new(ToonShader { ambient: 0.3, outline: 0.0 }))
    }

    #[test]
    fn same_seed_same_terrain() {
        let (a, b) = (test_planet(), test_planet());
        assert_eq!(a.lods[1].positions, b.lods[1].positions);
    }

    #[test]
    fn velocity_is_derivative_of_position() {
        let p = test_planet();
        let t = 12.0;
        let h = 1e-2;
        let numeric = (p.position(t + h) - p.position(t - h)) / (2.0 * h);
        assert!((numeric - p.velocity(t)).length() < 1e-2);
    }

    #[test]
    fn mesh_vertices_lie_on_surface_function() {
        let p = test_planet();
        let m = p.lods.last().unwrap();
        for v in m.positions.iter().step_by(97) {
            let r = p.surface_radius_local(v.normalize()) / p.def.radius;
            assert!((v.length() - r).abs() < 1e-4);
        }
    }
}
