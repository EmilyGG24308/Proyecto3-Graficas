//! Parámetros ajustables del proyecto. Para la demo se cambian aquí
//! (radios, velocidades, semillas, intensidades) y se recompila.

/// Resolución interna del framebuffer (se escala a la ventana).
pub const RENDER_W: usize = 480;
pub const RENDER_H: usize = 270;

/// Tamaño inicial de la ventana.
pub const WINDOW_W: i32 = 1440;
pub const WINDOW_H: i32 = 810;

/// Cámara.
pub const FOV_Y_DEG: f32 = 65.0;
pub const NEAR: f32 = 0.05;
pub const FAR: f32 = 600.0;

/// Semilla global del sistema (forma de planetas, distribución, estrellas).
pub const WORLD_SEED: u64 = 2026;

/// Sol.
pub const SUN_RADIUS: f32 = 12.0;
/// Velocidad de la animación de la superficie del sol.
pub const SUN_ANIM_SPEED: f32 = 0.35;

/// Niveles de detalle de los planetas: subdivisiones de la icosfera de cada
/// nivel (320, 1280, 5120 y 20480 triángulos).
pub const LOD_SUBDIVISIONS: [u32; 4] = [2, 3, 4, 5];
/// Radio en pantalla (pixeles) a partir del cual se usa el siguiente nivel.
pub const LOD_PIXEL_THRESHOLDS: [f32; 3] = [18.0, 55.0, 140.0];

// Los parámetros de cada planeta (radio, órbita, semilla…) están en
// `src/planets.rs`.

/// Cámara libre de depuración (tecla F2).
pub const FREEFLY_SPEED: f32 = 8.0;
pub const MOUSE_SENS: f32 = 0.0025;

// --- Nave -------------------------------------------------------------------

/// Escala del modelo (el .obj mide ~2 unidades de largo).
pub const SHIP_SCALE: f32 = 0.5;
/// Paso fijo de la física: 120 pasos por segundo sin importar los FPS.
pub const PHYSICS_DT: f32 = 1.0 / 120.0;
/// Aceleración del motor (unidades/s²). Velocidad máxima ≈ THRUST / DRAG.
pub const SHIP_THRUST: f32 = 12.0;
pub const SHIP_REVERSE: f32 = 4.0;
pub const SHIP_LIFT: f32 = 5.0;
pub const SHIP_BOOST_MULT: f32 = 3.5;
/// Rozamiento normal (1/s) y al frenar con S.
pub const SHIP_DRAG: f32 = 0.45;
pub const SHIP_BRAKE_DRAG: f32 = 2.2;
/// Velocidades de giro máximas (rad/s).
pub const SHIP_PITCH_RATE: f32 = 1.6;
pub const SHIP_YAW_RATE: f32 = 1.2;
pub const SHIP_ROLL_RATE: f32 = 2.6;
/// Qué tan rápido la velocidad angular alcanza la deseada (1/s).
pub const SHIP_TURN_RESPONSE: f32 = 5.0;

/// Radio de la esfera de colisión de la nave (en mundo).
pub const SHIP_COLLISION_RADIUS: f32 = 0.5;
/// Rebote al chocar (0 = nada, 1 = rebote perfecto) y fricción con la superficie.
pub const SHIP_RESTITUTION: f32 = 0.25;
pub const SHIP_FRICTION: f32 = 0.02;
/// La cámara se mantiene al menos a esta distancia de las superficies.
pub const CAM_SURFACE_MARGIN: f32 = 0.35;

/// Mouse como palanca virtual: el movimiento del mouse desplaza la palanca,
/// que vuelve sola al centro con esta rapidez (1/s).
pub const MOUSE_STICK_SENS: f32 = 0.006;
pub const MOUSE_STICK_RECENTER: f32 = 1.8;

/// Cámara en tercera persona.
pub const CAM_DISTANCE: f32 = 2.6;
pub const CAM_HEIGHT: f32 = 0.75;
pub const CAM_LOOK_AHEAD: f32 = 2.0;
/// Rapidez del seguimiento suave (1/s).
pub const CAM_FOLLOW: f32 = 7.0;

/// Número de estrellas del fondo.
pub const STAR_COUNT: usize = 900;
