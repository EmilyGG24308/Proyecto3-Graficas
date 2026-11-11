//! Objetos sobre la superficie de los planetas (árboles, lápidas, casas…).
//!
//! 1. Distribución procedural reproducible: se generan direcciones al azar
//!    con un `Rng` sembrado (misma semilla → mismas posiciones) y cada
//!    candidata se acepta o rechaza según:
//!      - un mapa de densidad hecho con ruido fBm (forma bosques, claros,
//!        cementerios…),
//!      - la altura del terreno (no poner palmeras dentro del pantano),
//!      - la pendiente (nada en acantilados).
//! 2. Posición y orientación sobre la superficie curva: cada instancia se
//!    apoya en la altura real del terreno y su eje "arriba" es la dirección
//!    radial (la vertical local del planeta). Se le agrega un giro al azar
//!    sobre esa vertical y una pequeña inclinación.
//! 3. Horneado: todas las instancias de un planeta se transforman UNA vez al
//!    iniciar y se juntan en una sola malla en el espacio local del planeta.
//!    Al dibujarla con la matriz del planeta, todo gira y orbita con él, y
//!    cuesta una sola llamada de dibujo.

use crate::math::{Mat4, Vec3};
use crate::mesh::Mesh;
use crate::noise::{Perlin, Rng};
use crate::planet::Terrain;

/// Cómo se eligen las posiciones de un tipo de objeto.
#[derive(Clone, Copy, Debug)]
pub enum Placement {
    /// `count` instancias repartidas según densidad, altura y pendiente.
    Scatter,
    /// De `count` candidatas válidas, solo la más alta (una casa en la cima).
    Highest,
}

/// Regla de distribución de un tipo de objeto.
pub struct PropSpec {
    pub name: &'static str,
    pub mesh: Mesh,
    pub count: u32,
    pub placement: Placement,
    /// Escala mínima y máxima (1 = tamaño del modelo en unidades del mundo).
    pub scale: (f32, f32),
    /// Rango de alturas permitido, en unidades de la amplitud del terreno.
    pub height: (f32, f32),
    /// Frecuencia del ruido de densidad (bajo = manchas grandes).
    pub density_frequency: f32,
    /// Solo se aceptan puntos donde fbm(dirección) > umbral (-1 = en todos).
    pub density_threshold: f32,
    /// Pendiente máxima (subida / avance). 0.3 ≈ 17°.
    pub max_slope: f32,
    /// Cuánto se entierra la base (unidades del mundo), para que no flote.
    pub sink: f32,
    /// Inclinación máxima al azar (radianes).
    pub max_tilt: f32,
}

/// Resultado del horneado.
pub struct BakedProps {
    pub mesh: Mesh,
    /// (nombre, instancias) para el HUD y la documentación.
    pub counts: Vec<(&'static str, usize)>,
}

/// Datos del planeta que hacen falta para colocar objetos.
pub struct Ground<'a> {
    pub terrain: &'a Terrain,
    pub noise: &'a Perlin,
    pub radius: f32,
    pub seed: u64,
}

impl Ground<'_> {
    /// Altura relativa (fracción del radio).
    fn h(&self, dir: Vec3) -> f32 {
        self.terrain.height(self.noise, dir.normalize())
    }

    /// Altura en unidades de la amplitud (la misma escala que usan los shaders).
    fn h_amp(&self, dir: Vec3) -> f32 {
        if self.terrain.amplitude == 0.0 { 0.0 } else { self.h(dir) / self.terrain.amplitude }
    }

    /// Pendiente del terreno por diferencias finitas.
    fn slope(&self, dir: Vec3) -> f32 {
        let (t1, t2) = tangents(dir);
        let e = 0.004;
        let d1 = (self.h(dir + t1 * e) - self.h(dir - t1 * e)) / (2.0 * e);
        let d2 = (self.h(dir + t2 * e) - self.h(dir - t2 * e)) / (2.0 * e);
        (d1 * d1 + d2 * d2).sqrt()
    }
}

/// Dos tangentes ortonormales a la esfera en `up`.
fn tangents(up: Vec3) -> (Vec3, Vec3) {
    let reference = if up.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let right = reference.cross(up).normalize();
    let fwd = right.cross(up);
    (right, fwd)
}

/// Dirección uniforme sobre la esfera.
fn random_dir(rng: &mut Rng) -> Vec3 {
    let z = rng.range(-1.0, 1.0);
    let a = rng.range(0.0, std::f32::consts::TAU);
    let r = (1.0 - z * z).sqrt();
    Vec3::new(r * a.cos(), z, r * a.sin())
}

/// Calcula las matrices (espacio local del planeta, radio 1) de un tipo de
/// objeto. Es determinista: depende solo de la semilla del planeta, del
/// índice del tipo y de las reglas.
pub fn place(ground: &Ground, spec: &PropSpec, index: usize) -> Vec<Mat4> {
    let mut rng = Rng::new(ground.seed.wrapping_mul(31).wrapping_add(index as u64 * 7919 + 1));
    // Desplazamiento para que cada tipo use una "zona" distinta del ruido.
    let offset = Vec3::new(13.1 * index as f32, -7.7 * index as f32, 5.3 * index as f32 + 3.0);

    // (matriz, altura) de cada posición aceptada.
    let mut accepted: Vec<(Mat4, f32)> = Vec::new();
    let max_attempts = spec.count * 60;
    for _ in 0..max_attempts {
        if accepted.len() as u32 >= spec.count {
            break;
        }
        let dir = random_dir(&mut rng);
        // Siempre se consumen los mismos números al azar por intento.
        let scale_t = rng.next_f32();
        let yaw = rng.range(0.0, std::f32::consts::TAU);
        let tilt = rng.next_f32() * spec.max_tilt;
        let tilt_dir = rng.range(0.0, std::f32::consts::TAU);

        let h = ground.h_amp(dir);
        if h < spec.height.0 || h > spec.height.1 {
            continue;
        }
        if ground.noise.fbm(dir * spec.density_frequency + offset, 3) < spec.density_threshold {
            continue;
        }
        if ground.slope(dir) > spec.max_slope {
            continue;
        }
        let scale = spec.scale.0 + (spec.scale.1 - spec.scale.0) * scale_t;
        accepted.push((instance_matrix(ground, dir, scale, yaw, tilt, tilt_dir, spec.sink), h));
    }

    match spec.placement {
        Placement::Scatter => accepted.into_iter().map(|(m, _)| m).collect(),
        Placement::Highest => accepted
            .into_iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(m, _)| vec![m])
            .unwrap_or_default(),
    }
}

/// Coloca todos los tipos y los junta en una sola malla.
pub fn bake(ground: &Ground, specs: &[PropSpec]) -> BakedProps {
    let mut mesh = Mesh::default();
    let mut counts = Vec::new();
    for (i, spec) in specs.iter().enumerate() {
        let matrices = place(ground, spec, i);
        counts.push((spec.name, matrices.len()));
        for m in &matrices {
            append_transformed(&mut mesh, &spec.mesh, m);
        }
    }
    mesh.compute_bounding_radius();
    BakedProps { mesh, counts }
}

/// Agrega una copia transformada de `src` al final de `dst`.
pub fn append_transformed(dst: &mut Mesh, src: &Mesh, m: &Mat4) {
    let base = dst.positions.len() as u32;
    for i in 0..src.positions.len() {
        dst.positions.push(m.transform_point(src.positions[i]));
        dst.normals.push(m.transform_dir(src.normals[i]).normalize());
        dst.uvs.push(src.uvs[i]);
        dst.colors.push(src.colors[i]);
        dst.emissive.push(src.emissive[i]);
    }
    for t in &src.indices {
        dst.indices.push([t[0] + base, t[1] + base, t[2] + base]);
    }
}

/// Matriz de una instancia: traslación al suelo * base local (vertical =
/// dirección radial) * inclinación * giro * escala.
fn instance_matrix(ground: &Ground, dir: Vec3, scale: f32, yaw: f32, tilt: f32, tilt_dir: f32, sink: f32) -> Mat4 {
    let up = dir.normalize();
    let (right, _) = tangents(up);
    let back = right.cross(up);
    let surface = up * (1.0 + ground.h(up) - sink / ground.radius);
    // Las medidas del modelo están en unidades del mundo; el planeta ya
    // escala por su radio, así que aquí se divide entre el radio.
    let s = scale / ground.radius;
    Mat4::translation(surface)
        * Mat4::from_basis(right, up, back)
        * Mat4::rotation_y(tilt_dir)
        * Mat4::rotation_x(tilt)
        * Mat4::rotation_y(yaw - tilt_dir)
        * Mat4::uniform_scale(s)
}

// ---------------------------------------------------------------------------
// Objetos que flotan y se mueven (fantasmas)
// ---------------------------------------------------------------------------

/// Reglas para objetos animados que vagan sobre el planeta.
pub struct FloaterSpec {
    pub mesh: Mesh,
    pub count: u32,
    /// Altura sobre el radio del planeta (unidades del mundo).
    pub altitude: (f32, f32),
    pub scale: (f32, f32),
    /// Velocidad angular de su recorrido (radianes por segundo).
    pub speed: (f32, f32),
}

/// Un objeto que recorre un círculo máximo del planeta, subiendo y bajando.
#[derive(Clone, Copy, Debug)]
pub struct Floater {
    /// Dirección inicial (unitaria, espacio local del planeta).
    pub start: Vec3,
    /// Eje del círculo máximo que recorre.
    pub axis: Vec3,
    pub speed: f32,
    pub altitude: f32,
    pub scale: f32,
    /// Desfase para que no se muevan todos al mismo tiempo.
    pub phase: f32,
}

impl Floater {
    /// Dirección local en el instante `t` (solo depende del tiempo).
    pub fn dir(&self, t: f32) -> Vec3 {
        self.start.rotate_around(self.axis, t * self.speed)
    }

    /// Altura (con un vaivén suave) en el instante `t`.
    pub fn height(&self, t: f32) -> f32 {
        self.altitude + 0.25 * (t * 1.4 + self.phase).sin()
    }

    /// Visibilidad en [0, 1]: aparecen y desaparecen lentamente.
    pub fn visibility(&self, t: f32) -> f32 {
        let s = (t * 0.35 + self.phase).sin();
        (s * 1.5 + 0.5).clamp(0.0, 1.0)
    }
}

pub struct Floaters {
    pub mesh: Mesh,
    pub items: Vec<Floater>,
}

pub fn spawn_floaters(seed: u64, spec: FloaterSpec) -> Floaters {
    let mut rng = Rng::new(seed ^ 0xF10A7);
    let items = (0..spec.count)
        .map(|_| {
            let start = random_dir(&mut rng);
            let (t1, t2) = tangents(start);
            let a = rng.range(0.0, std::f32::consts::TAU);
            Floater {
                start,
                axis: (t1 * a.cos() + t2 * a.sin()).normalize(),
                speed: rng.range(spec.speed.0, spec.speed.1),
                altitude: rng.range(spec.altitude.0, spec.altitude.1),
                scale: rng.range(spec.scale.0, spec.scale.1),
                phase: rng.range(0.0, 10.0),
            }
        })
        .collect();
    Floaters { mesh: spec.mesh, items }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh;
    use crate::planet::TerrainKind;

    fn spec(count: u32) -> PropSpec {
        PropSpec {
            name: "t",
            mesh: mesh::icosphere(0),
            count,
            placement: Placement::Scatter,
            scale: (1.0, 2.0),
            height: (-1.0, 1.0),
            density_frequency: 2.0,
            density_threshold: 0.0,
            max_slope: 1.0,
            sink: 0.0,
            max_tilt: 0.0,
        }
    }

    const TERRAIN: Terrain =
        Terrain { kind: TerrainKind::Smooth, amplitude: 0.1, frequency: 2.0, octaves: 3, sea_level: None };

    #[test]
    fn same_seed_same_positions_and_on_surface() {
        let noise = Perlin::new(5);
        let g = Ground { terrain: &TERRAIN, noise: &noise, radius: 8.0, seed: 42 };
        let a = place(&g, &spec(50), 0);
        let b = place(&g, &spec(50), 0);
        assert_eq!(a.len(), 50);
        assert_eq!(a, b, "misma semilla, mismas posiciones");
        for m in &a {
            // La base del modelo (origen local) queda sobre el terreno.
            let p = m.transform_point(Vec3::ZERO);
            let expected = 1.0 + TERRAIN.height(&noise, p.normalize());
            assert!((p.length() - expected).abs() < 1e-4);
            // Su eje "arriba" apunta hacia fuera del planeta.
            let up = m.transform_dir(Vec3::Y).normalize();
            assert!(up.dot(p.normalize()) > 0.999);
        }
        let other = place(&Ground { seed: 43, ..g }, &spec(50), 0);
        assert_ne!(a, other, "otra semilla, otras posiciones");
    }

    #[test]
    fn density_noise_rejects_points() {
        let noise = Perlin::new(5);
        let g = Ground { terrain: &TERRAIN, noise: &noise, radius: 8.0, seed: 1 };
        for m in place(&g, &spec(80), 0) {
            let d = m.transform_point(Vec3::ZERO).normalize();
            let offset = Vec3::new(0.0, 0.0, 3.0);
            assert!(noise.fbm(d * 2.0 + offset, 3) >= 0.0);
        }
    }
}
