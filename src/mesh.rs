//! Meshes indexados: generación procedural (icosfera) y carga de .obj/.mtl.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::math::{Vec2, Vec3};

#[derive(Clone, Default)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    /// Color por vértice (Kd del material, o blanco).
    pub colors: Vec<Vec3>,
    /// 1.0 si el vértice pertenece a un material emisivo (Ke > 0), si no 0.
    pub emissive: Vec<f32>,
    /// Triángulos en sentido antihorario visto desde fuera.
    pub indices: Vec<[u32; 3]>,
    /// Radio de la esfera envolvente centrada en el origen local.
    pub bounding_radius: f32,
}

impl Mesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len()
    }

    pub fn compute_bounding_radius(&mut self) {
        self.bounding_radius = self
            .positions
            .iter()
            .map(|p| p.length())
            .fold(0.0, f32::max);
    }

    /// Normales suaves: promedio de las normales de las caras vecinas
    /// (ponderadas por área, porque el producto cruz no se normaliza).
    pub fn compute_smooth_normals(&mut self) {
        let mut acc = vec![Vec3::ZERO; self.positions.len()];
        for t in &self.indices {
            let (a, b, c) = (
                self.positions[t[0] as usize],
                self.positions[t[1] as usize],
                self.positions[t[2] as usize],
            );
            let n = (b - a).cross(c - a);
            for &i in t {
                acc[i as usize] += n;
            }
        }
        self.normals = acc.into_iter().map(|n| n.normalize()).collect();
    }
}

// ---------------------------------------------------------------------------
// Icosfera: generación + subdivisión procedural antes de rasterizar
// ---------------------------------------------------------------------------

/// Genera una esfera unitaria partiendo de un icosaedro (20 caras) y
/// subdividiendo cada triángulo en 4, `subdivisions` veces. Cada nivel
/// multiplica los triángulos por 4: 20, 80, 320, 1280, 5120, 20480…
pub fn icosphere(subdivisions: u32) -> Mesh {
    let t = (1.0 + 5.0f32.sqrt()) / 2.0;
    let mut positions: Vec<Vec3> = [
        (-1.0, t, 0.0),
        (1.0, t, 0.0),
        (-1.0, -t, 0.0),
        (1.0, -t, 0.0),
        (0.0, -1.0, t),
        (0.0, 1.0, t),
        (0.0, -1.0, -t),
        (0.0, 1.0, -t),
        (t, 0.0, -1.0),
        (t, 0.0, 1.0),
        (-t, 0.0, -1.0),
        (-t, 0.0, 1.0),
    ]
    .iter()
    .map(|&(x, y, z)| Vec3::new(x, y, z).normalize())
    .collect();

    let mut indices: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];

    for _ in 0..subdivisions {
        // Cache de puntos medios: una arista compartida por dos triángulos
        // genera un solo vértice nuevo (así la malla queda sin grietas).
        let mut midpoints: HashMap<(u32, u32), u32> = HashMap::new();
        let mut midpoint = |a: u32, b: u32, pos: &mut Vec<Vec3>| -> u32 {
            let key = if a < b { (a, b) } else { (b, a) };
            *midpoints.entry(key).or_insert_with(|| {
                let m = ((pos[a as usize] + pos[b as usize]) * 0.5).normalize();
                pos.push(m);
                (pos.len() - 1) as u32
            })
        };

        let mut next = Vec::with_capacity(indices.len() * 4);
        for &[a, b, c] in &indices {
            let ab = midpoint(a, b, &mut positions);
            let bc = midpoint(b, c, &mut positions);
            let ca = midpoint(c, a, &mut positions);
            next.push([a, ab, ca]);
            next.push([b, bc, ab]);
            next.push([c, ca, bc]);
            next.push([ab, bc, ca]);
        }
        indices = next;
    }

    let normals = positions.clone();
    let uvs = positions.iter().map(|&p| sphere_uv(p)).collect();
    let colors = vec![Vec3::ONE; positions.len()];
    let emissive = vec![0.0; positions.len()];
    Mesh { positions, normals, uvs, colors, emissive, indices, bounding_radius: 1.0 }
}

/// Cono abierto con la base (radio 1) en z = 0 y la punta en z = 1.
/// Se usa para la flama del propulsor.
pub fn cone(segments: u32, rings: u32) -> Mesh {
    let mut m = Mesh::default();
    for r in 0..=rings {
        let z = r as f32 / rings as f32;
        let radius = 1.0 - z;
        for s in 0..segments {
            let a = std::f32::consts::TAU * s as f32 / segments as f32;
            let (sa, ca) = a.sin_cos();
            m.positions.push(Vec3::new(ca * radius, sa * radius, z));
            // Normal del costado del cono (pendiente 45°).
            m.normals.push(Vec3::new(ca, sa, 1.0).normalize());
            m.uvs.push(Vec2::new(s as f32 / segments as f32, z));
            m.colors.push(Vec3::ONE);
            m.emissive.push(1.0);
        }
    }
    for r in 0..rings {
        for s in 0..segments {
            let s2 = (s + 1) % segments;
            let i = |rr: u32, ss: u32| rr * segments + ss;
            m.indices.push([i(r, s), i(r, s2), i(r + 1, s2)]);
            m.indices.push([i(r, s), i(r + 1, s2), i(r + 1, s)]);
        }
    }
    m.compute_bounding_radius();
    m
}

/// UV esférica (longitud, latitud) de un punto en la esfera unitaria.
pub fn sphere_uv(p: Vec3) -> Vec2 {
    let u = 0.5 + p.z.atan2(p.x) / (2.0 * std::f32::consts::PI);
    let v = 0.5 - p.y.clamp(-1.0, 1.0).asin() / std::f32::consts::PI;
    Vec2::new(u, v)
}

// ---------------------------------------------------------------------------
// Cargador de .obj (con colores difusos del .mtl)
// ---------------------------------------------------------------------------

/// Carga un .obj triangulando polígonos en abanico. Si el archivo referencia
/// un .mtl, se usa el color difuso (Kd) de cada material como color de vértice.
pub fn load_obj(path: impl AsRef<Path>) -> Result<Mesh, String> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;

    let mut vs: Vec<Vec3> = Vec::new();
    let mut vts: Vec<Vec2> = Vec::new();
    let mut vns: Vec<Vec3> = Vec::new();
    let mut materials: HashMap<String, Material> = HashMap::new();
    let mut current = Material::default();

    let mut mesh = Mesh::default();
    let mut has_normals = true;
    // Vértices únicos por combinación (v, vt, vn, material).
    let mut dedup: HashMap<(i64, i64, i64, String), u32> = HashMap::new();
    let mut current_mtl = String::new();

    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("v") => vs.push(parse_vec3(&mut it)),
            Some("vt") => {
                let u = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                let v = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                vts.push(Vec2::new(u, v));
            }
            Some("vn") => vns.push(parse_vec3(&mut it)),
            Some("mtllib") => {
                if let Some(name) = it.next() {
                    let mtl_path = path.with_file_name(name);
                    materials.extend(load_mtl(&mtl_path));
                }
            }
            Some("usemtl") => {
                current_mtl = it.next().unwrap_or("").to_string();
                current = materials.get(&current_mtl).copied().unwrap_or_default();
            }
            Some("f") => {
                let mut face: Vec<u32> = Vec::new();
                for tok in it {
                    let mut parts = tok.split('/');
                    let vi = resolve(parts.next(), vs.len());
                    let ti = resolve(parts.next(), vts.len());
                    let ni = resolve(parts.next(), vns.len());
                    let Some(vi) = vi else { continue };
                    if ni.is_none() {
                        has_normals = false;
                    }
                    let key = (
                        vi as i64,
                        ti.map_or(-1, |x| x as i64),
                        ni.map_or(-1, |x| x as i64),
                        current_mtl.clone(),
                    );
                    let idx = *dedup.entry(key).or_insert_with(|| {
                        mesh.positions.push(vs[vi]);
                        mesh.uvs.push(ti.map_or(Vec2::default(), |t| vts[t]));
                        mesh.normals.push(ni.map_or(Vec3::Y, |n| vns[n]));
                        mesh.colors.push(current.kd);
                        mesh.emissive.push(if current.ke.max_elem() > 0.0 { 1.0 } else { 0.0 });
                        (mesh.positions.len() - 1) as u32
                    });
                    face.push(idx);
                }
                for k in 1..face.len().saturating_sub(1) {
                    mesh.indices.push([face[0], face[k], face[k + 1]]);
                }
            }
            _ => {}
        }
    }

    if mesh.indices.is_empty() {
        return Err(format!("{}: el modelo no tiene caras", path.display()));
    }
    if !has_normals {
        mesh.compute_smooth_normals();
    }
    mesh.compute_bounding_radius();
    Ok(mesh)
}

fn parse_vec3<'a>(it: &mut impl Iterator<Item = &'a str>) -> Vec3 {
    let mut f = || it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    Vec3::new(f(), f(), f())
}

/// Convierte un índice .obj (1-based o negativo/relativo) a 0-based.
fn resolve(tok: Option<&str>, len: usize) -> Option<usize> {
    let i: i64 = tok.filter(|s| !s.is_empty())?.parse().ok()?;
    let idx = if i < 0 { len as i64 + i } else { i - 1 };
    (idx >= 0 && (idx as usize) < len).then_some(idx as usize)
}

/// Lo que nos interesa de un material del .mtl.
#[derive(Clone, Copy)]
struct Material {
    /// Color difuso.
    kd: Vec3,
    /// Color de emisión (si es > 0, el material brilla solo).
    ke: Vec3,
}

impl Default for Material {
    fn default() -> Self {
        Self { kd: Vec3::ONE, ke: Vec3::ZERO }
    }
}

fn load_mtl(path: &Path) -> HashMap<String, Material> {
    let mut out: HashMap<String, Material> = HashMap::new();
    let Ok(text) = fs::read_to_string(path) else { return out };
    let mut name = String::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("newmtl") => {
                name = it.next().unwrap_or("").to_string();
                out.insert(name.clone(), Material::default());
            }
            Some("Kd") => {
                let v = parse_vec3(&mut it);
                out.entry(name.clone()).or_default().kd = v;
            }
            Some("Ke") => {
                let v = parse_vec3(&mut it);
                out.entry(name.clone()).or_default().ke = v;
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icosphere_triangle_counts() {
        for (s, n) in [(0, 20), (1, 80), (2, 320), (3, 1280)] {
            assert_eq!(icosphere(s).triangle_count(), n);
        }
    }

    #[test]
    fn icosphere_vertices_shared() {
        // V = 10 * 4^s + 2 para una icosfera sin vértices duplicados.
        assert_eq!(icosphere(2).positions.len(), 162);
    }

    #[test]
    fn icosphere_faces_point_outward() {
        let m = icosphere(1);
        for t in &m.indices {
            let (a, b, c) = (
                m.positions[t[0] as usize],
                m.positions[t[1] as usize],
                m.positions[t[2] as usize],
            );
            let n = (b - a).cross(c - a);
            assert!(n.dot(a + b + c) > 0.0);
        }
    }
}
