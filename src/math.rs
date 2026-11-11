//! Matemática propia del renderer: vectores y matrices 4x4.
//!
//! Convenciones:
//! - Sistema de mano derecha, Y hacia arriba, la cámara mira hacia -Z.
//! - Matrices en orden de filas (`m[fila][columna]`) que multiplican vectores
//!   columna: `v' = M * v`. Por eso una cadena de transformaciones se lee de
//!   derecha a izquierda: `P * V * M * v`.
//! - Clip space estilo OpenGL: tras dividir por w, x, y, z quedan en [-1, 1].

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

// ---------------------------------------------------------------------------
// Vec2
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}

// ---------------------------------------------------------------------------
// Vec3
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    pub const ONE: Vec3 = Vec3::new(1.0, 1.0, 1.0);
    pub const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
    pub const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    pub const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub const fn splat(v: f32) -> Self {
        Self::new(v, v, v)
    }

    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length_sq(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_sq().sqrt()
    }

    /// Devuelve el vector unitario; si la longitud es ~0 devuelve cero.
    pub fn normalize(self) -> Vec3 {
        let len = self.length();
        if len > 1e-8 { self / len } else { Vec3::ZERO }
    }

    /// Producto componente a componente (útil para mezclar colores).
    pub fn hadamard(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }

    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        self + (o - self) * t
    }

    pub fn max_elem(self) -> f32 {
        self.x.max(self.y).max(self.z)
    }

    pub fn clamp01(self) -> Vec3 {
        Vec3::new(
            self.x.clamp(0.0, 1.0),
            self.y.clamp(0.0, 1.0),
            self.z.clamp(0.0, 1.0),
        )
    }

    /// Rota `self` un ángulo `angle` alrededor del eje unitario `axis`
    /// (fórmula de Rodrigues).
    pub fn rotate_around(self, axis: Vec3, angle: f32) -> Vec3 {
        let (s, c) = angle.sin_cos();
        self * c + axis.cross(self) * s + axis * (axis.dot(self) * (1.0 - c))
    }

    /// Refleja `self` (dirección entrante) respecto a la normal `n`.
    pub fn reflect(self, n: Vec3) -> Vec3 {
        self - n * (2.0 * self.dot(n))
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        self.x += o.x;
        self.y += o.y;
        self.z += o.z;
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Mul<Vec3> for f32 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        v * self
    }
}

impl Div<f32> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f32) -> Vec3 {
        let inv = 1.0 / s;
        Vec3::new(self.x * inv, self.y * inv, self.z * inv)
    }
}

// ---------------------------------------------------------------------------
// Vec4 (coordenadas homogéneas)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn from_point(p: Vec3) -> Self {
        Self::new(p.x, p.y, p.z, 1.0)
    }

    pub fn xyz(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}

impl Add for Vec4 {
    type Output = Vec4;
    fn add(self, o: Vec4) -> Vec4 {
        Vec4::new(self.x + o.x, self.y + o.y, self.z + o.z, self.w + o.w)
    }
}

impl Sub for Vec4 {
    type Output = Vec4;
    fn sub(self, o: Vec4) -> Vec4 {
        Vec4::new(self.x - o.x, self.y - o.y, self.z - o.z, self.w - o.w)
    }
}

impl Mul<f32> for Vec4 {
    type Output = Vec4;
    fn mul(self, s: f32) -> Vec4 {
        Vec4::new(self.x * s, self.y * s, self.z * s, self.w * s)
    }
}

// ---------------------------------------------------------------------------
// Mat4
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub m: [[f32; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4 {
        m: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub fn translation(t: Vec3) -> Mat4 {
        let mut r = Mat4::IDENTITY;
        r.m[0][3] = t.x;
        r.m[1][3] = t.y;
        r.m[2][3] = t.z;
        r
    }

    pub fn scale(s: Vec3) -> Mat4 {
        let mut r = Mat4::IDENTITY;
        r.m[0][0] = s.x;
        r.m[1][1] = s.y;
        r.m[2][2] = s.z;
        r
    }

    pub fn uniform_scale(s: f32) -> Mat4 {
        Mat4::scale(Vec3::splat(s))
    }

    pub fn rotation_x(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        let mut r = Mat4::IDENTITY;
        r.m[1][1] = c;
        r.m[1][2] = -s;
        r.m[2][1] = s;
        r.m[2][2] = c;
        r
    }

    pub fn rotation_y(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        let mut r = Mat4::IDENTITY;
        r.m[0][0] = c;
        r.m[0][2] = s;
        r.m[2][0] = -s;
        r.m[2][2] = c;
        r
    }

    pub fn rotation_z(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        let mut r = Mat4::IDENTITY;
        r.m[0][0] = c;
        r.m[0][1] = -s;
        r.m[1][0] = s;
        r.m[1][1] = c;
        r
    }

    /// Construye una matriz de rotación a partir de una base ortonormal
    /// (las columnas son los ejes locales X, Y, Z expresados en mundo).
    pub fn from_basis(right: Vec3, up: Vec3, forward_z: Vec3) -> Mat4 {
        Mat4 {
            m: [
                [right.x, up.x, forward_z.x, 0.0],
                [right.y, up.y, forward_z.y, 0.0],
                [right.z, up.z, forward_z.z, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Matriz de vista: lleva el mundo al espacio de la cámara.
    /// La cámara queda en el origen mirando hacia -Z.
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
        let f = (target - eye).normalize(); // hacia dónde mira
        let r = f.cross(up).normalize(); // derecha
        let u = r.cross(f); // arriba real (ortogonal)
        Mat4 {
            m: [
                [r.x, r.y, r.z, -r.dot(eye)],
                [u.x, u.y, u.z, -u.dot(eye)],
                [-f.x, -f.y, -f.z, f.dot(eye)],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Proyección en perspectiva (estilo OpenGL).
    /// `fov_y` en radianes. Tras dividir por w, z_ndc está en [-1, 1]
    /// con -1 en el plano cercano y 1 en el lejano.
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fov_y * 0.5).tan();
        let nf = 1.0 / (near - far);
        Mat4 {
            m: [
                [f / aspect, 0.0, 0.0, 0.0],
                [0.0, f, 0.0, 0.0],
                [0.0, 0.0, (far + near) * nf, 2.0 * far * near * nf],
                [0.0, 0.0, -1.0, 0.0],
            ],
        }
    }

    pub fn mul_vec4(&self, v: Vec4) -> Vec4 {
        let m = &self.m;
        Vec4::new(
            m[0][0] * v.x + m[0][1] * v.y + m[0][2] * v.z + m[0][3] * v.w,
            m[1][0] * v.x + m[1][1] * v.y + m[1][2] * v.z + m[1][3] * v.w,
            m[2][0] * v.x + m[2][1] * v.y + m[2][2] * v.z + m[2][3] * v.w,
            m[3][0] * v.x + m[3][1] * v.y + m[3][2] * v.z + m[3][3] * v.w,
        )
    }

    /// Transforma un punto (w = 1): aplica rotación, escala y traslación.
    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        self.mul_vec4(Vec4::from_point(p)).xyz()
    }

    /// Transforma una dirección (w = 0): ignora la traslación.
    pub fn transform_dir(&self, d: Vec3) -> Vec3 {
        let m = &self.m;
        Vec3::new(
            m[0][0] * d.x + m[0][1] * d.y + m[0][2] * d.z,
            m[1][0] * d.x + m[1][1] * d.y + m[1][2] * d.z,
            m[2][0] * d.x + m[2][1] * d.y + m[2][2] * d.z,
        )
    }

    /// Posición (columna de traslación) de la matriz.
    pub fn translation_part(&self) -> Vec3 {
        Vec3::new(self.m[0][3], self.m[1][3], self.m[2][3])
    }
}

impl Mul for Mat4 {
    type Output = Mat4;
    fn mul(self, o: Mat4) -> Mat4 {
        let mut r = [[0.0f32; 4]; 4];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (0..4).map(|k| self.m[i][k] * o.m[k][j]).sum();
            }
        }
        Mat4 { m: r }
    }
}

// ---------------------------------------------------------------------------
// Utilidades escalares
// ---------------------------------------------------------------------------

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn fract(x: f32) -> f32 {
    x - x.floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn look_at_puts_target_on_negative_z() {
        let v = Mat4::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO, Vec3::Y);
        let p = v.transform_point(Vec3::ZERO);
        assert!(approx(p, Vec3::new(0.0, 0.0, -5.0)));
    }

    #[test]
    fn perspective_maps_near_far_to_ndc() {
        let p = Mat4::perspective(1.0, 1.0, 0.5, 100.0);
        let near = p.mul_vec4(Vec4::new(0.0, 0.0, -0.5, 1.0));
        let far = p.mul_vec4(Vec4::new(0.0, 0.0, -100.0, 1.0));
        assert!((near.z / near.w + 1.0).abs() < 1e-4);
        assert!((far.z / far.w - 1.0).abs() < 1e-3);
    }

    #[test]
    fn rotation_y_quarter_turn() {
        let r = Mat4::rotation_y(std::f32::consts::FRAC_PI_2);
        assert!(approx(r.transform_dir(Vec3::X), Vec3::new(0.0, 0.0, -1.0)));
    }
}
