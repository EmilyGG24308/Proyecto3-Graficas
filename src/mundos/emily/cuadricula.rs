//! Cuadrícula de calles sobre la esfera (proyección de cubo equiangular).
//! La usan el shader de la ciudad y la colocación de edificios.

use std::f32::consts::FRAC_PI_4;

use crate::math::Vec3;

/// Las 6 caras de un cubo: (normal, eje u, eje v).
pub(super) const CARAS: [(Vec3, Vec3, Vec3); 6] = [
    (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 1.0, 0.0)),
    (Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 1.0, 0.0)),
    (Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0)),
    (Vec3::new(0.0, -1.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
    (Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
    (Vec3::new(0.0, 0.0, -1.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)),
];

/// Dirección (unitaria) -> (cara, gu, gv) con gu, gv en [0, n): la parte
/// entera es la manzana y la fraccionaria la posición dentro de ella.
///
/// Por qué un cubo: con latitud/longitud las calles se amontonarían en los
/// polos. Proyectando la esfera sobre las 6 caras de un cubo cada cara es
/// un cuadrado normal. El `atan` ("equiangular") corrige que las manzanas
/// cerca de las esquinas del cubo salgan más chicas que las del centro.
pub(super) fn cuadricula(d: Vec3, n: f32) -> (usize, f32, f32) {
    let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
    let cara = if ax >= ay && ax >= az {
        if d.x > 0.0 { 0 } else { 1 }
    } else if ay >= az {
        if d.y > 0.0 { 2 } else { 3 }
    } else if d.z > 0.0 {
        4
    } else {
        5
    };
    let (normal, eu, ev) = CARAS[cara];
    let k = d.dot(normal);
    let u = (d.dot(eu) / k).atan() / FRAC_PI_4;
    let v = (d.dot(ev) / k).atan() / FRAC_PI_4;
    let a_celda = |x: f32| ((x * 0.5 + 0.5) * n).clamp(0.0, n - 1e-4);
    (cara, a_celda(u), a_celda(v))
}

/// Inversa de `cuadricula`: de coordenadas de la cuadrícula a dirección.
pub(super) fn direccion_cuadricula(cara: usize, gu: f32, gv: f32, n: f32) -> Vec3 {
    let (normal, eu, ev) = CARAS[cara];
    let tu = ((gu / n * 2.0 - 1.0) * FRAC_PI_4).tan();
    let tv = ((gv / n * 2.0 - 1.0) * FRAC_PI_4).tan();
    (normal + eu * tu + ev * tv).normalize()
}

/// Qué hay en una manzana. Lo decide su hash, igual en el shader y en la CPU.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Manzana {
    Parque,
    Plaza,
    Edificios,
}

pub(super) fn tipo_manzana(hash: f32, parques: f32) -> Manzana {
    if hash < parques {
        Manzana::Parque
    } else if hash < parques + 0.08 {
        Manzana::Plaza
    } else {
        Manzana::Edificios
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cuadricula_ida_y_vuelta() {
        // Del centro de una manzana a dirección y de regreso: misma manzana.
        let n = 7.0;
        for cara in 0..6 {
            for (i, j) in [(0, 0), (3, 5), (6, 6)] {
                let d = direccion_cuadricula(cara, i as f32 + 0.5, j as f32 + 0.5, n);
                let (c, gu, gv) = cuadricula(d, n);
                assert_eq!((c, gu.floor() as i32, gv.floor() as i32), (cara, i, j));
                assert!((gu.fract() - 0.5).abs() < 1e-3 && (gv.fract() - 0.5).abs() < 1e-3);
            }
        }
    }
}
