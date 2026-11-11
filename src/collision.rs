//! Colisión básica: la nave (una esfera pequeña) contra cuerpos esféricos
//! (planetas y sol) que pueden estar moviéndose en su órbita.

use crate::math::Vec3;

/// Un cuerpo sólido con el que la nave puede chocar.
#[derive(Clone, Copy, Debug)]
pub struct Collider {
    pub center: Vec3,
    /// Velocidad del cuerpo (los planetas orbitan): el rebote se calcula
    /// relativo a ella para que la nave no se "hunda" en un planeta que la
    /// alcanza por detrás.
    pub velocity: Vec3,
    /// Radio de la superficie sólida.
    pub radius: f32,
    /// Nombre para el HUD.
    pub name: &'static str,
}

/// Resultado de resolver una colisión en un paso.
#[derive(Clone, Copy, Debug)]
pub struct Contact {
    /// Rapidez con la que se golpeó la superficie (para efectos / HUD).
    pub impact_speed: f32,
}

impl Collider {
    /// Distancia de un punto a la superficie (negativa si está dentro).
    pub fn signed_distance(&self, p: Vec3) -> f32 {
        (p - self.center).length() - self.radius
    }

    /// Empuja una esfera de radio `r` (centro `pos`, velocidad `vel`) fuera
    /// del cuerpo si lo está atravesando y corrige su velocidad:
    ///  - se elimina la componente que va hacia dentro (con un rebote leve),
    ///  - se aplica fricción a la componente tangencial (roza la superficie).
    pub fn resolve(&self, pos: &mut Vec3, vel: &mut Vec3, r: f32, restitution: f32, friction: f32) -> Option<Contact> {
        let d = *pos - self.center;
        let dist = d.length();
        let min = self.radius + r;
        if dist >= min {
            return None;
        }
        // Normal de la superficie (si justo está en el centro, empujar hacia arriba).
        let n = if dist > 1e-6 { d / dist } else { Vec3::Y };
        *pos = self.center + n * min;

        let rel = *vel - self.velocity;
        let vn = rel.dot(n);
        if vn >= 0.0 {
            // Ya se está alejando: solo se corrigió la posición.
            return Some(Contact { impact_speed: 0.0 });
        }
        let normal = n * vn;
        let tangent = rel - normal;
        let new_rel = tangent * (1.0 - friction) - normal * restitution;
        *vel = new_rel + self.velocity;
        Some(Contact { impact_speed: -vn })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushes_out_and_bounces() {
        let c = Collider { center: Vec3::ZERO, velocity: Vec3::ZERO, radius: 5.0, name: "p" };
        let mut pos = Vec3::new(0.0, 0.0, 5.1);
        let mut vel = Vec3::new(1.0, 0.0, -10.0);
        let hit = c.resolve(&mut pos, &mut vel, 0.5, 0.2, 0.1).unwrap();
        assert!((pos.length() - 5.5).abs() < 1e-5);
        assert!((hit.impact_speed - 10.0).abs() < 1e-5);
        assert!(vel.z > 0.0, "debe rebotar hacia fuera");
        assert!((vel.x - 0.9).abs() < 1e-5, "fricción tangencial");
    }

    #[test]
    fn moving_planet_carries_ship() {
        // El planeta viene hacia la nave a 3 u/s; la nave está quieta.
        let c = Collider { center: Vec3::ZERO, velocity: Vec3::new(0.0, 0.0, 3.0), radius: 5.0, name: "p" };
        let mut pos = Vec3::new(0.0, 0.0, 5.2);
        let mut vel = Vec3::ZERO;
        c.resolve(&mut pos, &mut vel, 0.5, 0.0, 0.0);
        assert!((vel.z - 3.0).abs() < 1e-5, "la nave sale con la velocidad del planeta");
    }
}
