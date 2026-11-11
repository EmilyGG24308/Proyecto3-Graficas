//! Planeta embrujado: tierra muerta bajo luz de luna, senderos pálidos,
//! farolitos que parpadean y niebla fantasmal que se arrastra con el tiempo.
//!
//! Técnicas (distintas a las de los otros shaders):
//!  - Luz "de luna": la difusa se tiñe de azul frío y la sombra de violeta,
//!    con muy poca luz ambiental.
//!  - Senderos: líneas sinuosas donde |ruido| ≈ 0.
//!  - Farolitos: ruido celular; en algunas celdas (elegidas por su id) hay
//!    una luz cálida cuyo brillo parpadea con una función del tiempo
//!    distinta por farol (cada uno con su propia fase).
//!  - Niebla animada: domain warping que se desplaza con el tiempo; es más
//!    densa en las zonas bajas y brilla débilmente en el lado nocturno.
//!  - Borde (Fresnel) verde espectral que "respira".

use crate::math::{Vec3, smoothstep};
use crate::noise::{Perlin, worley};
use crate::shader::{Shader, Uniforms, Varyings, lambert, rim};

pub struct HauntedShader {
    pub noise: Perlin,
    /// Debe coincidir con `Terrain::amplitude` del planeta.
    pub amplitude: f32,
    /// Densidad de la niebla (0 = sin niebla, 1 = muy densa).
    pub fog_density: f32,
    /// Velocidad con la que se mueve la niebla.
    pub fog_speed: f32,
    /// Proporción de celdas con farolito (0..1).
    pub lantern_density: f32,
}

impl Shader for HauntedShader {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let n = v.normal.normalize();
        let p = v.local_pos;
        let dir = p.normalize();
        let h = (p.length() - 1.0) / self.amplitude; // ~[-1, 1]
        let t = u.time;

        // --- Luz de luna: bandas frías, sombra violeta -----------------------
        let d = lambert(n, v.world_pos, u.sun_pos);
        let moon = smoothstep(0.0, 0.1, d) * 0.45 + smoothstep(0.4, 0.5, d) * 0.35;
        let light = Vec3::new(0.22, 0.15, 0.32) + Vec3::new(0.75, 0.85, 1.1) * moon;

        // --- Suelo muerto -----------------------------------------------------
        let patches = self.noise.fbm(dir * 7.0, 4);
        let dead_grass = Vec3::new(0.38, 0.42, 0.26);
        let soil = Vec3::new(0.30, 0.20, 0.32);
        let mut c = dead_grass.lerp(soil, smoothstep(-0.1, 0.15, patches));
        // Peñascos en las crestas.
        c = c.lerp(Vec3::new(0.45, 0.43, 0.50), smoothstep(0.55, 0.8, h));

        // Senderos pálidos y sinuosos.
        let path = self.noise.fbm(dir * 4.0 + Vec3::splat(7.7), 3).abs();
        let path_mask = 1.0 - smoothstep(0.010, 0.018, path);
        c = c.lerp(Vec3::new(0.55, 0.50, 0.48), path_mask);

        let mut col = c.hadamard(light);

        // --- Farolitos parpadeantes ------------------------------------------
        let cell = worley(dir * 18.0);
        if cell.id < self.lantern_density {
            let phase = cell.id * 97.0;
            // Parpadeo irregular: suma de senos de frecuencias no múltiplos.
            let flicker = 0.55
                + 0.25 * (t * 9.0 + phase).sin()
                + 0.2 * (t * 23.0 + phase * 1.7).sin().max(0.0);
            let core = 1.0 - smoothstep(0.07, 0.15, cell.f1);
            let halo = (1.0 - smoothstep(0.12, 0.55, cell.f1)) * 0.45;
            col += Vec3::new(1.0, 0.62, 0.22) * ((core + halo) * flicker);
        }

        // --- Niebla fantasmal animada ----------------------------------------
        let drift = Vec3::new(t * self.fog_speed, 0.0, -t * self.fog_speed * 0.7);
        let warp = Vec3::new(
            self.noise.fbm(dir * 3.0 + drift, 3),
            self.noise.fbm(dir * 3.0 - drift + Vec3::splat(4.1), 3),
            0.0,
        );
        let fog_n = self.noise.fbm(dir * 5.0 + warp * 2.0 + drift * 0.5, 4) * 0.5 + 0.5;
        // Más densa en lo bajo.
        let low = 1.0 - smoothstep(0.0, 0.7, h);
        let fog = smoothstep(0.45, 0.8, fog_n) * (0.35 + 0.65 * low) * self.fog_density;
        let fog_col = Vec3::new(0.55, 0.85, 0.80).lerp(Vec3::new(0.70, 0.55, 0.95), fog_n);
        // En el lado nocturno la niebla brilla un poco por sí sola.
        let night_glow = 1.0 - smoothstep(0.0, 0.3, d);
        col = col.lerp(fog_col * (0.35 + 0.4 * moon), fog * 0.7) + fog_col * (fog * night_glow * 0.25);

        // --- Borde espectral que respira -------------------------------------
        let breathe = 0.75 + 0.25 * (t * 1.3).sin();
        let r = smoothstep(0.5, 1.0, rim(n, v.world_pos, u.camera_pos));
        col + Vec3::new(0.35, 1.0, 0.6) * (r * 0.35 * breathe)
    }
}
