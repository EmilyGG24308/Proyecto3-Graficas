//! Planeta Hielo / océano: shader de olas y casquetes, y sus objetos.

use std::f32::consts::TAU;

use crate::config::WORLD_SEED;
use crate::math::{Mat4, Vec3, smoothstep};
use crate::mesh::Mesh;
use crate::noise::{Perlin, Rng, worley};
use crate::planet::{Orbit, Planet, PlanetDef, Terrain, TerrainKind};
use crate::planets::model;
use crate::props::{Ground, Placement, PropSpec, place};
use crate::shader::{Shader, Uniforms, Varyings, blinn_phong, lambert, rim, toon_diffuse};

use super::comun::{hornear, noche, sobre_suelo};

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

pub(super) fn hielo() -> Result<Planet, String> {
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
    fn pinguinos_solo_en_casquetes_y_faro_unico() {
        let p = &hielo().unwrap();
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
