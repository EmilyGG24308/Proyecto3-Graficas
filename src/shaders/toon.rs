//! Toon shader (cel shading) para la nave y los modelos.
//!
//! - La luz difusa de Lambert se cuantiza en bandas duras (sombra, medio, luz).
//! - El brillo especular de Blinn-Phong se vuelve una mancha blanca con borde
//!   duro (look de caricatura / plástico de juguete).
//! - Un término de Fresnel oscurece la silueta (contorno falso) y otro
//!   ilumina el borde opuesto al sol (rim light).
//! - Los materiales emisivos (Ke en el .mtl) se pintan sin iluminación.

use crate::math::{Vec3, smoothstep};
use crate::shader::{Shader, Uniforms, Varyings, blinn_phong, lambert, rim};

pub struct ToonShader {
    /// Luz mínima en la zona de sombra (para que la nave siempre se lea).
    pub ambient: f32,
    /// Grosor del contorno oscuro (0 = sin contorno).
    pub outline: f32,
}

impl Shader for ToonShader {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let albedo = v.color;
        if v.emissive > 0.5 {
            // Pulso suave para que el foco de la antena "respire".
            return albedo * (1.1 + 0.25 * (u.time * 4.0).sin());
        }
        let n = v.normal.normalize();

        // Bandas de luz difusa con bordes ligeramente suavizados.
        let d = lambert(n, v.world_pos, u.sun_pos);
        let band = self.ambient
            + (0.55 - self.ambient) * smoothstep(0.02, 0.06, d)
            + 0.45 * smoothstep(0.45, 0.5, d);

        // Brillo especular duro.
        let spec = blinn_phong(n, v.world_pos, u.sun_pos, u.camera_pos, 40.0);
        let spec = smoothstep(0.5, 0.55, spec) * 0.6;

        let r = rim(n, v.world_pos, u.camera_pos);
        // Luz de borde del lado no iluminado (le da volumen en la sombra).
        let rim_light = smoothstep(0.6, 0.8, r) * (1.0 - d) * 0.25;
        let mut c = albedo * band + Vec3::splat(spec) + Vec3::new(0.4, 0.6, 1.0) * rim_light;

        // Contorno: oscurecer donde la normal es casi perpendicular a la vista.
        if self.outline > 0.0 {
            let o = smoothstep(1.0 - self.outline, 1.0 - self.outline * 0.5, r);
            c = c * (1.0 - 0.75 * o);
        }
        c
    }
}
