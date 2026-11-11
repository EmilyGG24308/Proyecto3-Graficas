//! Mundos de Esteban: Lava (órbita 30) y Desierto con anillo de asteroides (órbita 80).
//!
//! Todo lo de estos planetas vive en este archivo (o en submódulos de
//! `src/mundos/esteban/`): definición, shaders y reglas de objetos.
//! Ver `GUIA_MUNDOS.md` y usar como referencia `src/planets.rs`
//! (jungla y embrujado), `src/shaders/jungle.rs` y `src/shaders/haunted.rs`.

use crate::planet::Planet;

/// Planetas de Esteban. Se agregan solos al sistema solar.
pub fn planetas() -> Result<Vec<Planet>, String> {
    Ok(vec![
        // planeta_1()?,
        // planeta_2()?,
    ])
}
