//! Mundos de Emily: Ciudad (órbita 45) y Hielo / océano (órbita 120).
//!
//! Todo lo de estos planetas vive en este archivo: definición, shaders y
//! reglas de objetos. Los modelos salen de `assets/models/modelos_emily.py`.
//!
//! Ciudad: un planeta casi plano cubierto por una cuadrícula de calles. La
//! cuadrícula se proyecta desde un cubo (como un "cube map"), así no hay
//! costuras ni calles que se junten en los polos. El MISMO cálculo de
//! cuadrícula decide en el shader qué es calle, parque o plaza, y en la CPU
//! dónde va cada edificio: por eso los edificios caen dentro de las
//! manzanas y nunca encima de una calle.
//!
//! Hielo / océano: un mar alto cubre casi todo; quedan islas. El agua tiene
//! olas animadas con normal mapping procedural (la normal se perturba con la
//! derivada analítica de las olas), espuma en crestas y en la costa,
//! casquetes polares por latitud con témpanos rotos en el borde y una
//! aurora que solo se ve del lado nocturno.

use std::f32::consts::{FRAC_PI_4, TAU};

use crate::config::WORLD_SEED;
use crate::math::{Mat4, Vec3, smoothstep};
use crate::mesh::Mesh;
use crate::noise::{Perlin, Rng, hash3, worley};
use crate::planet::{Orbit, Planet, PlanetDef, Terrain, TerrainKind};
use crate::planets::model;
use crate::props::{BakedProps, Ground, Placement, PropSpec, append_transformed, place};
use crate::shader::{Shader, Uniforms, Varyings, blinn_phong, lambert, rim, toon_diffuse};

/// Planetas de Emily. Se agregan solos al sistema solar (índices 6 y 7 en
/// `--planeta N`).
pub fn planetas() -> Result<Vec<Planet>, String> {
    Ok(vec![ciudad()?, hielo()?])
}

// ===========================================================================
// Utilidades compartidas
// ===========================================================================

/// Noche en [0, 1] según la normal GEOMÉTRICA de la esfera (no la del
/// relieve): así el límite día/noche es una línea limpia y las luces de la
/// ciudad o la aurora no parpadean con cada lomita.
fn noche(v: &Varyings, u: &Uniforms) -> f32 {
    let arriba = (v.world_pos - u.object_center).normalize();
    1.0 - smoothstep(0.0, 0.12, lambert(arriba, v.world_pos, u.sun_pos))
}

/// Hash entero de una celda (cara, i, j) a [0, 1). Es exacto (no usa senos),
/// así la CPU y el shader obtienen el mismo número para la misma manzana.
fn hash_celda(cara: usize, i: i32, j: i32, semilla: u32) -> f32 {
    let mut h = (i as u32).wrapping_mul(0x9E37_79B1)
        ^ (j as u32).wrapping_mul(0x85EB_CA77)
        ^ (cara as u32).wrapping_mul(0xC2B2_AE3D)
        ^ semilla.wrapping_mul(0x27D4_EB2F);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Coloca un modelo apoyado en el terreno: "arriba" = dirección radial,
/// "derecha" = `eje` proyectado sobre el plano tangente (para alinear los
/// edificios con las calles). `escala` va en unidades del mundo.
fn sobre_suelo(planeta: &Planet, dir: Vec3, eje: Vec3, giro: f32, escala: Vec3, hundir: f32) -> Mat4 {
    let arriba = dir.normalize();
    let mut derecha = eje - arriba * eje.dot(arriba);
    if derecha.length_sq() < 1e-6 {
        derecha = if arriba.y.abs() < 0.9 { Vec3::Y.cross(arriba) } else { Vec3::X.cross(arriba) };
    }
    let derecha = derecha.normalize();
    let atras = derecha.cross(arriba);
    let r = planeta.def.radius;
    let suelo = planeta.surface_point_local(arriba) - arriba * (hundir / r);
    Mat4::translation(suelo)
        * Mat4::from_basis(derecha, arriba, atras)
        * Mat4::rotation_y(giro)
        * Mat4::scale(escala / r)
}

/// Junta todas las instancias en la malla de objetos del planeta y actualiza
/// su esfera envolvente (lo mismo que hace `Planet::with_props`).
fn hornear(planeta: &mut Planet, grupos: Vec<(&'static str, &Mesh, Vec<Mat4>)>) {
    let mut malla = Mesh::default();
    let mut cuentas = Vec::new();
    for (nombre, modelo, matrices) in &grupos {
        for m in matrices {
            append_transformed(&mut malla, modelo, m);
        }
        cuentas.push((*nombre, matrices.len()));
    }
    malla.compute_bounding_radius();
    planeta.bounding_radius = planeta.bounding_radius.max(malla.bounding_radius * planeta.def.radius);
    planeta.props = Some(BakedProps { mesh: malla, counts: cuentas });
}

// ===========================================================================
// Cuadrícula sobre la esfera (proyección de cubo equiangular)
// ===========================================================================

/// Las 6 caras de un cubo: (normal, eje u, eje v).
const CARAS: [(Vec3, Vec3, Vec3); 6] = [
    (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0)),
    (Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 1.0, 0.0)),
    (Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0)),
    (Vec3::new(0.0, -1.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
    (Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
    (Vec3::new(0.0, 0.0, -1.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
];

/// Dirección (unitaria) -> (cara, gu, gv) con gu, gv en [0, n): la parte
/// entera es la manzana y la fraccionaria la posición dentro de ella.
///
/// Por qué un cubo: con latitud/longitud las calles se amontonarían en los
/// polos. Proyectando la esfera sobre las 6 caras de un cubo cada cara es
/// un cuadrado normal. El `atan` ("equiangular") corrige que las manzanas
/// cerca de las esquinas del cubo salgan más chicas que las del centro.
fn cuadricula(d: Vec3, n: f32) -> (usize, f32, f32) {
    let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
    let cara = if ax >= ay && ax >= az {
        if d.x > 0.0 { 0 } else { 1 }
    } else if ay >= az {
        if d.y > 0.0 { 2 } else { 3 }
    } else if d.z > 0.0 {
        4
    } else {
        5
    };
    let (normal, eu, ev) = CARAS[cara];
    let k = d.dot(normal);
    let u = (d.dot(eu) / k).atan() / FRAC_PI_4;
    let v = (d.dot(ev) / k).atan() / FRAC_PI_4;
    let a_celda = |x: f32| ((x * 0.5 + 0.5) * n).clamp(0.0, n - 1e-4);
    (cara, a_celda(u), a_celda(v))
}

/// Inversa de `cuadricula`: de coordenadas de la cuadrícula a dirección.
fn direccion_cuadricula(cara: usize, gu: f32, gv: f32, n: f32) -> Vec3 {
    let (normal, eu, ev) = CARAS[cara];
    let tu = ((gu / n * 2.0 - 1.0) * FRAC_PI_4).tan();
    let tv = ((gv / n * 2.0 - 1.0) * FRAC_PI_4).tan();
    (normal + eu * tu + ev * tv).normalize()
}

/// Qué hay en una manzana. Lo decide su hash, igual en el shader y en la CPU.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Manzana {
    Parque,
    Plaza,
    Edificios,
}

fn tipo_manzana(hash: f32, parques: f32) -> Manzana {
    if hash < parques {
        Manzana::Parque
    } else if hash < parques + 0.08 {
        Manzana::Plaza
    } else {
        Manzana::Edificios
    }
}

// ===========================================================================
// Planeta Ciudad
// ===========================================================================

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

fn ciudad() -> Result<Planet, String> {
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

// ===========================================================================
// Planeta Hielo / océano
// ===========================================================================

/// Un tren de olas: dirección (en la esfera), número de onda, velocidad y
/// altura. Varios trenes con direcciones distintas se ven como mar picado.
const OLAS: [(Vec3, f32, f32, f32); 3] = [
    (Vec3::new(0.80, 0.20, 0.56), 38.0, 1.6, 0.45),
    (Vec3::new(-0.35, 0.50, 0.79), 55.0, -2.1, 0.33),
    (Vec3::new(0.10, -0.90, 0.42), 80.0, 2.7, 0.22),
];

/// Shader del planeta de hielo y océano.
///
/// Técnicas (distintas a Cristalino, que usa vetas de ruido y brillo violeta):
///  - Olas: suma de senos sobre la esfera `sin(k · dir * f + v * t)`. Su
///    derivada analítica (cosenos) da el gradiente de la "altura" del agua;
///    restarlo a la normal es un normal map procedural (las olas se ven en
///    la luz y en el brillo especular, sin mover un solo vértice).
///  - Profundidad: se vuelve a evaluar el MISMO ruido del terreno debajo del
///    agua (que quedó aplanada por `sea_level`): aguas someras más claras
///    cerca de las islas, azul marino en lo profundo.
///  - Espuma: en las crestas de las olas y en anillos que avanzan hacia la
///    costa con el tiempo (`sin(profundidad * k + v * t)`).
///  - Casquetes polares por latitud (`|local_pos.y|`), con un borde
///    ondulado por ruido; en la orilla, ruido celular forma témpanos
///    sueltos: cada celda es hielo o agua según su id.
///  - Aurora del lado nocturno: cortinas verdes/rosas sobre los casquetes,
///    hechas con senos de la longitud que se desplazan con el tiempo.
pub struct HieloShader {
    /// Ruido del terreno (misma semilla que el planeta) para la profundidad.
    pub terreno: Perlin,
    pub frecuencia_terreno: f32,
    /// Ruido para el borde ondulado de los casquetes.
    pub ruido: Perlin,
    /// Debe coincidir con `Terrain::amplitude`.
    pub amplitud: f32,
    /// Debe coincidir con `Terrain::sea_level` (unidades de amplitud).
    pub nivel_mar: f32,
    /// Latitud (|y| en la esfera unitaria) donde empieza el casquete.
    pub casquete: f32,
    /// Intensidad del normal map de las olas (0 = mar liso).
    pub olas: f32,
    /// Multiplica la velocidad de las olas y la espuma.
    pub velocidad_olas: f32,
    /// Intensidad de la aurora (0 = apagada).
    pub aurora: f32,
}

impl Shader for HieloShader {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let n0 = v.normal.normalize();
        let p = v.local_pos;
        let dir = p.normalize();
        let h = (p.length() - 1.0) / self.amplitud;
        let t = u.time * self.velocidad_olas;
        let noche = noche(v, u);

        // --- Casquete: latitud con borde ondulado ---------------------------
        let lat = dir.y.abs();
        // Un solo fBm de baja frecuencia sirve para dos cosas: ondular el
        // borde del casquete y decidir dónde el mar está picado (espuma).
        let ondulacion = self.ruido.fbm(dir * 3.0, 3);
        let orilla = self.casquete + 0.06 * ondulacion;
        let en_casquete = lat > orilla;
        // Franja de témpanos (agua con hielo roto) justo antes del casquete.
        let franja = smoothstep(orilla - 0.10, orilla, lat);

        let mut col = if en_casquete || (franja > 0.0 && h <= self.nivel_mar + 0.01) {
            // Una sola llamada a worley: grietas del casquete o témpanos.
            let celda = worley(dir * 16.0);
            if en_casquete || celda.id < franja * 0.9 {
                let d = lambert(n0, v.world_pos, u.sun_pos);
                let luz = toon_diffuse(d, 0.20);
                let grieta = 1.0 - smoothstep(0.02, 0.07, celda.f2 - celda.f1);
                let nieve = Vec3::new(0.95, 0.97, 1.0).lerp(Vec3::new(0.55, 0.75, 0.98), grieta * 0.8);
                // Sombra azulada (la nieve no se ve gris en sombra, se ve azul).
                nieve * luz + Vec3::new(0.05, 0.12, 0.28) * (1.0 - smoothstep(0.0, 0.4, d))
            } else {
                self.mar(v, u, dir, n0, t, ondulacion)
            }
        } else if h <= self.nivel_mar + 0.01 {
            self.mar(v, u, dir, n0, t, ondulacion)
        } else {
            // --- Islas: playa, tundra, roca y nieve en lo alto ---------------
            let d = lambert(n0, v.world_pos, u.sun_pos);
            let luz = toon_diffuse(d, 0.18);
            let arriba = (v.world_pos - u.object_center).normalize();
            let pendiente = 1.0 - n0.dot(arriba).clamp(0.0, 1.0);
            let sobre_mar = h - self.nivel_mar;
            let mut c = Vec3::new(0.94, 0.86, 0.62); // playa
            c = c.lerp(Vec3::new(0.42, 0.58, 0.40), smoothstep(0.05, 0.09, sobre_mar)); // tundra
            c = c.lerp(Vec3::new(0.45, 0.44, 0.50), smoothstep(0.12, 0.22, pendiente)); // roca
            c = c.lerp(Vec3::new(0.95, 0.97, 1.0), smoothstep(0.38, 0.44, sobre_mar)); // nieve
            c * luz
        };

        // --- Aurora (solo de noche, alrededor de los casquetes) -------------
        if self.aurora > 0.0 && noche > 0.0 {
            let banda = smoothstep(orilla - 0.22, orilla - 0.08, lat) * (1.0 - smoothstep(orilla + 0.05, orilla + 0.2, lat));
            if banda > 0.0 {
                let lon = dir.z.atan2(dir.x);
                let cortina = (lon * 5.0 + (lon * 3.0 + u.time * 0.5).sin() * 1.3 + u.time * 0.25).sin();
                let brillo = smoothstep(0.3, 1.0, cortina) * banda * noche * self.aurora;
                let color = Vec3::new(0.30, 1.0, 0.55).lerp(Vec3::new(1.0, 0.40, 0.80), smoothstep(orilla - 0.1, orilla + 0.1, lat));
                col += color * (brillo * 0.75);
            }
        }

        // --- Atmósfera fría y clara en el borde -----------------------------
        let borde = smoothstep(0.55, 1.0, rim(n0, v.world_pos, u.camera_pos));
        col + Vec3::new(0.70, 0.85, 1.0) * (borde * 0.35)
    }
}

impl HieloShader {
    /// Océano con olas (normal map procedural), profundidad y espuma.
    fn mar(&self, v: &Varyings, u: &Uniforms, dir: Vec3, n0: Vec3, t: f32, ondulacion: f32) -> Vec3 {
        // Altura de las olas y su gradiente (derivada analítica de cada seno).
        // `ondulacion` desfasa cada tren de olas de forma distinta según el
        // lugar ("domain warping"): sin eso la suma de senos forma un patrón
        // de lunares perfectamente regular.
        let mut ola = 0.0;
        let mut gradiente = Vec3::ZERO;
        let mut total = 0.0;
        for (i, (k, f, vel, a)) in OLAS.into_iter().enumerate() {
            let fase = dir.dot(k) * f + t * vel + ondulacion * (6.0 + 3.0 * i as f32);
            ola += a * fase.sin();
            gradiente += k * (a * f * fase.cos());
            total += a;
        }
        let ola = ola / total; // [-1, 1]
        // Solo la parte tangente a la esfera inclina la normal.
        let tangente = gradiente - dir * gradiente.dot(dir);
        let tangente_mundo = u.model.transform_dir(tangente).normalize() * (tangente.length() / total);
        let n = (n0 - tangente_mundo * (self.olas * 0.02)).normalize();

        let d = lambert(n, v.world_pos, u.sun_pos);
        let luz = toon_diffuse(d, 0.18);

        // Profundidad: el terreno "real" bajo el agua aplanada.
        let fondo = self.terreno.fbm(dir * self.frecuencia_terreno, 3);
        let profundidad = (self.nivel_mar - fondo).max(0.0);
        let somero = 1.0 - smoothstep(0.0, 0.22, profundidad);
        let mut agua = Vec3::new(0.04, 0.16, 0.52).lerp(Vec3::new(0.18, 0.55, 0.92), somero);
        agua = agua * (0.9 + 0.12 * ola);

        // Espuma: crestas de olas (solo en las zonas de mar picado que marca
        // el mismo ruido) y anillos que avanzan hacia la costa.
        let cresta = smoothstep(0.6, 0.7, ola) * smoothstep(0.05, 0.25, ondulacion) * 0.8;
        let costa = smoothstep(0.55, 0.8, (profundidad * 70.0 + t * 2.2).sin()) * smoothstep(0.55, 0.85, somero);
        let espuma = cresta.max(costa);
        agua = agua.lerp(Vec3::new(0.93, 0.97, 1.0), espuma);

        // Brillo especular "toon": mancha blanca de borde duro que se mueve
        // con las olas gracias a la normal perturbada.
        let brillo = smoothstep(0.55, 0.6, blinn_phong(n, v.world_pos, u.sun_pos, u.camera_pos, 70.0));
        agua * luz + Vec3::splat(brillo * 0.7)
    }
}

/// Parámetros del hielo compartidos por el shader y la colocación.
const HIELO_CASQUETE: f32 = 0.66;

fn hielo() -> Result<Planet, String> {
    // Mar MUY alto (0.12 en unidades de amplitud): solo las crestas del fBm
    // salen como islas; todo lo demás se aplana en océano.
    let terreno = Terrain { kind: TerrainKind::Smooth, amplitude: 0.08, frequency: 1.5, octaves: 4, sea_level: Some(0.12) };
    let def = PlanetDef {
        name: "Hielo",
        radius: 8.0,
        orbit: Orbit { radius: 120.0, speed: 0.013, phase: 5.3, inclination: 0.07 },
        spin_speed: 0.07,
        // Eje bien inclinado: al acercarse se ve un casquete y el ecuador a la vez.
        axial_tilt: -0.50,
        seed: WORLD_SEED + 31,
        terrain: terreno,
    };
    let mar = terreno.sea_level.unwrap_or(-1.0);
    let shader = HieloShader {
        terreno: Perlin::new(def.seed),
        frecuencia_terreno: terreno.frequency,
        ruido: Perlin::new(def.seed + 100),
        amplitud: terreno.amplitude,
        nivel_mar: mar,
        casquete: HIELO_CASQUETE,
        olas: 1.0,
        velocidad_olas: 1.0,
        aurora: 1.0,
    };
    let mut planeta = Planet::new(def, Box::new(shader));
    let ground = Ground { terrain: &def.terrain, noise: &planeta.noise, radius: def.radius, seed: def.seed };

    // Reglas del motor (`props::place`):
    let reglas = [
        // Icebergs: solo sobre el mar (altura exacta del nivel del mar),
        // agrupados por un ruido de densidad, y hundidos para que floten.
        PropSpec {
            name: "Icebergs",
            mesh: Mesh::default(),
            count: 34,
            placement: Placement::Scatter,
            scale: (0.8, 1.5),
            height: (mar - 0.01, mar + 0.01),
            density_frequency: 2.0,
            density_threshold: 0.0,
            max_slope: 0.2,
            sink: 0.3,
            max_tilt: 0.12,
        },
        // Iglús: aldeas en la tundra de las islas (umbral alto = grupos).
        PropSpec {
            name: "Iglus",
            mesh: Mesh::default(),
            count: 16,
            placement: Placement::Scatter,
            scale: (0.9, 1.2),
            height: (mar + 0.12, mar + 0.45),
            density_frequency: 3.0,
            density_threshold: -0.05,
            max_slope: 0.3,
            sink: 0.04,
            max_tilt: 0.05,
        },
        // Faro: monumento único en la isla más alta.
        PropSpec {
            name: "Faro",
            mesh: Mesh::default(),
            count: 120,
            placement: Placement::Highest,
            scale: (1.3, 1.3),
            height: (mar + 0.05, 2.0),
            density_frequency: 1.0,
            density_threshold: -1.0,
            max_slope: 0.6,
            sink: 0.1,
            max_tilt: 0.0,
        },
    ];
    let icebergs = place(&ground, &reglas[0], 0);
    let iglus = place(&ground, &reglas[1], 1);
    let faro = place(&ground, &reglas[2], 2);

    let pinguinos = colonias_pinguinos(&planeta);

    hornear(
        &mut planeta,
        vec![
            ("Icebergs", &model("hielo_iceberg")?, icebergs),
            ("Iglus", &model("hielo_iglu")?, iglus),
            ("Faro", &model("hielo_faro")?, faro),
            ("Pinguinos", &model("hielo_pinguino")?, pinguinos),
        ],
    );
    Ok(planeta)
}

/// Regla propia: colonias de pingüinos sobre los casquetes. El motor no
/// filtra por latitud, así que se hace en dos pasos: primero se eligen 6
/// centros de colonia con |y| > casquete (alternando polo norte y sur; en
/// espacio local, así los polos giran con el planeta) y luego cada colonia
/// reparte 7 pingüinos alrededor de su centro, con giro y tamaño al azar.
/// Margen de 0.15: el borde ondulado del casquete nunca llega tan arriba.
fn colonias_pinguinos(planeta: &Planet) -> Vec<Mat4> {
    let mut rng = Rng::new(planeta.def.seed ^ 0x9E9);
    let mut pinguinos = Vec::new();
    for colonia in 0..6 {
        let polo = if colonia % 2 == 0 { 1.0 } else { -1.0 };
        let y = rng.range(HIELO_CASQUETE + 0.15, 0.97) * polo;
        let a = rng.range(0.0, TAU);
        let r = (1.0 - y * y).sqrt();
        let centro = Vec3::new(r * a.cos(), y, r * a.sin());
        let referencia = if centro.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let t1 = referencia.cross(centro).normalize();
        let t2 = t1.cross(centro);
        for _ in 0..7 {
            let (ox, oz) = (rng.range(-1.0, 1.0), rng.range(-1.0, 1.0));
            let giro = rng.range(0.0, TAU);
            let escala = rng.range(0.85, 1.25);
            let dir = (centro + (t1 * ox + t2 * oz) * 0.07).normalize();
            pinguinos.push(sobre_suelo(planeta, dir, t1, giro, Vec3::splat(escala), 0.02));
        }
    }
    pinguinos
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cuadricula_ida_y_vuelta() {
        // Del centro de una manzana a dirección y de regreso: misma manzana.
        let n = CIUDAD_MANZANAS;
        for cara in 0..6 {
            for (i, j) in [(0, 0), (3, 5), (6, 6)] {
                let d = direccion_cuadricula(cara, i as f32 + 0.5, j as f32 + 0.5, n);
                let (c, gu, gv) = cuadricula(d, n);
                assert_eq!((c, gu.floor() as i32, gv.floor() as i32), (cara, i, j));
                assert!((gu.fract() - 0.5).abs() < 1e-3 && (gv.fract() - 0.5).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn hash_de_manzana_es_reproducible() {
        assert_eq!(hash_celda(2, 3, 4, 77), hash_celda(2, 3, 4, 77));
        assert_ne!(hash_celda(2, 3, 4, 77), hash_celda(2, 3, 4, 78));
        let h = hash_celda(5, 6, 1, 9);
        assert!((0.0..1.0).contains(&h));
    }

    #[test]
    fn mundos_reproducibles_y_en_presupuesto() {
        let a = planetas().unwrap();
        let b = planetas().unwrap();
        assert_eq!(a.len(), 2);
        assert_eq!((a[0].def.name, a[0].def.orbit.radius, a[0].def.seed), ("Ciudad", 45.0, WORLD_SEED + 30));
        assert_eq!((a[1].def.name, a[1].def.orbit.radius, a[1].def.seed), ("Hielo", 120.0, WORLD_SEED + 31));
        for (x, y) in a.iter().zip(&b) {
            let px = x.props.as_ref().unwrap();
            assert_eq!(px.mesh.positions, y.props.as_ref().unwrap().mesh.positions, "misma semilla, mismos objetos");
            assert!(px.mesh.triangle_count() <= 60_000, "{}: {} triángulos", x.def.name, px.mesh.triangle_count());
            assert!(x.bounding_radius >= px.mesh.bounding_radius * x.def.radius);
            // Al menos dos tipos de objeto con instancias.
            assert!(px.counts.iter().filter(|(_, c)| *c > 0).count() >= 2, "{:?}", px.counts);
        }
    }

    #[test]
    fn pinguinos_solo_en_casquetes_y_faro_unico() {
        let p = &planetas().unwrap()[1];
        let counts = &p.props.as_ref().unwrap().counts;
        let cuenta = |n: &str| counts.iter().find(|(m, _)| *m == n).unwrap().1;
        assert_eq!(cuenta("Faro"), 1);
        assert!(cuenta("Icebergs") > 0 && cuenta("Iglus") > 0);
        let pinguinos = colonias_pinguinos(p);
        assert_eq!(cuenta("Pinguinos"), pinguinos.len());
        for m in &pinguinos {
            let pos = m.transform_point(Vec3::ZERO).normalize();
            assert!(pos.y.abs() > HIELO_CASQUETE + 0.04, "pingüino fuera del casquete: {pos:?}");
            // De pie: su "arriba" apunta hacia fuera del planeta.
            assert!(m.transform_dir(Vec3::Y).normalize().dot(pos) > 0.999);
        }
    }
}
