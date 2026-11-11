//! Planeta Ciudad: shader de calles y luces, y colocación de edificios.

use std::f32::consts::TAU;

use crate::config::WORLD_SEED;
use crate::math::{Vec3, smoothstep};
use crate::mesh::Mesh;
use crate::noise::{Perlin, Rng, hash3};
use crate::planet::{Orbit, Planet, PlanetDef, Terrain, TerrainKind};
use crate::planets::model;
use crate::props::{Ground, Placement, PropSpec, place};
use crate::shader::{Shader, Uniforms, Varyings, blinn_phong, lambert, rim, toon_diffuse};

use super::comun::{hash_celda, hornear, noche, sobre_suelo};
use super::cuadricula::{Manzana, cuadricula, direccion_cuadricula, tipo_manzana};

/// Shader de la ciudad.
///
/// Técnica: cuadrícula de calles por proyección de cubo + capas por manzana.
///  - Calles: donde la parte fraccionaria de la cuadrícula está cerca de 0 o
///    de 1 (`min(f, 1 - f) < ancho`). Junto a cada calle, una acera clara.
///  - Manzanas: su hash elige parque (pasto con franjas de podadora),
///    plaza (con fuente animada) o lotes con techos de colores (2×2 lotes).
///  - Tráfico: puntos que avanzan por las calles con `u.time`, en sentidos
///    opuestos a cada lado de la línea; de noche son faros y calaveras.
///  - Noche (`noche()` ≈ lambert < 0.1): faroles en los cruces y ventanas
///    que se encienden y apagan por turnos (cada ventana cambia de estado
///    cada pocos segundos según un hash de su id y del tiempo).
///  - Lagos y bahías: lo aplanado por el nivel del mar es agua con brillo.
///  - Atmósfera de neón rosa en el borde.
pub struct CiudadShader {
    /// Debe coincidir con `Terrain::amplitude`.
    pub amplitud: f32,
    /// Debe coincidir con `Terrain::sea_level` (unidades de amplitud).
    pub nivel_mar: f32,
    /// Manzanas por lado en cada cara del cubo.
    pub manzanas: f32,
    /// Medio ancho de la calle, en fracción de manzana.
    pub ancho_calle: f32,
    /// Proporción de manzanas que son parque.
    pub parques: f32,
    /// Proporción de ventanas encendidas de noche.
    pub luces: f32,
    /// Velocidad de los coches (manzanas por segundo).
    pub trafico: f32,
    pub semilla: u32,
}

impl Shader for CiudadShader {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let n = v.normal.normalize();
        let p = v.local_pos;
        let dir = p.normalize();
        let h = (p.length() - 1.0) / self.amplitud;
        let t = u.time;

        let d = lambert(n, v.world_pos, u.sun_pos);
        let luz = toon_diffuse(d, 0.18);
        let noche = noche(v, u);
        let borde = smoothstep(0.6, 1.0, rim(n, v.world_pos, u.camera_pos));
        let atmosfera = Vec3::new(1.0, 0.42, 0.72) * (borde * (0.22 + 0.18 * noche));

        // --- Lagos y bahías ---------------------------------------------------
        if h <= self.nivel_mar + 0.03 {
            let agua = Vec3::new(0.10, 0.32, 0.55);
            let brillo = smoothstep(0.5, 0.56, blinn_phong(n, v.world_pos, u.sun_pos, u.camera_pos, 60.0));
            // De noche, reflejos de las luces de la orilla que tiemblan.
            let reflejo = smoothstep(0.92, 1.0, ((dir.x - dir.z) * 160.0 + (dir.y * 90.0 + t * 2.0).sin()).sin());
            return agua * luz
                + Vec3::splat(brillo * 0.55)
                + Vec3::new(1.0, 0.75, 0.35) * (reflejo * noche * 0.35)
                + atmosfera;
        }

        let (cara, gu, gv) = cuadricula(dir, self.manzanas);
        let (i, j) = (gu.floor(), gv.floor());
        let (fu, fv) = (gu - i, gv - j);
        // Distancia (en fracción de manzana) a la línea de la cuadrícula más cercana.
        let du = fu.min(1.0 - fu);
        let dv = fv.min(1.0 - fv);
        let w = self.ancho_calle;

        let mut emision = Vec3::ZERO;
        let base = if du < w || dv < w {
            // --- Calles ------------------------------------------------------
            let mut c = Vec3::new(0.24, 0.24, 0.29);
            let cruce = du < w && dv < w;
            if cruce {
                // Farol en cada cruce: halo naranja que solo se nota de noche.
                let r = (du * du + dv * dv).sqrt();
                emision += Vec3::new(1.0, 0.62, 0.25) * ((1.0 - smoothstep(0.0, w * 1.6, r)) * noche * 0.9);
            } else {
                // Coches: la calle corre a lo largo de v (si du < w) o de u.
                let (a_lo_largo, lado, carril, linea) =
                    if du < w { (gv, fu < 0.5, du, gu.round()) } else { (gu, fv < 0.5, dv, gv.round()) };
                let sentido = if lado { 1.0 } else { -1.0 };
                let fase = hash3(Vec3::new(linea, cara as f32, if du < w { 1.0 } else { 2.0 }));
                let x = (a_lo_largo * 2.0 + sentido * t * self.trafico + fase * 7.0).rem_euclid(1.0);
                let en_carril = carril > w * 0.2 && carril < w * 0.8;
                if en_carril && x < 0.16 {
                    if noche > 0.5 {
                        let faro = if lado { Vec3::new(1.0, 0.95, 0.7) } else { Vec3::new(1.0, 0.15, 0.1) };
                        emision += faro * noche;
                    } else {
                        let colores = [Vec3::new(0.9, 0.2, 0.2), Vec3::new(0.95, 0.8, 0.2), Vec3::new(0.25, 0.5, 0.95)];
                        c = colores[((fase * 3.0) as usize).min(2)];
                    }
                }
            }
            c
        } else if du < w + 0.05 || dv < w + 0.05 {
            // --- Acera -------------------------------------------------------
            Vec3::new(0.72, 0.70, 0.68)
        } else {
            let hash = hash_celda(cara, i as i32, j as i32, self.semilla);
            match tipo_manzana(hash, self.parques) {
                Manzana::Parque => {
                    // Franjas de pasto como recién podado (bandas de 4 en la manzana).
                    let franja = (fu * 4.0).floor() as i32 % 2 == 0;
                    if franja { Vec3::new(0.36, 0.70, 0.30) } else { Vec3::new(0.28, 0.60, 0.26) }
                }
                Manzana::Plaza => {
                    // Fuente al centro de la plaza: ondas que salen con el tiempo.
                    let r = ((fu - 0.5) * (fu - 0.5) + (fv - 0.5) * (fv - 0.5)).sqrt();
                    if r < 0.12 {
                        let onda = 0.5 + 0.5 * (r * 70.0 - t * 4.0).sin();
                        Vec3::new(0.25, 0.55, 0.90).lerp(Vec3::new(0.75, 0.90, 1.0), onda * 0.6)
                    } else if r < 0.16 {
                        Vec3::new(0.55, 0.55, 0.60)
                    } else {
                        Vec3::new(0.90, 0.80, 0.62)
                    }
                }
                Manzana::Edificios => {
                    // Lotes 2×2: cada uno con su techo (de lejos los edificios no
                    // se dibujan, y esto hace que la ciudad se siga leyendo).
                    let (lu, lv) = ((fu * 2.0).floor(), (fv * 2.0).floor());
                    let lote = hash_celda(cara, i as i32 * 2 + lu as i32, j as i32 * 2 + lv as i32, self.semilla ^ 0x51);
                    let techos = [
                        Vec3::new(0.80, 0.42, 0.30),
                        Vec3::new(0.55, 0.55, 0.62),
                        Vec3::new(0.92, 0.82, 0.60),
                        Vec3::new(0.40, 0.48, 0.70),
                    ];
                    let (xu, xv) = ((fu * 2.0).fract(), (fv * 2.0).fract());
                    let callejon = xu.min(1.0 - xu).min(xv.min(1.0 - xv)) < 0.07;
                    let techo = if callejon { Vec3::new(0.35, 0.33, 0.36) } else { techos[(lote * 4.0) as usize % 4] };
                    // Ventanas nocturnas: 4×4 por lote. Cada una se prende o se
                    // apaga cada ~6 s (el "turno" depende del tiempo y de su id).
                    if noche > 0.05 && !callejon {
                        let (wu, wv) = ((gu * 8.0).floor(), (gv * 8.0).floor());
                        let id = hash3(Vec3::new(wu, wv, cara as f32));
                        let turno = (t * 0.16 + id * 9.0).floor();
                        let prendida = hash3(Vec3::new(wu + turno * 0.37, wv, turno)) < self.luces;
                        let (cu, cv) = ((gu * 8.0).fract(), (gv * 8.0).fract());
                        if prendida && cu > 0.2 && cu < 0.8 && cv > 0.25 && cv < 0.75 {
                            emision += Vec3::new(1.0, 0.82, 0.42) * noche;
                        }
                    }
                    techo
                }
            }
        };
        base * luz + emision + atmosfera
    }
}

/// Parámetros de la ciudad que comparten el shader y la colocación de
/// edificios (deben ser los mismos para que coincidan).
const CIUDAD_MANZANAS: f32 = 7.0;
const CIUDAD_PARQUES: f32 = 0.16;

pub(super) fn ciudad() -> Result<Planet, String> {
    // Casi plano: la ciudad necesita suelo parejo. Las partes que quedan
    // bajo el nivel del mar se vuelven lagos y bahías.
    let terreno = Terrain { kind: TerrainKind::Smooth, amplitude: 0.03, frequency: 1.6, octaves: 3, sea_level: Some(-0.22) };
    let def = PlanetDef {
        name: "Ciudad",
        radius: 5.5,
        orbit: Orbit { radius: 45.0, speed: 0.032, phase: 4.4, inclination: 0.06 },
        spin_speed: 0.12,
        axial_tilt: 0.15,
        seed: WORLD_SEED + 30,
        terrain: terreno,
    };
    let semilla = def.seed as u32;
    let shader = CiudadShader {
        amplitud: terreno.amplitude,
        nivel_mar: terreno.sea_level.unwrap_or(-1.0),
        manzanas: CIUDAD_MANZANAS,
        ancho_calle: 0.09,
        parques: CIUDAD_PARQUES,
        luces: 0.45,
        trafico: 0.35,
        semilla,
    };
    let mut planeta = Planet::new(def, Box::new(shader));

    let edificio = model("ciudad_edificio")?;
    let rascacielos = model("ciudad_rascacielos")?;
    let antena = model("ciudad_antena")?;
    let arbol = model("ciudad_arbol")?;

    // 1) Torre de radio: monumento único en la loma más alta (regla del motor
    //    `Placement::Highest`, igual que la casona del embrujado).
    let ground = Ground { terrain: &def.terrain, noise: &planeta.noise, radius: def.radius, seed: def.seed };
    let torre = PropSpec {
        name: "Torre de radio",
        mesh: Mesh::default(),
        count: 80,
        placement: Placement::Highest,
        scale: (1.5, 1.5),
        height: (-0.1, 2.0),
        density_frequency: 1.0,
        density_threshold: -1.0,
        max_slope: 0.5,
        sink: 0.05,
        max_tilt: 0.0,
    };
    let torres = place(&ground, &torre, 0);
    let celda_torre = torres.first().map(|m| {
        let (c, gu, gv) = cuadricula(m.transform_point(Vec3::ZERO).normalize(), CIUDAD_MANZANAS);
        (c, gu.floor(), gv.floor())
    });

    // 2) Edificios en la cuadrícula: uno por manzana de tipo "Edificios",
    //    centrado y alineado con las calles. En el "centro" (donde un ruido
    //    de baja frecuencia es alto) se vuelven rascacielos; afuera son
    //    edificios bajos. 3) Árboles: dos por parque, en esquinas opuestas.
    let mut rng = Rng::new(def.seed ^ 0xC17);
    let n = CIUDAD_MANZANAS;
    let centro_ciudad = Perlin::new(def.seed + 200);
    let mut bajos = Vec::new();
    let mut altos = Vec::new();
    let mut arboles = Vec::new();
    // Tamaño de una manzana en unidades del mundo (un cuarto de vuelta / n).
    let manzana = TAU / 4.0 * def.radius / n;
    for cara in 0..6 {
        for i in 0..n as i32 {
            for j in 0..n as i32 {
                // Siempre se consumen los mismos números al azar por manzana
                // (reproducible aunque cambien las reglas de abajo).
                let (giro_r, alto_r, a_r, b_r) = (rng.next_f32(), rng.next_f32(), rng.next_f32(), rng.next_f32());
                if celda_torre == Some((cara, i as f32, j as f32)) {
                    continue; // la manzana de la torre queda libre
                }
                let (cu, cv) = (i as f32 + 0.5, j as f32 + 0.5);
                let dir = direccion_cuadricula(cara, cu, cv, n);
                // Eje u de la cuadrícula en ese punto: alinea el edificio con la calle.
                let eje = direccion_cuadricula(cara, cu + 0.3, cv, n) - dir;
                let h = ground.terrain.height(ground.noise, dir) / terreno.amplitude;
                if h <= terreno.sea_level.unwrap_or(-1.0) + 0.08 {
                    continue; // orilla o agua
                }
                match tipo_manzana(hash_celda(cara, i, j, semilla), CIUDAD_PARQUES) {
                    Manzana::Parque => {
                        for (du, dv) in [(0.3, 0.3), (0.7, 0.68)] {
                            let d = direccion_cuadricula(cara, i as f32 + du + (a_r - 0.5) * 0.1, j as f32 + dv + (b_r - 0.5) * 0.1, n);
                            let s = manzana * (0.55 + 0.25 * a_r);
                            arboles.push(sobre_suelo(&planeta, d, eje, giro_r * TAU, Vec3::splat(s), 0.03));
                        }
                    }
                    Manzana::Plaza => {}
                    Manzana::Edificios => {
                        // Una de cada cuatro manzanas queda sin edificio (lotes
                        // y estacionamientos): así se ven las calles y los techos.
                        if a_r < 0.25 {
                            continue;
                        }
                        // Huella del modelo ≈ 1: la escala horizontal deja libre la acera.
                        let ancho = manzana * 0.46;
                        let giro = (giro_r * 4.0).floor() * (TAU / 4.0); // solo giros de 90°
                        if centro_ciudad.fbm(dir * 1.3, 3) > 0.18 {
                            let alto = manzana * (0.38 + 0.24 * alto_r);
                            altos.push(sobre_suelo(&planeta, dir, eje, giro, Vec3::new(ancho, alto, ancho), 0.06));
                        } else {
                            let alto = manzana * (0.45 + 0.35 * alto_r);
                            bajos.push(sobre_suelo(&planeta, dir, eje, giro, Vec3::new(ancho, alto, ancho), 0.06));
                        }
                    }
                }
            }
        }
    }

    hornear(
        &mut planeta,
        vec![
            ("Torre de radio", &antena, torres),
            ("Rascacielos", &rascacielos, altos),
            ("Edificios", &edificio, bajos),
            ("Arboles", &arbol, arboles),
        ],
    );
    Ok(planeta)
}
