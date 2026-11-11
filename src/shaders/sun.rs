//! Shader del sol: plasma caricaturesco animado.
//!
//! Técnica: "domain warping" (se deforma la entrada del ruido con otro ruido)
//! desplazado con el tiempo, y luego el valor se cuantiza en bandas para un
//! look de caricatura (cel shading). El borde de la silueta se tiñe de rojo
//! anaranjado con un término de Fresnel.

use crate::math::{Vec3, smoothstep};
use crate::noise::Perlin;
use crate::shader::{Shader, Uniforms, Varyings, rim};

pub struct SunShader {
    pub noise: Perlin,
    /// Multiplicador de la velocidad de la animación.
    pub speed: f32,
    /// Número de bandas de color (más = más suave).
    pub bands: f32,
}

impl Shader for SunShader {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let n = v.normal.normalize();
        let t = u.time * self.speed;
        let p = v.local_pos * 2.2;

        // Domain warping: q desplaza las coordenadas del segundo ruido.
        let q = Vec3::new(
            self.noise.fbm(p + Vec3::new(0.0, t, 0.0), 3),
            self.noise.fbm(p + Vec3::new(5.2, -t * 0.7, 1.3), 3),
            self.noise.fbm(p + Vec3::new(1.7, 9.2, t * 0.5), 3),
        );
        let f = self.noise.fbm(p + q * 1.8 + Vec3::new(t * 0.3, 0.0, 0.0), 4) * 0.5 + 0.5;

        // Cuantizar en bandas (cel shading).
        let banded = (f * self.bands).floor() / (self.bands - 1.0);

        let deep = Vec3::new(1.0, 0.42, 0.05);
        let mid = Vec3::new(1.0, 0.72, 0.12);
        let hot = Vec3::new(1.0, 0.97, 0.55);
        let base = if banded < 0.5 {
            deep.lerp(mid, banded * 2.0)
        } else {
            mid.lerp(hot, (banded - 0.5) * 2.0)
        };

        // Borde brillante rojo-anaranjado.
        let r = rim(n, v.world_pos, u.camera_pos);
        let edge = smoothstep(0.55, 0.95, r);
        base.lerp(Vec3::new(1.0, 0.3, 0.08), edge * 0.8)
    }
}
