//! Planeta jungla jurásica.
//!
//! Capas, de abajo hacia arriba (según la ALTURA real del terreno deformado):
//!  - Pantanos (lo aplanado por el nivel del mar): agua turbia verde-azulada.
//!  - Ríos: donde el valor absoluto de un ruido es casi cero se forma una
//!    línea sinuosa ("ridge" invertido) → cauces que serpentean por la selva.
//!  - Selva: ruido celular (Voronoi). Cada celda es la copa de un árbol: el
//!    centro de la celda se aclara y el borde se oscurece (f2 - f1 ≈ 0), y
//!    cada copa toma un verde distinto según el id de su celda.
//!  - Acantilados: donde la pendiente es fuerte (la normal se aleja de la
//!    vertical) aparece roca con estratos (bandas por altura).
//!  - Volcanes: en las cumbres, basalto oscuro y un cráter de lava que
//!    pulsa con el tiempo (animado y sin iluminar: brilla de noche).
//!  - Neblina húmeda en los valles y atmósfera verdosa en el borde.

use crate::math::{Vec3, smoothstep};
use crate::noise::{Perlin, worley};
use crate::shader::{Shader, Uniforms, Varyings, blinn_phong, lambert, rim, toon_diffuse};

pub struct JungleShader {
    pub noise: Perlin,
    /// Debe coincidir con `Terrain::amplitude` del planeta.
    pub amplitude: f32,
    /// Debe coincidir con `Terrain::sea_level` (en unidades de amplitud).
    pub sea_level: f32,
    /// Tamaño de las copas de los árboles (más alto = árboles más chicos).
    pub canopy_scale: f32,
    /// Altura (en unidades de amplitud) donde empiezan los volcanes.
    pub volcano_height: f32,
}

impl Shader for JungleShader {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let n = v.normal.normalize();
        let p = v.local_pos;
        let h = (p.length() - 1.0) / self.amplitude;
        let dir = p.normalize();

        let d = lambert(n, v.world_pos, u.sun_pos);
        let light = toon_diffuse(d, 0.16);

        // Pendiente: 0 en terreno plano, crece en laderas empinadas.
        let up = (v.world_pos - u.object_center).normalize();
        let slope = 1.0 - n.dot(up).clamp(0.0, 1.0);

        // --- Lava del volcán (emisiva, animada) ------------------------------
        // El cráter es solo la punta: el resto del cono es basalto.
        let crater = self.volcano_height + 0.12;
        if h > crater {
            let flow = self.noise.fbm(dir * 30.0 + Vec3::new(0.0, u.time * 0.4, 0.0), 3);
            let pulse = 0.75 + 0.25 * (u.time * 2.5 + flow * 6.0).sin();
            let hot = Vec3::new(1.0, 0.85, 0.3).lerp(Vec3::new(1.0, 0.3, 0.05), smoothstep(-0.2, 0.3, flow));
            return hot * (1.2 * pulse);
        }

        // --- Pantano ----------------------------------------------------------
        let water_line = self.sea_level + 0.015;
        if h <= water_line {
            let murk = self.noise.fbm(dir * 14.0, 3) * 0.5 + 0.5;
            let swamp = Vec3::new(0.08, 0.36, 0.34).lerp(Vec3::new(0.30, 0.50, 0.22), murk);
            let spec = smoothstep(0.5, 0.56, blinn_phong(n, v.world_pos, u.sun_pos, u.camera_pos, 50.0));
            return swamp * light + Vec3::splat(spec * 0.5) + self.mist(h, d);
        }

        // --- Selva (copas de árboles con ruido celular) -----------------------
        let cell = worley(dir * self.canopy_scale);
        // Bordes suaves: se lee como follaje, no como mosaico.
        let crown_center = 1.0 - smoothstep(0.0, 0.8, cell.f1);
        let crown_edge = smoothstep(0.0, 0.2, cell.f2 - cell.f1);
        let greens = [
            Vec3::new(0.10, 0.42, 0.14),
            Vec3::new(0.20, 0.55, 0.12),
            Vec3::new(0.36, 0.62, 0.15),
            Vec3::new(0.12, 0.35, 0.20),
        ];
        let leaf = greens[(cell.id * 4.0) as usize % 4];
        let mut c = leaf * (0.7 + 0.3 * crown_center) * (0.72 + 0.28 * crown_edge);

        // Claros de tierra con helechos en zonas bajas.
        let clearing = self.noise.fbm(dir * 6.0 + Vec3::splat(3.3), 3);
        c = c.lerp(Vec3::new(0.42, 0.40, 0.18), smoothstep(0.18, 0.24, clearing) * 0.8);

        // --- Ríos (ruido con valor absoluto casi cero) -----------------------
        let river = self.noise.fbm(dir * 3.2 + Vec3::splat(11.0), 4).abs();
        let river_mask = (1.0 - smoothstep(0.012, 0.022, river)) * (1.0 - smoothstep(0.1, 0.2, h));
        c = c.lerp(Vec3::new(0.20, 0.45, 0.42), river_mask);

        // --- Acantilados con estratos ----------------------------------------
        let strata = 0.5 + 0.5 * (h * 40.0 + self.noise.noise(dir * 8.0) * 3.0).sin();
        let rock = Vec3::new(0.50, 0.36, 0.22).lerp(Vec3::new(0.70, 0.55, 0.35), strata);
        c = c.lerp(rock, smoothstep(0.18, 0.28, slope));

        // --- Ladera del volcán: basalto oscuro con brillo de lava cerca -------
        let basalt = smoothstep(self.volcano_height - 0.04, self.volcano_height, h);
        c = c.lerp(Vec3::new(0.18, 0.15, 0.14), basalt);
        let glow = smoothstep(crater - 0.03, crater, h);

        let mut col = c * light + Vec3::new(1.0, 0.35, 0.05) * (glow * 0.6);
        col += self.mist(h, d);

        // Atmósfera húmeda verdosa en el borde.
        let atmo = smoothstep(0.55, 1.0, rim(n, v.world_pos, u.camera_pos)) * (0.3 + 0.7 * d);
        col + Vec3::new(0.55, 0.85, 0.55) * (atmo * 0.45)
    }
}

impl JungleShader {
    /// Neblina blanquecina que se acumula en los valles (alturas bajas).
    fn mist(&self, h: f32, d: f32) -> Vec3 {
        let amount = (1.0 - smoothstep(self.sea_level, self.sea_level + 0.1, h)) * 0.1;
        Vec3::new(0.85, 0.95, 0.85) * (amount * (0.3 + 0.7 * d))
    }
}
