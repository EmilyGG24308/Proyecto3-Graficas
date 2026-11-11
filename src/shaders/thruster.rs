//! Flama del propulsor (shader animado, mezcla aditiva).
//!
//! Se dibuja sobre un cono cuya punta está en z = 1 (espacio local).
//! - Gradiente a lo largo del cono: núcleo blanco-amarillo → naranja → rojo.
//! - Más intensa en el centro que en el borde (usa Fresnel invertido).
//! - Ruido que se desplaza con el tiempo hacia la punta: parpadeo de fuego.
//! - `intensity` (la potencia del motor) la apaga o la aviva.

use crate::math::{Vec3, smoothstep};
use crate::noise::Perlin;
use crate::shader::{Shader, Uniforms, Varyings, rim};

pub struct ThrusterShader {
    pub noise: Perlin,
}

/// Parámetros propios de la flama para este draw call (potencia del motor).
pub struct ThrusterParams {
    pub intensity: f32,
    pub boost: bool,
}

pub struct ThrusterDraw<'a> {
    pub shader: &'a ThrusterShader,
    pub params: ThrusterParams,
}

impl Shader for ThrusterDraw<'_> {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3 {
        let p = v.local_pos;
        let along = p.z.clamp(0.0, 1.0); // 0 en la base, 1 en la punta

        // Ruido que "fluye" hacia atrás.
        let flow = self.shader.noise.fbm(Vec3::new(p.x * 3.0, p.y * 3.0, p.z * 4.0 - u.time * 9.0), 3);
        let flicker = 0.75 + 0.5 * flow;

        let n = v.normal.normalize();
        let core = 1.0 - smoothstep(0.2, 0.95, rim(n, v.world_pos, u.camera_pos));

        let hot = if self.params.boost { Vec3::new(0.75, 0.9, 1.0) } else { Vec3::new(1.0, 0.95, 0.7) };
        let mid = if self.params.boost { Vec3::new(0.3, 0.55, 1.0) } else { Vec3::new(1.0, 0.55, 0.12) };
        let tail = Vec3::new(0.8, 0.15, 0.05);
        let col = if along < 0.4 {
            hot.lerp(mid, along / 0.4)
        } else {
            mid.lerp(tail, (along - 0.4) / 0.6)
        };

        let fade = (1.0 - along).powf(1.5);
        col * (1.6 * fade * flicker * (0.45 + 0.55 * core) * self.params.intensity)
    }
}
