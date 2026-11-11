//! Fantasmas: translúcidos y brillantes (mezcla aditiva, sin escribir
//! profundidad). Más intensos en el borde (Fresnel), con un ondular interno
//! que se mueve con el tiempo. Las partes oscuras del modelo (ojos y boca)
//! no suman luz, así que se ven como huecos por los que se ve el fondo.

use crate::math::{Vec3, smoothstep};
use crate::shader::{Shader, Uniforms, Varyings, rim};

pub struct GhostShader {
    /// Intensidad general del brillo.
    pub glow: f32,
}

/// Un fantasma concreto: el shader más su visibilidad actual.
pub struct GhostDraw<'a> {
    pub shader: &'a GhostShader,
    pub visibility: f32,
}

impl Shader for GhostDraw<'_> {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        // Ojos y boca: no aportan luz.
        if v.color.max_elem() < 0.2 {
            return Vec3::ZERO;
        }
        let n = v.normal.normalize();
        let r = rim(n, v.world_pos, u.camera_pos);
        let edge = 0.45 + 0.55 * smoothstep(0.2, 0.95, r);
        // Ondas que suben por el cuerpo.
        let waves = 0.85 + 0.15 * (v.local_pos.y * 18.0 - u.time * 4.0).sin();
        let tint = Vec3::new(0.55, 0.95, 1.0).lerp(Vec3::new(0.8, 0.7, 1.0), smoothstep(0.0, 1.0, v.local_pos.y));
        tint * (edge * waves * self.shader.glow * self.visibility)
    }
}
