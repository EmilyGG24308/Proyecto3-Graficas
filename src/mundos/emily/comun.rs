//! Utilidades que comparten la Ciudad y el Hielo.

use crate::math::{Mat4, Vec3, smoothstep};
use crate::mesh::Mesh;
use crate::planet::Planet;
use crate::props::{BakedProps, append_transformed};
use crate::shader::{Uniforms, Varyings, lambert};

/// Noche en [0, 1] según la normal GEOMÉTRICA de la esfera (no la del
/// relieve): así el límite día/noche es una línea limpia y las luces de la
/// ciudad o la aurora no parpadean con cada lomita.
pub(super) fn noche(v: &Varyings, u: &Uniforms) -> f32 {
    let arriba = (v.world_pos - u.object_center).normalize();
    1.0 - smoothstep(0.0, 0.12, lambert(arriba, v.world_pos, u.sun_pos))
}

/// Hash entero de una celda (cara, i, j) a [0, 1). Es exacto (no usa senos),
/// así la CPU y el shader obtienen el mismo número para la misma manzana.
pub(super) fn hash_celda(cara: usize, i: i32, j: i32, semilla: u32) -> f32 {
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
pub(super) fn sobre_suelo(planeta: &Planet, dir: Vec3, eje: Vec3, giro: f32, escala: Vec3, hundir: f32) -> Mat4 {
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
pub(super) fn hornear(planeta: &mut Planet, grupos: Vec<(&'static str, &Mesh, Vec<Mat4>)>) {
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

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn hash_de_manzana_es_reproducible() {
        assert_eq!(hash_celda(2, 3, 4, 77), hash_celda(2, 3, 4, 77));
        assert_ne!(hash_celda(2, 3, 4, 77), hash_celda(2, 3, 4, 78));
        let h = hash_celda(5, 6, 1, 9);
        assert!((0.0..1.0).contains(&h));
    }
}
