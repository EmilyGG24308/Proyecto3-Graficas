//! Planeta bosque: pradera con manchas de pasto, florecitas, lagos, playas,
//! montañas de roca y nieve en las cumbres.
//!
//! El color depende de la ALTURA real del terreno (la distancia del vértice
//! al centro en espacio local), así que los lagos coinciden con las zonas
//! aplanadas por el nivel del mar y la nieve con las montañas deformadas.
//! Las flores son un patrón celular: el espacio se divide en celdas 3D y en
//! algunas (elegidas por un hash) se pinta un punto de color.

use crate::math::{Vec3, fract, smoothstep};
use crate::noise::{Perlin, hash3};
use crate::shader::{Shader, Uniforms, Varyings, blinn_phong, lambert, rim, toon_diffuse};

pub struct ForestShader {
    pub noise: Perlin,
    /// Debe coincidir con `Terrain::amplitude` del planeta.
    pub amplitude: f32,
    /// Debe coincidir con `Terrain::sea_level` del planeta (en unidades de amplitud).
    pub sea_level: f32,
    /// Densidad de flores (0 = ninguna, 0.2 = muchas).
    pub flower_density: f32,
}

impl Shader for ForestShader {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let n = v.normal.normalize();
        let p = v.local_pos;
        // Altura en [-1, 1] aprox. (en unidades de la amplitud del terreno).
        let h = (p.length() - 1.0) / self.amplitude;
        let water_line = self.sea_level + 0.01;

        let d = lambert(n, v.world_pos, u.sun_pos);
        let light = toon_diffuse(d, 0.18);

        let albedo = if h <= water_line {
            // Lago: más claro en la orilla.
            let shore = smoothstep(self.sea_level - 0.25, water_line, h);
            let deep = Vec3::new(0.10, 0.38, 0.78);
            let shallow = Vec3::new(0.25, 0.75, 0.85);
            let water = deep.lerp(shallow, shore);
            let spec = smoothstep(0.55, 0.6, blinn_phong(n, v.world_pos, u.sun_pos, u.camera_pos, 60.0));
            return water * light + Vec3::splat(spec * 0.8);
        } else if h < water_line + 0.02 {
            Vec3::new(0.95, 0.86, 0.55) // playa
        } else {
            // Pradera: dos verdes en manchas grandes.
            let patches = self.noise.fbm(p * 5.0, 3);
            let grass_a = Vec3::new(0.36, 0.78, 0.30);
            let grass_b = Vec3::new(0.22, 0.60, 0.25);
            let mut c = grass_a.lerp(grass_b, smoothstep(-0.05, 0.05, patches));

            // Florecitas (solo en la pradera baja).
            let cell = p * 55.0;
            let id = Vec3::new(cell.x.floor(), cell.y.floor(), cell.z.floor());
            let local = Vec3::new(fract(cell.x), fract(cell.y), fract(cell.z)) - Vec3::splat(0.5);
            let r = hash3(id);
            if r < self.flower_density && local.length() < 0.3 && h < 0.2 {
                let palette = [
                    Vec3::new(1.0, 0.45, 0.65),
                    Vec3::new(1.0, 0.9, 0.3),
                    Vec3::new(0.95, 0.95, 1.0),
                ];
                c = palette[((r / self.flower_density) * 3.0) as usize % 3];
            }

            // Roca en las laderas altas y nieve en las cumbres. (El fBm
            // rara vez pasa de 0.4, por eso los umbrales son bajos.)
            let rock = Vec3::new(0.55, 0.47, 0.40);
            c = c.lerp(rock, smoothstep(0.22, 0.3, h));
            c.lerp(Vec3::new(0.97, 0.97, 1.0), smoothstep(0.34, 0.38, h))
        };

        let mut col = albedo * light;
        // Atmósfera: borde azulado, más fuerte del lado iluminado.
        let atmo = smoothstep(0.55, 1.0, rim(n, v.world_pos, u.camera_pos)) * (0.3 + 0.7 * d);
        col += Vec3::new(0.45, 0.75, 1.0) * (atmo * 0.55);
        col
    }
}
