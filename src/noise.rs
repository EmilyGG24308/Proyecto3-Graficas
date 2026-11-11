//! Ruido procedural propio: Perlin 3D con semilla, fBm, ridged y hash.
//!
//! Con la misma semilla se obtiene siempre el mismo resultado. Eso es lo que
//! hace reproducible la distribución de objetos y la forma de los planetas.

use crate::math::{Vec3, lerp};

/// Generador pseudoaleatorio pequeño y determinista (SplitMix64).
#[derive(Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15) }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Flotante uniforme en [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        lerp(a, b, self.next_f32())
    }
}

/// Ruido de Perlin 3D (gradientes) con tabla de permutación sembrada.
#[derive(Clone)]
pub struct Perlin {
    perm: [u8; 512],
}

impl Perlin {
    pub fn new(seed: u64) -> Self {
        let mut p: [u8; 256] = std::array::from_fn(|i| i as u8);
        // Barajado de Fisher-Yates con la semilla.
        let mut rng = Rng::new(seed);
        for i in (1..256).rev() {
            let j = (rng.next_u64() % (i as u64 + 1)) as usize;
            p.swap(i, j);
        }
        let mut perm = [0u8; 512];
        for i in 0..512 {
            perm[i] = p[i & 255];
        }
        Self { perm }
    }

    fn fade(t: f32) -> f32 {
        // 6t^5 - 15t^4 + 10t^3 (curva de Perlin mejorada)
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    fn grad(hash: u8, x: f32, y: f32, z: f32) -> f32 {
        // 12 direcciones hacia las aristas de un cubo.
        let h = hash & 15;
        let u = if h < 8 { x } else { y };
        let v = if h < 4 {
            y
        } else if h == 12 || h == 14 {
            x
        } else {
            z
        };
        (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
    }

    /// Ruido en [-1, 1] aproximadamente.
    pub fn noise(&self, p: Vec3) -> f32 {
        let xf = p.x.floor();
        let yf = p.y.floor();
        let zf = p.z.floor();
        let xi = (xf as i32 & 255) as usize;
        let yi = (yf as i32 & 255) as usize;
        let zi = (zf as i32 & 255) as usize;
        let x = p.x - xf;
        let y = p.y - yf;
        let z = p.z - zf;
        let u = Self::fade(x);
        let v = Self::fade(y);
        let w = Self::fade(z);

        let pm = &self.perm;
        let a = pm[xi] as usize + yi;
        let aa = pm[a] as usize + zi;
        let ab = pm[a + 1] as usize + zi;
        let b = pm[xi + 1] as usize + yi;
        let ba = pm[b] as usize + zi;
        let bb = pm[b + 1] as usize + zi;

        lerp(
            lerp(
                lerp(Self::grad(pm[aa], x, y, z), Self::grad(pm[ba], x - 1.0, y, z), u),
                lerp(Self::grad(pm[ab], x, y - 1.0, z), Self::grad(pm[bb], x - 1.0, y - 1.0, z), u),
                v,
            ),
            lerp(
                lerp(
                    Self::grad(pm[aa + 1], x, y, z - 1.0),
                    Self::grad(pm[ba + 1], x - 1.0, y, z - 1.0),
                    u,
                ),
                lerp(
                    Self::grad(pm[ab + 1], x, y - 1.0, z - 1.0),
                    Self::grad(pm[bb + 1], x - 1.0, y - 1.0, z - 1.0),
                    u,
                ),
                v,
            ),
            w,
        )
    }

    /// Fractal Brownian Motion: suma de octavas con frecuencia creciente
    /// y amplitud decreciente. Resultado aproximadamente en [-1, 1].
    pub fn fbm(&self, p: Vec3, octaves: u32) -> f32 {
        let mut sum = 0.0;
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            sum += amp * self.noise(p * freq);
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / norm
    }

    /// Ruido "ridged": crestas afiladas, útil para montañas y grietas.
    /// Resultado en [0, 1].
    pub fn ridged(&self, p: Vec3, octaves: u32) -> f32 {
        let mut sum = 0.0;
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            let n = 1.0 - self.noise(p * freq).abs();
            sum += amp * n * n;
            norm += amp;
            amp *= 0.5;
            freq *= 2.0;
        }
        sum / norm
    }
}

/// Hash rápido de un punto 3D a [0, 1). Útil para variación por celda.
pub fn hash3(p: Vec3) -> f32 {
    let h = (p.x * 127.1 + p.y * 311.7 + p.z * 74.7).sin() * 43_758.547;
    h - h.floor()
}

/// Resultado del ruido celular (Worley / Voronoi).
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    /// Distancia al punto de celda más cercano.
    pub f1: f32,
    /// Distancia al segundo más cercano (f2 - f1 ≈ 0 en los bordes).
    pub f2: f32,
    /// Valor aleatorio fijo de la celda más cercana, en [0, 1).
    pub id: f32,
}

/// Ruido celular 3D: el espacio se divide en cubos, cada cubo tiene un punto
/// en una posición aleatoria (fija), y se mide la distancia a los puntos de
/// los 27 cubos vecinos. Sirve para copas de árboles, piedras, escamas…
pub fn worley(p: Vec3) -> Cell {
    let base = Vec3::new(p.x.floor(), p.y.floor(), p.z.floor());
    let mut f1 = f32::MAX;
    let mut f2 = f32::MAX;
    let mut id = 0.0;
    for dx in -1..=1 {
        for dy in -1..=1 {
            for dz in -1..=1 {
                let cell = base + Vec3::new(dx as f32, dy as f32, dz as f32);
                let jitter = Vec3::new(
                    hash3(cell),
                    hash3(cell + Vec3::new(17.31, 3.7, 9.1)),
                    hash3(cell + Vec3::new(5.17, 41.3, 23.9)),
                );
                let d = (cell + jitter - p).length();
                if d < f1 {
                    f2 = f1;
                    f1 = d;
                    id = hash3(cell + Vec3::new(71.7, 11.3, 57.1));
                } else if d < f2 {
                    f2 = d;
                }
            }
        }
    }
    Cell { f1, f2, id }
}
