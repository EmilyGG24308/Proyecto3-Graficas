use std::f32::consts::TAU;

use crate::config::WORLD_SEED;
use crate::math::{Mat4, Vec3, smoothstep};
use crate::noise::{Perlin, Rng};
use crate::planet::{Orbit, Planet, PlanetDef, Terrain, TerrainKind};
use crate::planets::model;
use crate::props::{Placement, PropSpec, append_transformed};
use crate::shader::{Shader, Uniforms, Varyings, lambert, rim, toon_diffuse};

pub fn planetas() -> Result<Vec<Planet>, String> {
    Ok(vec![lava()?, desierto()?, mecanico()?])
}

// Ruido celular con hash entero: evita calcular senos en las 27 celdas
// vecinas por cada pixel, manteniendo placas reproducibles con la semilla.
fn placas(p: Vec3, semilla: u64) -> (f32, f32) {
    let base = Vec3::new(p.x.floor(), p.y.floor(), p.z.floor());
    let mut primera = f32::MAX;
    let mut segunda = f32::MAX;
    let mut id = 0.0;
    for x in -1i32..=1 {
        for y in -1i32..=1 {
            for z in -1i32..=1 {
                let celda = base + Vec3::new(x as f32, y as f32, z as f32);
                let mut hash = (celda.x as i32 as u32).wrapping_mul(0x8da6b343)
                    ^ (celda.y as i32 as u32).wrapping_mul(0xd8163841)
                    ^ (celda.z as i32 as u32).wrapping_mul(0xcb1ab31f)
                    ^ semilla as u32;
                hash ^= hash >> 16;
                hash = hash.wrapping_mul(0x7feb352d);
                hash ^= hash >> 15;
                hash = hash.wrapping_mul(0x846ca68b);
                hash ^= hash >> 16;
                let desplazamiento = Vec3::new(
                    (hash & 1023) as f32 / 1024.0,
                    ((hash >> 10) & 1023) as f32 / 1024.0,
                    ((hash >> 20) & 1023) as f32 / 1024.0,
                );
                let distancia = (celda + desplazamiento - p).length_sq();
                if distancia < primera {
                    segunda = primera;
                    primera = distancia;
                    id = (hash & 65535) as f32 / 65535.0;
                } else if distancia < segunda {
                    segunda = distancia;
                }
            }
        }
    }
    (segunda.sqrt() - primera.sqrt(), id)
}

struct LavaShader {
    amplitud: f32,
    semilla: u64,
}

impl Shader for LavaShader {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let dir = v.local_pos.normalize();
        let altura = (v.local_pos.length() - 1.0) / self.amplitud;
        let normal = v.normal.normalize();
        let luz = lambert(normal, v.world_pos, u.sun_pos);
        // La distancia entre las dos celdas más cercanas dibuja las grietas.
        let (distancia, id) = placas(dir * 5.0, self.semilla);
        let grieta = 1.0 - smoothstep(0.025, 0.10, distancia);
        let roca = Vec3::new(0.12, 0.10, 0.14).lerp(
            Vec3::new(0.30, 0.23, 0.24),
            (id * 0.55 + smoothstep(-0.2, 0.8, altura) * 0.45).clamp(0.0, 1.0),
        );
        let pulso = 0.8 + 0.2 * (u.time * 1.8 + id * 6.0).sin();
        let brasa = Vec3::new(1.0, 0.25, 0.025).lerp(Vec3::new(1.0, 0.75, 0.15), grieta);
        let borde = smoothstep(0.6, 1.0, rim(normal, v.world_pos, u.camera_pos));
        roca * toon_diffuse(luz, 0.20) + brasa * (grieta * pulso) + Vec3::new(0.65, 0.12, 0.03) * (borde * 0.25)
    }
}

struct DesiertoShader {
    ruido: Perlin,
    amplitud: f32,
}

impl Shader for DesiertoShader {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let dir = v.local_pos.normalize();
        let altura = (v.local_pos.length() - 1.0) / self.amplitud;
        let normal = v.normal.normalize();
        let luz = lambert(normal, v.world_pos, u.sun_pos);
        // Las bandas onduladas siguen la esfera y el ruido rompe su regularidad.
        let ondulacion = self.ruido.fbm(dir * 3.5, 3);
        let duna = (dir.x * 23.0 + dir.z * 10.0 + ondulacion * 5.0).sin();
        let arena = Vec3::new(0.72, 0.40, 0.18).lerp(
            Vec3::new(1.0, 0.76, 0.38),
            smoothstep(-0.45, 0.65, duna),
        );
        let estrato = smoothstep(0.12, 0.5, altura);
        let color = arena.lerp(Vec3::new(0.68, 0.28, 0.16), estrato * 0.5);
        let borde = smoothstep(0.6, 1.0, rim(normal, v.world_pos, u.camera_pos));
        color * toon_diffuse(luz, 0.22) + Vec3::new(0.85, 0.47, 0.18) * (borde * 0.22)
    }
}

struct MecanicoShader {
    amplitud: f32,
}

impl Shader for MecanicoShader {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let dir = v.local_pos.normalize();
        let altura = (v.local_pos.length() - 1.0) / self.amplitud;
        let normal = v.normal.normalize();
        let luz = lambert(normal, v.world_pos, u.sun_pos);
        // Tres familias de bandas envuelven la esfera sin costura en los polos.
        let bandas = (dir.x * 14.0).sin().abs()
            .min((dir.y * 14.0).sin().abs())
            .min((dir.z * 14.0).sin().abs());
        let union = 1.0 - smoothstep(0.06, 0.19, bandas);
        let panel = Vec3::new(0.24, 0.34, 0.40).lerp(
            Vec3::new(0.44, 0.53, 0.56), smoothstep(-0.4, 0.5, altura),
        );
        let metal = panel.lerp(Vec3::new(0.08, 0.16, 0.20), union * 0.8);
        let pulso = 0.65 + 0.35 * (u.time * 1.5 + dir.y * 6.0).sin();
        let borde = smoothstep(0.6, 1.0, rim(normal, v.world_pos, u.camera_pos));
        metal * toon_diffuse(luz, 0.22)
            + Vec3::new(0.05, 0.80, 1.0) * (union * pulso * 0.42 + borde * 0.20)
    }
}

fn mecanico() -> Result<Planet, String> {
    let terreno = Terrain { kind: TerrainKind::Smooth, amplitude: 0.035, frequency: 2.0, octaves: 3, sea_level: None };
    let def = PlanetDef {
        name: "Mecanico", radius: 6.5,
        orbit: Orbit { radius: 145.0, speed: 0.012, phase: 0.9, inclination: 0.03 },
        spin_speed: 0.10, axial_tilt: 0.18, seed: WORLD_SEED + 22, terrain: terreno,
    };
    let objetos = vec![
        PropSpec { name: "Engranajes", mesh: model("mecanico_engranaje")?, count: 45,
            placement: Placement::Scatter, scale: (0.7, 1.2), height: (-1.0, 1.0),
            density_frequency: 2.3, density_threshold: -0.10, max_slope: 0.35, sink: 0.05, max_tilt: 0.04 },
        PropSpec { name: "Antenas", mesh: model("mecanico_antena")?, count: 35,
            placement: Placement::Scatter, scale: (0.75, 1.3), height: (-1.0, 1.0),
            density_frequency: 2.5, density_threshold: -0.08, max_slope: 0.35, sink: 0.05, max_tilt: 0.04 },
    ];
    Ok(Planet::new(def, Box::new(MecanicoShader { amplitud: terreno.amplitude })).with_props(objetos))
}

fn lava() -> Result<Planet, String> {
    let terreno = Terrain { kind: TerrainKind::Ridged, amplitude: 0.12, frequency: 2.0, octaves: 5, sea_level: None };
    let def = PlanetDef {
        name: "Lava", radius: 6.0,
        orbit: Orbit { radius: 30.0, speed: 0.04, phase: 2.5, inclination: 0.05 },
        spin_speed: 0.2, axial_tilt: 0.1, seed: WORLD_SEED + 20, terrain: terreno,
    };
    let objetos = vec![
        PropSpec { name: "Columnas de basalto", mesh: model("lava_columnas")?, count: 55,
            placement: Placement::Scatter, scale: (0.7, 1.3), height: (-1.0, 1.0),
            density_frequency: 2.5, density_threshold: -0.08, max_slope: 0.55, sink: 0.08, max_tilt: 0.12 },
        PropSpec { name: "Obsidiana", mesh: model("lava_obsidiana")?, count: 45,
            placement: Placement::Scatter, scale: (0.7, 1.25), height: (-0.6, 1.0),
            density_frequency: 3.0, density_threshold: -0.08, max_slope: 0.55, sink: 0.09, max_tilt: 0.15 },
    ];
    Ok(Planet::new(def, Box::new(LavaShader { amplitud: terreno.amplitude, semilla: def.seed })).with_props(objetos))
}

fn desierto() -> Result<Planet, String> {
    let terreno = Terrain { kind: TerrainKind::Smooth, amplitude: 0.06, frequency: 3.0, octaves: 4, sea_level: None };
    let def = PlanetDef {
        name: "Desierto", radius: 7.0,
        orbit: Orbit { radius: 80.0, speed: 0.019, phase: 1.8, inclination: 0.08 },
        spin_speed: 0.08, axial_tilt: 0.22, seed: WORLD_SEED + 21, terrain: terreno,
    };
    let objetos = vec![
        PropSpec { name: "Cactus", mesh: model("desierto_cactus")?, count: 75,
            placement: Placement::Scatter, scale: (0.65, 1.15), height: (-1.0, 0.7),
            density_frequency: 2.5, density_threshold: -0.1, max_slope: 0.35, sink: 0.04, max_tilt: 0.08 },
        PropSpec { name: "Arcos", mesh: model("desierto_arco")?, count: 16,
            placement: Placement::Scatter, scale: (0.8, 1.2), height: (-1.0, 0.8),
            density_frequency: 1.8, density_threshold: -0.2, max_slope: 0.3, sink: 0.12, max_tilt: 0.05 },
    ];
    let shader = DesiertoShader { ruido: Perlin::new(def.seed + 100), amplitud: terreno.amplitude };
    let mut planeta = Planet::new(def, Box::new(shader)).with_props(objetos);
    let roca = model("desierto_asteroide")?;
    let mut rng = Rng::new(def.seed + 7);
    if let Some(objetos) = planeta.props.as_mut() {
        // El anillo se hornea en espacio local, por lo que acompaña el giro.
        for _ in 0..120 {
            let angulo = rng.range(0.0, TAU);
            let radio = rng.range(1.6, 2.2);
            let posicion = Vec3::new(angulo.cos() * radio, rng.range(-0.03, 0.03), angulo.sin() * radio);
            let matriz = Mat4::translation(posicion) * Mat4::rotation_y(angulo) * Mat4::uniform_scale(rng.range(0.04, 0.10));
            append_transformed(&mut objetos.mesh, &roca, &matriz);
        }
        objetos.mesh.compute_bounding_radius();
        planeta.bounding_radius = planeta.bounding_radius.max(objetos.mesh.bounding_radius * def.radius);
    }
    Ok(planeta)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn placas_son_reproducibles_y_dependen_de_la_semilla() {
        let punto = Vec3::new(1.2, -3.4, 5.6);
        let a = placas(punto, 42);
        assert_eq!(a, placas(punto, 42));
        assert_ne!(a, placas(punto, 43));
        assert!(a.0 >= 0.0);
    }

    #[test]
    fn mundos_y_anillo_son_reproducibles_y_quedan_en_el_limite() {
        let a = planetas().unwrap();
        let b = planetas().unwrap();
        assert_eq!(a.len(), 3);
        for (primero, segundo) in a.iter().zip(&b) {
            let objetos = primero.props.as_ref().unwrap();
            assert_eq!(objetos.mesh.positions, segundo.props.as_ref().unwrap().mesh.positions);
            assert!(objetos.mesh.triangle_count() <= 60_000);
            assert!(primero.bounding_radius >= objetos.mesh.bounding_radius * primero.def.radius);
        }
        assert!(a[1].bounding_radius > a[1].def.radius * 2.0);
    }
}
