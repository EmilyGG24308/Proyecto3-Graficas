//! Mundos de Emily: Ciudad (órbita 45) y Hielo / océano (órbita 120).
//!
//! Cada planeta vive en su propio archivo:
//!  - `ciudad.rs`: shader de la ciudad y colocación de edificios y árboles.
//!  - `cuadricula.rs`: la cuadrícula de calles sobre la esfera (proyección de
//!    cubo). La usan el shader Y la colocación, por eso está aparte.
//!  - `hielo.rs`: shader del océano y los casquetes, y sus objetos.
//!  - `comun.rs`: utilidades que usan los dos planetas.
//!
//! Los modelos salen de `assets/models/modelos_emily.py`. Los valores que se
//! pueden cambiar en la demo (radio, órbita, semilla, intensidades) están en
//! `ciudad()` y `hielo()`.
//!
//! Ciudad: un planeta casi plano cubierto por una cuadrícula de calles. El
//! MISMO cálculo de cuadrícula decide en el shader qué es calle, parque o
//! plaza, y en la CPU dónde va cada edificio: por eso los edificios caen
//! dentro de las manzanas y nunca encima de una calle.
//!
//! Hielo / océano: un mar alto cubre casi todo; quedan islas. El agua tiene
//! olas animadas con normal mapping procedural (la normal se perturba con la
//! derivada analítica de las olas), espuma en crestas y en la costa,
//! casquetes polares por latitud con témpanos rotos en el borde y una
//! aurora que solo se ve del lado nocturno.

mod ciudad;
mod comun;
mod cuadricula;
mod hielo;

use crate::planet::Planet;

/// Planetas de Emily. Se agregan solos al sistema solar (índices 6 y 7 en
/// `--planeta N`).
pub fn planetas() -> Result<Vec<Planet>, String> {
    Ok(vec![ciudad::ciudad()?, hielo::hielo()?])
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::config::WORLD_SEED;

    #[test]
    fn mundos_reproducibles_y_en_presupuesto() {
        let a = planetas().unwrap();
        let b = planetas().unwrap();
        assert_eq!(a.len(), 2);
        assert_eq!((a[0].def.name, a[0].def.orbit.radius, a[0].def.seed), ("Ciudad", 45.0, WORLD_SEED + 30));
        assert_eq!((a[1].def.name, a[1].def.orbit.radius, a[1].def.seed), ("Hielo", 120.0, WORLD_SEED + 31));
        for (x, y) in a.iter().zip(&b) {
            let px = x.props.as_ref().unwrap();
            assert_eq!(px.mesh.positions, y.props.as_ref().unwrap().mesh.positions, "misma semilla, mismos objetos");
            assert!(px.mesh.triangle_count() <= 60_000, "{}: {} triángulos", x.def.name, px.mesh.triangle_count());
            assert!(x.bounding_radius >= px.mesh.bounding_radius * x.def.radius);
            // Al menos dos tipos de objeto con instancias.
            assert!(px.counts.iter().filter(|(_, c)| *c > 0).count() >= 2, "{:?}", px.counts);
        }
    }
}
