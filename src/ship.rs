//! Nave: estado, física de vuelo y matriz de modelo.
//!
//! La física corre con un paso de tiempo FIJO (`PHYSICS_DT`). Cada cuadro se
//! acumula el tiempo real transcurrido y se ejecutan tantos pasos fijos como
//! quepan; el sobrante se usa para interpolar entre el estado anterior y el
//! actual al dibujar. Así la nave acelera, frena y gira exactamente igual en
//! una computadora a 30 FPS que en otra a 144 FPS.

use crate::collision::Collider;
use crate::config::*;
use crate::math::{Mat4, Vec3};

/// Lo que pide quien juega en este instante. Ejes en [-1, 1].
#[derive(Clone, Copy, Default)]
pub struct ShipInput {
    /// +1 acelerar (W), -1 frenar / reversa (S).
    pub thrust: f32,
    /// Empuje vertical (Espacio / Ctrl).
    pub lift: f32,
    pub boost: bool,
    /// Cabeceo: +1 sube la nariz.
    pub pitch: f32,
    /// Guiñada: +1 gira a la derecha.
    pub yaw: f32,
    /// Alabeo: +1 rueda a la derecha.
    pub roll: f32,
}

/// Estado físico instantáneo. Se guarda el anterior para interpolar.
#[derive(Clone, Copy)]
pub struct ShipState {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Base ortonormal de la nave (hacia dónde apunta la nariz y el techo).
    pub forward: Vec3,
    pub up: Vec3,
}

impl ShipState {
    pub fn right(&self) -> Vec3 {
        self.forward.cross(self.up).normalize()
    }

    /// Mezcla dos estados (para dibujar entre dos pasos de física).
    fn interpolate(&self, o: &ShipState, t: f32) -> ShipState {
        let forward = self.forward.lerp(o.forward, t).normalize();
        let up = self.up.lerp(o.up, t);
        let right = forward.cross(up).normalize();
        ShipState {
            pos: self.pos.lerp(o.pos, t),
            vel: self.vel.lerp(o.vel, t),
            forward,
            up: right.cross(forward),
        }
    }
}

pub struct Ship {
    prev: ShipState,
    pub state: ShipState,
    /// Velocidad angular actual (rad/s) en ejes locales: (cabeceo, guiñada, alabeo).
    ang_vel: Vec3,
    /// Potencia del motor suavizada en [0, 1] (para la flama).
    pub throttle: f32,
    pub boosting: bool,
    /// Segundos que quedan mostrando el aviso de contacto en el HUD.
    pub contact_timer: f32,
    /// Con qué cuerpo fue el último contacto.
    pub contact_with: &'static str,
    accumulator: f32,
}

impl Ship {
    pub fn new(pos: Vec3, forward: Vec3) -> Self {
        let forward = forward.normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let state = ShipState { pos, vel: Vec3::ZERO, forward, up: right.cross(forward) };
        Self {
            prev: state,
            state,
            ang_vel: Vec3::ZERO,
            throttle: 0.0,
            boosting: false,
            contact_timer: 0.0,
            contact_with: "",
            accumulator: 0.0,
        }
    }

    /// Avanza la simulación `dt` segundos reales con pasos fijos.
    /// `colliders` son los cuerpos sólidos en este instante.
    pub fn update(&mut self, input: &ShipInput, dt: f32, colliders: &[Collider]) {
        self.contact_timer = (self.contact_timer - dt).max(0.0);
        self.accumulator += dt;
        while self.accumulator >= PHYSICS_DT {
            self.prev = self.state;
            self.step(input, PHYSICS_DT, colliders);
            self.accumulator -= PHYSICS_DT;
        }
    }

    /// Estado listo para dibujar (interpolado entre los dos últimos pasos).
    pub fn render_state(&self) -> ShipState {
        self.prev.interpolate(&self.state, self.accumulator / PHYSICS_DT)
    }

    pub fn speed(&self) -> f32 {
        self.state.vel.length()
    }

    /// Un paso de física de duración fija `dt`.
    fn step(&mut self, input: &ShipInput, dt: f32, colliders: &[Collider]) {
        // --- Giro progresivo -------------------------------------------------
        // La velocidad angular se acerca a la deseada de forma exponencial:
        // no gira de golpe ni se detiene de golpe.
        let target = Vec3::new(
            input.pitch * SHIP_PITCH_RATE,
            input.yaw * SHIP_YAW_RATE,
            input.roll * SHIP_ROLL_RATE,
        );
        let k = 1.0 - (-SHIP_TURN_RESPONSE * dt).exp();
        self.ang_vel = self.ang_vel.lerp(target, k);

        let s = &mut self.state;
        let right = s.right();
        // Cabeceo: alrededor del eje derecho. Guiñada: alrededor del techo.
        // Alabeo: alrededor de la nariz.
        let (pitch, yaw, roll) = (self.ang_vel.x * dt, self.ang_vel.y * dt, self.ang_vel.z * dt);
        s.forward = s.forward.rotate_around(right, pitch);
        s.up = s.up.rotate_around(right, pitch);
        s.forward = s.forward.rotate_around(s.up, -yaw);
        s.up = s.up.rotate_around(s.forward, roll);
        // Re-ortonormalizar para que los errores numéricos no se acumulen.
        s.forward = s.forward.normalize();
        let right = s.forward.cross(s.up).normalize();
        s.up = right.cross(s.forward);

        // --- Aceleración y frenado progresivos -------------------------------
        self.boosting = input.boost && input.thrust > 0.0;
        let boost = if self.boosting { SHIP_BOOST_MULT } else { 1.0 };
        let mut accel = Vec3::ZERO;
        if input.thrust > 0.0 {
            accel += s.forward * (SHIP_THRUST * boost * input.thrust);
        } else if input.thrust < 0.0 {
            // Reversa suave (útil para retroceder cerca de un planeta).
            accel += s.forward * (SHIP_REVERSE * input.thrust);
        }
        accel += s.up * (SHIP_LIFT * input.lift);

        s.vel += accel * dt;

        // Rozamiento exponencial: define una velocidad máxima "natural"
        // (thrust / drag) y hace que la nave se detenga sola poco a poco.
        // Al frenar (S) el rozamiento es mucho mayor.
        let drag = if input.thrust < 0.0 { SHIP_BRAKE_DRAG } else { SHIP_DRAG };
        s.vel = s.vel * (-drag * dt).exp();

        s.pos += s.vel * dt;

        // --- Colisión: la nave no puede atravesar planetas ni el sol ---------
        for c in colliders {
            if let Some(hit) = c.resolve(&mut s.pos, &mut s.vel, SHIP_COLLISION_RADIUS, SHIP_RESTITUTION, SHIP_FRICTION) {
                self.contact_with = c.name;
                if hit.impact_speed > 0.5 || self.contact_timer > 0.0 {
                    self.contact_timer = 1.0;
                }
            }
        }

        // Potencia visual del motor (flama), también suavizada.
        let target_thr = (input.thrust.max(0.0) * boost / SHIP_BOOST_MULT).max(input.lift.abs() * 0.3);
        self.throttle += (target_thr - self.throttle) * (1.0 - (-8.0 * dt).exp());
    }
}

/// Matriz de modelo de la nave. El modelo .obj mira hacia -Z local, así que
/// el eje Z local corresponde a "atrás" (-forward) en mundo.
pub fn model_matrix(s: &ShipState) -> Mat4 {
    let right = s.right();
    Mat4::translation(s.pos) * Mat4::from_basis(right, s.up, -s.forward) * Mat4::uniform_scale(SHIP_SCALE)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simula `secs` segundos con cuadros de duración `frame_dt` y la misma
    /// secuencia de controles (acelerar + girar, luego frenar).
    fn fly(frame_dt: f32, secs: f32) -> ShipState {
        let mut ship = Ship::new(Vec3::ZERO, -Vec3::Z);
        // Se cuentan cuadros enteros: acumular el tiempo en f32 haría que
        // cada framerate terminara en un instante ligeramente distinto.
        let frames = (secs / frame_dt).round() as u32;
        for i in 0..frames {
            let t = i as f32 * frame_dt;
            let input = if t < secs * 0.6 {
                ShipInput { thrust: 1.0, yaw: 0.5, pitch: 0.2, roll: 0.3, ..Default::default() }
            } else {
                ShipInput { thrust: -1.0, ..Default::default() }
            };
            ship.update(&input, frame_dt, &[]);
        }
        ship.state
    }

    #[test]
    fn same_result_at_30_and_144_fps() {
        // 3 s es múltiplo exacto de 1/30, 1/60, 1/120 y casi de 1/144.
        let a = fly(1.0 / 30.0, 3.0);
        let b = fly(1.0 / 144.0, 3.0);
        let c = fly(1.0 / 60.0, 3.0);
        // A lo sumo un paso de física de diferencia (el cambio de controles
        // cae dentro de un cuadro distinto según el framerate).
        assert!((a.pos - b.pos).length() < 0.05, "{:?} vs {:?}", a.pos, b.pos);
        assert!((a.pos - c.pos).length() < 1e-3);
        assert!((a.forward - c.forward).length() < 1e-4);
    }

    #[test]
    fn basis_stays_orthonormal() {
        let s = fly(1.0 / 60.0, 20.0);
        assert!((s.forward.length() - 1.0).abs() < 1e-4);
        assert!((s.up.length() - 1.0).abs() < 1e-4);
        assert!(s.forward.dot(s.up).abs() < 1e-4);
    }

    #[test]
    fn cannot_fly_through_planet() {
        use crate::collision::Collider;
        let planet = Collider::sphere(Vec3::new(0.0, 0.0, -30.0), Vec3::ZERO, 7.0, "p");
        let mut ship = Ship::new(Vec3::ZERO, -Vec3::Z);
        // Turbo a fondo directo hacia el planeta durante 5 s.
        let input = ShipInput { thrust: 1.0, boost: true, ..Default::default() };
        for _ in 0..300 {
            ship.update(&input, 1.0 / 60.0, &[planet]);
            let d = planet.signed_distance(ship.state.pos);
            assert!(d >= SHIP_COLLISION_RADIUS - 1e-3, "atravesó la superficie: {d}");
        }
        assert_eq!(ship.contact_with, "p");
    }
}
