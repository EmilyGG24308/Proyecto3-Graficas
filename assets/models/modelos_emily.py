"""
Modelos low-poly de los planetas de Emily (diseño propio).

Ciudad:        ciudad_edificio.obj, ciudad_rascacielos.obj, ciudad_antena.obj, ciudad_arbol.obj
Hielo/océano:  hielo_iceberg.obj, hielo_iglu.obj, hielo_pinguino.obj, hielo_faro.obj

Misma convención que `modelos_superficie.py`: se modela con +Z arriba y +Y
al frente, la base en z = 0 y las medidas en unidades del mundo (la nave
mide 1). Al exportar se hace la misma conversión que el exportador de
Blender con up=Y / forward=-Z: (x, y, z) -> (x, z, -y).

Por qué no usa `bpy`: el script arma la malla y escribe el .obj/.mtl él
mismo, así no depende de tener Blender instalado y el resultado es
idéntico en cualquier máquina. Corre con Python 3 o dentro de Blender:
  python3 assets/models/modelos_emily.py
  /Applications/Blender.app/Contents/MacOS/Blender -b -P assets/models/modelos_emily.py

Cada material es un color plano (Kd); los emisivos además llevan Ke, que
`mesh::load_obj` usa para pintarlos sin iluminación (ventanas, faro, focos).
"""
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))

# ---------------------------------------------------------------- materiales
_materiales = {}


def material(nombre, color, emisivo=False):
    _materiales[nombre] = (color, emisivo)


# Ciudad: paredes cálidas y techos oscuros para que se lean contra las calles.
material("ParedCrema", (0.93, 0.82, 0.62))
material("ParedLadrillo", (0.78, 0.36, 0.26))
material("ParedAzul", (0.32, 0.46, 0.78))
material("Techo", (0.22, 0.20, 0.26))
material("Ventana", (1.00, 0.80, 0.35), emisivo=True)
material("VidrioDia", (0.55, 0.72, 0.92))
material("VentanaTibia", (1.00, 0.86, 0.52))
material("Concreto", (0.62, 0.62, 0.66))
material("Metal", (0.40, 0.42, 0.48))
material("FocoRojo", (1.00, 0.18, 0.12), emisivo=True)
material("Tanque", (0.55, 0.38, 0.24))
material("Copa", (0.28, 0.66, 0.26))
material("CopaOscura", (0.16, 0.48, 0.20))
material("TroncoCiudad", (0.45, 0.30, 0.18))
# Hielo: blancos con sombra azul (nada de turquesa/violeta: eso es Cristalino).
material("Nieve", (0.95, 0.97, 1.00))
material("HieloAzul", (0.62, 0.80, 0.98))
material("HieloProfundo", (0.30, 0.52, 0.86))
material("Entrada", (0.10, 0.12, 0.20))
material("PlumaNegra", (0.10, 0.11, 0.16))
material("PlumaBlanca", (0.97, 0.97, 0.95))
material("Pico", (1.00, 0.55, 0.10))
material("FaroRojo", (0.86, 0.16, 0.16))
material("FaroBlanco", (0.96, 0.96, 0.94))
material("Lampara", (1.00, 0.92, 0.55), emisivo=True)
material("Roca", (0.34, 0.33, 0.38))


# ---------------------------------------------------------------- vectores
def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def add(a, b):
    return (a[0] + b[0], a[1] + b[1], a[2] + b[2])


def mul(a, s):
    return (a[0] * s, a[1] * s, a[2] * s)


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def norm(a):
    l = math.sqrt(dot(a, a)) or 1.0
    return (a[0] / l, a[1] / l, a[2] / l)


def rot_z(p, grados):
    a = math.radians(grados)
    c, s = math.cos(a), math.sin(a)
    return (p[0] * c - p[1] * s, p[0] * s + p[1] * c, p[2])


class Modelo:
    """Malla con caras de color plano por material (equivalente al bmesh)."""

    def __init__(self, nombre):
        self.nombre = nombre
        # Cada cara: (lista de puntos, material, normales por vértice o None).
        self.caras = []

    def _cara(self, puntos, nombre, centro, normales=None):
        """Agrega una cara convexa orientada hacia fuera de `centro`: si el
        orden de los vértices la deja mirando hacia dentro, se invierte."""
        n = cross(sub(puntos[1], puntos[0]), sub(puntos[2], puntos[0]))
        medio = mul(
            (sum(p[0] for p in puntos), sum(p[1] for p in puntos), sum(p[2] for p in puntos)),
            1.0 / len(puntos),
        )
        if dot(n, sub(medio, centro)) < 0:
            puntos = list(reversed(puntos))
            normales = list(reversed(normales)) if normales else None
        self.caras.append((puntos, nombre, normales))

    # ---------------------------------------------------------- primitivas
    def caja(self, centro, tam, nombre, giro=0.0):
        """Caja de lados `tam` centrada en `centro`, girada `giro` grados en Z."""
        hx, hy, hz = tam[0] / 2, tam[1] / 2, tam[2] / 2
        esquinas = [
            add(centro, rot_z((sx * hx, sy * hy, sz * hz), giro))
            for sz in (-1, 1)
            for sy in (-1, 1)
            for sx in (-1, 1)
        ]
        e = esquinas
        for cara in ((0, 1, 3, 2), (4, 5, 7, 6), (0, 1, 5, 4), (2, 3, 7, 6), (0, 2, 6, 4), (1, 3, 7, 5)):
            self._cara([e[i] for i in cara], nombre, centro)

    def cono(self, centro, r1, r2, alto, seg, nombre, giro=0.0):
        """Tronco de cono (r2 = 0: cono con punta) con eje vertical."""
        abajo = sub(centro, (0, 0, alto / 2))
        arriba = add(centro, (0, 0, alto / 2))
        anillo = lambda base, r: [
            add(base, rot_z((r * math.cos(2 * math.pi * k / seg), r * math.sin(2 * math.pi * k / seg), 0), giro))
            for k in range(seg)
        ]
        a, b = anillo(abajo, r1), anillo(arriba, r2)
        for k in range(seg):
            k2 = (k + 1) % seg
            if r2 > 0:
                self._cara([a[k], a[k2], b[k2], b[k]], nombre, centro)
            else:
                self._cara([a[k], a[k2], arriba], nombre, centro)
        self._cara(a, nombre, centro)
        if r2 > 0:
            self._cara(b, nombre, centro)

    def esfera(self, centro, radios, seg, anillos, nombre, media=False):
        """Elipsoide UV con normales suaves (cuerpos redondos). `media` = solo
        la mitad de arriba (cúpula del iglú)."""
        def punto(i, k):
            lat = math.pi * i / anillos - math.pi / 2
            if media:
                lat = (math.pi / 2) * i / anillos
            lon = 2 * math.pi * k / seg
            u = (math.cos(lat) * math.cos(lon), math.cos(lat) * math.sin(lon), math.sin(lat))
            p = add(centro, (u[0] * radios[0], u[1] * radios[1], u[2] * radios[2]))
            # Normal de un elipsoide: gradiente de (x/a)² + (y/b)² + (z/c)².
            n = norm((u[0] / radios[0], u[1] / radios[1], u[2] / radios[2]))
            return p, n

        for i in range(anillos):
            for k in range(seg):
                quad = [punto(i, k), punto(i, k + 1), punto(i + 1, k + 1), punto(i + 1, k)]
                # Se quitan los vértices repetidos de los polos (triángulos).
                unicos = []
                for p, n in quad:
                    if all(math.dist(p, q) > 1e-6 for q, _ in unicos):
                        unicos.append((p, n))
                if len(unicos) >= 3:
                    self._cara([p for p, _ in unicos], nombre, centro, [n for _, n in unicos])
        if media:
            base = [punto(0, k)[0] for k in range(seg)]
            self._cara(base, nombre, add(centro, (0, 0, radios[2])))

    # ---------------------------------------------------------- exportar
    def exportar(self):
        conv = lambda p: (p[0], p[2], -p[1])  # Blender (Z arriba) -> juego (Y arriba)
        mats = []
        lineas = ["# modelos_emily.py", f"mtllib {self.nombre}.mtl", f"o {self.nombre}"]
        vs, vns, fs = [], [], {}
        for puntos, nombre, normales in self.caras:
            if nombre not in mats:
                mats.append(nombre)
            plana = norm(cross(sub(puntos[1], puntos[0]), sub(puntos[2], puntos[0])))
            idx = []
            for j, p in enumerate(puntos):
                vs.append(conv(p))
                vns.append(conv(normales[j] if normales else plana))
                idx.append(len(vs))
            fs.setdefault(nombre, []).append(idx)
        lineas += [f"v {x:.6f} {y:.6f} {z:.6f}" for x, y, z in vs]
        lineas += [f"vn {x:.4f} {y:.4f} {z:.4f}" for x, y, z in vns]
        tris = 0
        for nombre in mats:
            lineas += [f"usemtl {nombre}", "s off"]
            for idx in fs[nombre]:
                lineas.append("f " + " ".join(f"{i}//{i}" for i in idx))
                tris += len(idx) - 2
        with open(os.path.join(HERE, self.nombre + ".obj"), "w", newline="\n") as f:
            f.write("\n".join(lineas) + "\n")
        mtl = ["# modelos_emily.py"]
        for nombre in mats:
            color, emisivo = _materiales[nombre]
            ke = color if emisivo else (0.0, 0.0, 0.0)
            mtl += [
                "",
                f"newmtl {nombre}",
                "Kd {:.6f} {:.6f} {:.6f}".format(*color),
                "Ke {:.6f} {:.6f} {:.6f}".format(*ke),
                "d 1.000000",
                "illum 2",
            ]
        with open(os.path.join(HERE, self.nombre + ".mtl"), "w", newline="\n") as f:
            f.write("\n".join(mtl) + "\n")
        print(f"{self.nombre}: {tris} triangulos")


def ventanas(m, ancho, fondo, pisos, alto_piso, z0, por_lado, nombre="Ventana", cada=1):
    """Hileras de ventanas (cajitas delgadas) en las cuatro fachadas. Solo se
    ponen cada `cada` pisos para no gastar triángulos."""
    for piso in range(0, pisos, cada):
        z = z0 + (piso + 0.55) * alto_piso
        for k in range(por_lado):
            t = (k + 0.5) / por_lado - 0.5
            m.caja((t * ancho * 0.8, fondo / 2 + 0.01, z), (ancho * 0.8 / por_lado * 0.6, 0.03, alto_piso * 0.45), nombre)
            m.caja((t * ancho * 0.8, -fondo / 2 - 0.01, z), (ancho * 0.8 / por_lado * 0.6, 0.03, alto_piso * 0.45), nombre)
            m.caja((ancho / 2 + 0.01, t * fondo * 0.8, z), (0.03, fondo * 0.8 / por_lado * 0.6, alto_piso * 0.45), nombre)
            m.caja((-ancho / 2 - 0.01, t * fondo * 0.8, z), (0.03, fondo * 0.8 / por_lado * 0.6, alto_piso * 0.45), nombre)


# =============================================================== CIUDAD
# Edificio de departamentos: bloque bajo con dos pisos de ventanas (no
# emisivas: de noche se apagan y solo brillan los rascacielos), cornisa
# oscura y un tanque de agua en la azotea.
m = Modelo("ciudad_edificio")
m.caja((0, 0, 0.55), (0.9, 0.9, 1.1), "ParedLadrillo")
m.caja((0, 0, 1.13), (0.98, 0.98, 0.08), "Techo")
ventanas(m, 0.9, 0.9, 2, 0.45, 0.12, 1, "VentanaTibia")
m.cono((0.18, 0.16, 1.32), 0.13, 0.13, 0.3, 6, "Tanque")
m.cono((0.18, 0.16, 1.5), 0.15, 0.0, 0.08, 6, "Techo")
m.caja((-0.22, -0.2, 1.25), (0.25, 0.25, 0.16), "Concreto")
m.exportar()

# Rascacielos escalonado (silueta "art déco"): tres cuerpos que se
# angostan, franjas de ventanas y una aguja con foco rojo de aviación.
m = Modelo("ciudad_rascacielos")
m.caja((0, 0, 0.9), (1.0, 1.0, 1.8), "ParedAzul")
m.caja((0, 0, 2.2), (0.72, 0.72, 0.8), "ParedCrema")
m.caja((0, 0, 2.8), (0.46, 0.46, 0.4), "ParedAzul")
for z, lado in [(0.6, 1.0), (1.3, 1.0), (2.15, 0.72)]:
    for s in (-1, 1):
        m.caja((0, s * (lado / 2 + 0.01), z), (lado * 0.82, 0.03, 0.22), "Ventana")
        m.caja((s * (lado / 2 + 0.01), 0, z), (0.03, lado * 0.82, 0.22), "Ventana")
m.caja((0, 0, 1.82), (1.08, 1.08, 0.06), "Techo")
m.caja((0, 0, 2.62), (0.8, 0.8, 0.05), "Techo")
m.cono((0, 0, 3.25), 0.07, 0.0, 0.5, 4, "Metal")
m.esfera((0, 0, 3.5), (0.07, 0.07, 0.07), 6, 3, "FocoRojo")
m.exportar()

# Antena de radio: torre de celosía (pirámide de cuatro lados), travesaños
# y un foco que se lee de noche.
m = Modelo("ciudad_antena")
m.cono((0, 0, 0.06), 0.3, 0.3, 0.12, 4, "Concreto", giro=45)
m.cono((0, 0, 1.05), 0.16, 0.03, 1.9, 4, "Metal", giro=45)
for z, ancho in [(0.55, 0.5), (1.15, 0.36), (1.65, 0.24)]:
    m.caja((0, 0, z), (ancho, 0.05, 0.05), "FocoRojo" if z > 1.6 else "Metal")
    m.caja((0, 0, z), (0.05, ancho, 0.05), "Metal")
m.esfera((0, 0, 2.05), (0.09, 0.09, 0.09), 6, 3, "FocoRojo")
m.exportar()

# Árbol de parque: copa redonda en dos tonos, tronco corto.
m = Modelo("ciudad_arbol")
m.cono((0, 0, 0.2), 0.06, 0.05, 0.4, 5, "TroncoCiudad")
m.esfera((0, 0, 0.6), (0.3, 0.3, 0.28), 7, 4, "Copa")
m.esfera((0.1, -0.08, 0.78), (0.18, 0.18, 0.16), 6, 3, "CopaOscura")
m.exportar()

# =============================================================== HIELO / OCÉANO
# Iceberg: bloque facetado con meseta plana; la base es ancha y azul
# porque queda medio hundida en el mar (se entierra con `sink`).
m = Modelo("hielo_iceberg")
m.cono((0, 0, 0.25), 0.95, 0.8, 0.5, 6, "HieloProfundo", giro=10)
m.cono((0.05, 0.0, 0.75), 0.8, 0.55, 0.5, 6, "HieloAzul", giro=35)
m.cono((-0.15, 0.1, 1.15), 0.5, 0.0, 0.5, 5, "Nieve", giro=5)
m.cono((0.35, -0.15, 1.05), 0.38, 0.22, 0.4, 5, "Nieve", giro=50)
m.exportar()

# Iglú: cúpula de nieve con túnel de entrada oscuro.
m = Modelo("hielo_iglu")
m.esfera((0, 0, 0), (0.6, 0.6, 0.5), 10, 4, "Nieve", media=True)
m.caja((0, 0.62, 0.17), (0.34, 0.36, 0.34), "HieloAzul")
m.caja((0, 0.81, 0.15), (0.22, 0.02, 0.24), "Entrada")
m.cono((0, 0, 0.52), 0.12, 0.0, 0.06, 6, "HieloAzul")
m.exportar()

# Pingüino caricatura: cuerpo negro, panza blanca, pico y patas naranjas.
# Grande respecto al planeta (escala "Mario Galaxy") para leerse de lejos.
m = Modelo("hielo_pinguino")
m.esfera((0, 0, 0.36), (0.24, 0.22, 0.34), 8, 5, "PlumaNegra")
m.esfera((0, 0.1, 0.33), (0.18, 0.14, 0.27), 8, 4, "PlumaBlanca")
m.esfera((0, 0.02, 0.76), (0.16, 0.16, 0.15), 8, 4, "PlumaNegra")
m.esfera((-0.06, 0.15, 0.8), (0.035, 0.02, 0.04), 4, 2, "PlumaBlanca")
m.esfera((0.06, 0.15, 0.8), (0.035, 0.02, 0.04), 4, 2, "PlumaBlanca")
m.caja((0, 0.22, 0.73), (0.08, 0.14, 0.05), "Pico")
m.caja((-0.09, 0.08, 0.03), (0.1, 0.16, 0.05), "Pico")
m.caja((0.09, 0.08, 0.03), (0.1, 0.16, 0.05), "Pico")
m.caja((-0.25, 0.0, 0.4), (0.05, 0.1, 0.28), "PlumaNegra", giro=0)
m.caja((0.25, 0.0, 0.4), (0.05, 0.1, 0.28), "PlumaNegra", giro=0)
m.exportar()

# Faro (monumento único en la isla más alta): franjas rojas y blancas y
# una linterna emisiva que se ve desde el lado nocturno.
m = Modelo("hielo_faro")
m.cono((0, 0, 0.12), 0.55, 0.5, 0.24, 8, "Roca")
franjas = ["FaroRojo", "FaroBlanco", "FaroRojo", "FaroBlanco"]
r = 0.34
for i, nombre in enumerate(franjas):
    m.cono((0, 0, 0.24 + 0.35 * i + 0.175), r, r - 0.04, 0.35, 8, nombre)
    r -= 0.04
m.cono((0, 0, 1.68), 0.24, 0.24, 0.08, 8, "Roca")
m.cono((0, 0, 1.86), 0.15, 0.15, 0.28, 8, "Lampara")
m.cono((0, 0, 2.1), 0.22, 0.0, 0.22, 8, "FaroRojo")
m.exportar()
