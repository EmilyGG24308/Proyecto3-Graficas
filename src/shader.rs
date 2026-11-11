//! Interfaz de shaders del pipeline por software.
//!
//! El vertex stage está en `renderer.rs` (transformación MVP fija). Los
//! fragment shaders implementan el trait `Shader` y reciben los varyings
//! interpolados con corrección de perspectiva.

use crate::math::{Mat4, Vec2, Vec3};

/// Valores constantes durante un draw call ("uniforms" en OpenGL).
#[derive(Clone, Copy)]
pub struct Uniforms {
    /// Segundos desde el inicio del programa (para shaders animados).
    pub time: f32,
    pub camera_pos: Vec3,
    /// Posición del sol en mundo (única fuente de luz).
    pub sun_pos: Vec3,
    /// Matriz de modelo del objeto que se está dibujando.
    pub model: Mat4,
    /// Centro del objeto en mundo (útil para planetas).
    pub object_center: Vec3,
}

/// Datos por vértice que se interpolan a lo largo del triángulo.
#[derive(Clone, Copy, Default)]
pub struct Varyings {
    /// Posición en espacio de mundo.
    pub world_pos: Vec3,
    /// Normal en espacio de mundo (no necesariamente unitaria tras interpolar).
    pub normal: Vec3,
    /// Posición en espacio del objeto (sirve para ruido "pegado" al modelo).
    pub local_pos: Vec3,
    pub uv: Vec2,
    /// Color de vértice / material (Kd del .mtl, o blanco).
    pub color: Vec3,
    /// 1 = material emisivo (no se ilumina), 0 = normal.
    pub emissive: f32,
}

impl Varyings {
    /// Combinación lineal de tres varyings con pesos (baricéntricos).
    #[inline]
    pub fn blend3(a: &Varyings, b: &Varyings, c: &Varyings, w0: f32, w1: f32, w2: f32) -> Varyings {
        Varyings {
            world_pos: a.world_pos * w0 + b.world_pos * w1 + c.world_pos * w2,
            normal: a.normal * w0 + b.normal * w1 + c.normal * w2,
            local_pos: a.local_pos * w0 + b.local_pos * w1 + c.local_pos * w2,
            uv: a.uv * w0 + b.uv * w1 + c.uv * w2,
            color: a.color * w0 + b.color * w1 + c.color * w2,
            emissive: a.emissive * w0 + b.emissive * w1 + c.emissive * w2,
        }
    }

    /// Interpolación lineal entre dos varyings (usada al recortar).
    #[inline]
    pub fn lerp(a: &Varyings, b: &Varyings, t: f32) -> Varyings {
        Varyings::blend3(a, b, a, 1.0 - t, t, 0.0)
    }
}

/// Un fragment shader: calcula el color RGB (0..1) de un pixel.
pub trait Shader: Sync {
    fn fragment(&self, v: &Varyings, u: &Uniforms) -> Vec3;
}

// ---------------------------------------------------------------------------
// Utilidades de iluminación compartidas por los shaders
// ---------------------------------------------------------------------------

/// Componente difusa de Lambert hacia el sol, en [0, 1].
#[inline]
pub fn lambert(normal: Vec3, world_pos: Vec3, sun_pos: Vec3) -> f32 {
    let l = (sun_pos - world_pos).normalize();
    normal.dot(l).max(0.0)
}

/// Especular de Blinn-Phong.
#[inline]
pub fn blinn_phong(normal: Vec3, world_pos: Vec3, sun_pos: Vec3, cam: Vec3, shininess: f32) -> f32 {
    let l = (sun_pos - world_pos).normalize();
    let v = (cam - world_pos).normalize();
    let h = (l + v).normalize();
    if normal.dot(l) <= 0.0 {
        return 0.0;
    }
    normal.dot(h).max(0.0).powf(shininess)
}

/// Término de Fresnel/rim: 0 de frente, 1 en el borde de la silueta.
#[inline]
pub fn rim(normal: Vec3, world_pos: Vec3, cam: Vec3) -> f32 {
    let v = (cam - world_pos).normalize();
    1.0 - normal.dot(v).clamp(0.0, 1.0)
}
