"""
Modelos de superficie para los planetas (diseño propio, low-poly).

Jungla jurásica:  palmera.obj, helecho.obj, dino.obj (esqueleto)
Embrujado:        arbol_seco.obj, lapida.obj, casona.obj, fantasma.obj

Convención (igual que la nave): en Blender +Z es "arriba" y +Y es el
frente. Al exportar con up=Y / forward=-Z, en el .obj "arriba" es +Y y el
frente es -Z. La base de cada modelo está en el origen (z = 0), así se apoya
sobre el terreno. Las medidas están en unidades del mundo (la nave mide 1).

Ejecutar desde la carpeta del proyecto:
  /Applications/Blender.app/Contents/MacOS/Blender -b -P assets/models/modelos_superficie.py
"""
import math
import os

import bmesh
import bpy
from mathutils import Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
bpy.ops.wm.read_factory_settings(use_empty=True)

# ---------------------------------------------------------------- materiales
_materiales = {}


def material(nombre, color, emisivo=False):
    m = bpy.data.materials.new(nombre)
    m.diffuse_color = (*color, 1.0)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*color, 1.0)
    if emisivo:
        bsdf.inputs["Emission Color"].default_value = (*color, 1.0)
        bsdf.inputs["Emission Strength"].default_value = 1.0
    _materiales[nombre] = m


material("Tronco", (0.45, 0.30, 0.18))
material("TroncoAnillo", (0.32, 0.20, 0.12))
material("Hoja", (0.20, 0.58, 0.18))
material("HojaOscura", (0.10, 0.42, 0.16))
material("Helecho", (0.40, 0.72, 0.22))
material("Hueso", (0.93, 0.89, 0.76))
material("MaderaMuerta", (0.30, 0.24, 0.32))
material("Piedra", (0.56, 0.56, 0.62))
material("Musgo", (0.32, 0.42, 0.26))
material("Pared", (0.36, 0.25, 0.44))
material("Techo", (0.16, 0.12, 0.22))
material("Moldura", (0.58, 0.52, 0.64))
material("Ventana", (1.00, 0.80, 0.30), emisivo=True)
material("Puerta", (0.26, 0.15, 0.10))
material("Fantasma", (0.85, 0.95, 1.00))
material("Ojo", (0.02, 0.02, 0.05))


class Modelo:
    """Un objeto de Blender con su bmesh y su lista de materiales."""

    def __init__(self, nombre):
        self.nombre = nombre
        self.mesh = bpy.data.meshes.new(nombre)
        self.obj = bpy.data.objects.new(nombre, self.mesh)
        bpy.context.scene.collection.objects.link(self.obj)
        self.bm = bmesh.new()
        self.mats = []

    def mat(self, nombre):
        if nombre not in self.mats:
            self.mats.append(nombre)
            self.mesh.materials.append(_materiales[nombre])
        return self.mats.index(nombre)

    def pintar(self, caras, nombre, smooth=True):
        i = self.mat(nombre)
        for f in caras:
            f.material_index = i
            f.smooth = smooth
        return caras

    # ---------------------------------------------------------- primitivas
    def caja(self, centro, tam, nombre, rot=Matrix.Identity(4)):
        m = Matrix.Translation(Vector(centro)) @ rot @ Matrix.Diagonal((*tam, 1.0))
        r = bmesh.ops.create_cube(self.bm, size=1.0, matrix=m)
        caras = list({f for v in r["verts"] for f in v.link_faces})
        return self.pintar(caras, nombre, smooth=False)

    def esfera(self, centro, radios, seg, anillos, nombre, rot=Matrix.Identity(4)):
        m = Matrix.Translation(Vector(centro)) @ rot @ Matrix.Diagonal((*radios, 1.0))
        r = bmesh.ops.create_uvsphere(self.bm, u_segments=seg, v_segments=anillos, radius=1.0, matrix=m)
        caras = list({f for v in r["verts"] for f in v.link_faces})
        return self.pintar(caras, nombre)

    def cono(self, centro, r1, r2, alto, seg, nombre, rot=Matrix.Identity(4)):
        m = Matrix.Translation(Vector(centro)) @ rot
        r = bmesh.ops.create_cone(self.bm, cap_ends=True, segments=seg, radius1=r1, radius2=r2, depth=alto, matrix=m)
        caras = list({f for v in r["verts"] for f in v.link_faces})
        return self.pintar(caras, nombre, smooth=False)

    def tubo(self, puntos, radios, lados, nombre, mat_por_tramo=None, tapa=True):
        """Tubo que sigue una lista de puntos, con un radio por punto."""
        bm = self.bm
        anillos = []
        for i, p in enumerate(puntos):
            p = Vector(p)
            if i < len(puntos) - 1:
                t = (Vector(puntos[i + 1]) - p).normalized()
            else:
                t = (p - Vector(puntos[i - 1])).normalized()
            ref = Vector((0, 0, 1)) if abs(t.z) < 0.9 else Vector((1, 0, 0))
            a = t.cross(ref).normalized()
            b = t.cross(a).normalized()
            anillo = []
            for k in range(lados):
                ang = 2 * math.pi * k / lados
                anillo.append(bm.verts.new(p + (a * math.cos(ang) + b * math.sin(ang)) * radios[i]))
            anillos.append(anillo)
        caras = []
        for i in range(len(anillos) - 1):
            nom = mat_por_tramo(i) if mat_por_tramo else nombre
            for k in range(lados):
                k2 = (k + 1) % lados
                f = bm.faces.new((anillos[i][k], anillos[i][k2], anillos[i + 1][k2], anillos[i + 1][k]))
                self.pintar([f], nom)
                caras.append(f)
        if tapa:
            f = bm.faces.new(list(reversed(anillos[-1])))
            self.pintar([f], nombre)
            f = bm.faces.new(anillos[0])
            self.pintar([f], nombre)
        return caras

    def hoja(self, base, direccion, largo, ancho, sube, cae, tramos, nombre):
        """Hoja arqueada con grosor: sale de `base` hacia `direccion`,
        subiendo `sube` y cayendo `cae` en la punta."""
        bm = self.bm
        d = Vector(direccion).normalized()
        lado = d.cross(Vector((0, 0, 1))).normalized()
        arriba, abajo = [], []
        for i in range(tramos + 1):
            t = i / tramos
            centro = Vector(base) + d * (largo * t) + Vector((0, 0, sube * math.sin(math.pi * t * 0.6) - cae * t * t))
            w = ancho * math.sin(math.pi * min(t * 1.1 + 0.08, 1.0))
            grosor = Vector((0, 0, 0.012))
            for s in (-1, 1):
                p = centro + lado * (w * s)
                arriba.append(bm.verts.new(p + grosor))
                abajo.append(bm.verts.new(p - grosor))
        caras = []
        for i in range(tramos):
            a0, a1, b0, b1 = 2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3
            caras.append(bm.faces.new((arriba[a0], arriba[a1], arriba[b1], arriba[b0])))
            caras.append(bm.faces.new((abajo[a0], abajo[b0], abajo[b1], abajo[a1])))
            caras.append(bm.faces.new((arriba[a0], arriba[b0], abajo[b0], abajo[a0])))
            caras.append(bm.faces.new((arriba[a1], abajo[a1], abajo[b1], arriba[b1])))
        return self.pintar(caras, nombre, smooth=False)

    def exportar(self):
        bm = self.bm
        bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
        bm.to_mesh(self.mesh)
        bm.free()
        self.mesh.update()
        bpy.ops.object.select_all(action="DESELECT")
        self.obj.select_set(True)
        bpy.context.view_layer.objects.active = self.obj
        bpy.ops.wm.obj_export(
            filepath=os.path.join(HERE, self.nombre + ".obj"),
            export_selected_objects=True,
            export_uv=False,
            export_normals=True,
            export_colors=False,
            export_triangulated_mesh=True,
            export_materials=True,
            forward_axis="NEGATIVE_Z",
            up_axis="Y",
        )
        tris = sum(len(p.vertices) - 2 for p in self.mesh.polygons)
        print(f"{self.nombre}: {tris} triangulos")


def rot_x(g):
    return Matrix.Rotation(math.radians(g), 4, "X")


def rot_y(g):
    return Matrix.Rotation(math.radians(g), 4, "Y")


def rot_z(g):
    return Matrix.Rotation(math.radians(g), 4, "Z")


# =============================================================== JUNGLA
# ---------------------------------------------------------------- palmera
m = Modelo("palmera")
alto, tramos = 1.15, 5
pts = [(0.2 * (i / tramos) ** 2, 0.0, alto * i / tramos) for i in range(tramos + 1)]
radios = [0.16 - 0.06 * (i / tramos) for i in range(tramos + 1)]
m.tubo(pts, radios, 6, "Tronco", mat_por_tramo=lambda i: "TroncoAnillo" if i % 2 else "Tronco")
top = Vector(pts[-1])
for k in range(8):
    ang = 2 * math.pi * k / 8 + 0.3
    d = (math.cos(ang), math.sin(ang), 0.0)
    m.hoja(top, d, 1.15, 0.26, 0.3, 0.6, 4, "Hoja" if k % 2 else "HojaOscura")
m.esfera(tuple(top), (0.18, 0.18, 0.14), 6, 4, "TroncoAnillo")
m.exportar()

# ---------------------------------------------------------------- helecho
m = Modelo("helecho")
for k in range(8):
    ang = 2 * math.pi * k / 8 + (0.2 if k % 2 else 0.0)
    d = (math.cos(ang), math.sin(ang), 0.0)
    largo = 0.75 if k % 2 else 0.6
    m.hoja((0, 0, 0.02), d, largo, 0.12, 0.55, 0.45, 4, "Helecho" if k % 2 else "Hoja")
m.exportar()

# ---------------------------------------------------------------- esqueleto de dinosaurio
m = Modelo("dino")
# Columna: de la cola (x negativa) al cuello (x positiva), arqueada.
n_vert = 16
columna = []
for i in range(n_vert):
    t = i / (n_vert - 1)
    x = -2.0 + 3.6 * t
    z = 0.15 + 0.75 * math.sin(math.pi * min(t * 1.15, 1.0)) * (0.4 + 0.6 * t)
    columna.append(Vector((x, 0.0, z)))
m.tubo([tuple(p) for p in columna], [0.05 + 0.05 * math.sin(math.pi * i / (n_vert - 1)) for i in range(n_vert)], 5, "Hueso")
for p in columna[::2]:
    m.caja(tuple(p + Vector((0, 0, 0.08))), (0.07, 0.07, 0.12), "Hueso")
# Costillas: arcos a ambos lados del tórax, enterrándose en el suelo.
for i in range(5, 11):
    base = columna[i]
    largo = 0.55 + 0.25 * math.sin(math.pi * (i - 5) / 5)
    for lado in (1, -1):
        arco = []
        for j in range(6):
            a = math.pi * 0.5 * j / 5
            arco.append((base.x, lado * largo * math.sin(a) * 0.9, base.z - largo * (1 - math.cos(a)) * 1.25))
        m.tubo(arco, [0.035] * len(arco), 4, "Hueso", tapa=False)
# Cráneo y mandíbula al final del cuello.
cabeza = columna[-1] + Vector((0.35, 0.0, 0.05))
m.esfera(tuple(cabeza), (0.42, 0.18, 0.2), 8, 5, "Hueso")
m.esfera(tuple(cabeza + Vector((0.08, 0.0, -0.18))), (0.36, 0.13, 0.07), 8, 4, "Hueso", rot=rot_y(12))
m.esfera(tuple(cabeza + Vector((0.08, 0.15, 0.07))), (0.07, 0.04, 0.06), 6, 4, "TroncoAnillo")
m.esfera(tuple(cabeza + Vector((0.08, -0.15, 0.07))), (0.07, 0.04, 0.06), 6, 4, "TroncoAnillo")
# Huesos de las patas, tirados en el suelo.
m.tubo([(-0.4, 0.5, 0.05), (0.3, 0.75, 0.05)], [0.05, 0.04], 5, "Hueso")
m.tubo([(0.6, -0.55, 0.05), (1.1, -0.95, 0.04)], [0.045, 0.035], 5, "Hueso")
m.exportar()

# =============================================================== EMBRUJADO
# ---------------------------------------------------------------- árbol seco
m = Modelo("arbol_seco")
tronco = [(0, 0, 0), (0.05, 0.02, 0.45), (-0.04, 0.05, 0.9), (0.08, -0.03, 1.3), (0.02, 0.0, 1.6)]
m.tubo(tronco, [0.12, 0.09, 0.07, 0.05, 0.02], 5, "MaderaMuerta")


def rama(origen, direccion, largo, radio, nivel):
    d = Vector(direccion).normalized()
    p0 = Vector(origen)
    p1 = p0 + d * (largo * 0.5) + Vector((0, 0, 0.08 * largo))
    p2 = p0 + d * largo + Vector((0, 0, -0.05 * largo))
    m.tubo([tuple(p0), tuple(p1), tuple(p2)], [radio, radio * 0.6, radio * 0.15], 4, "MaderaMuerta")
    if nivel > 0:
        for g in (35, -40):
            nd = Matrix.Rotation(math.radians(g), 3, "Z") @ d + Vector((0, 0, 0.35))
            rama(tuple(p1), nd, largo * 0.55, radio * 0.5, nivel - 1)


for ang, h, l in ((20, 0.8, 0.7), (150, 1.05, 0.6), (260, 1.3, 0.5), (80, 1.45, 0.4)):
    a = math.radians(ang)
    rama(tronco[2] if h < 1.0 else tronco[3], (math.cos(a), math.sin(a), 0.55), l, 0.05, 1)
m.exportar()

# ---------------------------------------------------------------- lápida
m = Modelo("lapida")
# Losa con remate redondeado (polígono en el plano XZ extruido en Y).
perfil = [(-0.22, 0.0), (0.22, 0.0), (0.22, 0.42)]
for i in range(1, 6):
    a = math.pi * i / 6
    perfil.append((0.22 * math.cos(a), 0.42 + 0.18 * math.sin(a)))
perfil.append((-0.22, 0.42))
bm = m.bm
frente = [bm.verts.new((x, -0.05, z)) for x, z in perfil]
atras = [bm.verts.new((x, 0.05, z)) for x, z in perfil]
caras = [bm.faces.new(frente), bm.faces.new(list(reversed(atras)))]
for i in range(len(perfil)):
    j = (i + 1) % len(perfil)
    caras.append(bm.faces.new((frente[i], atras[i], atras[j], frente[j])))
m.pintar(caras, "Piedra", smooth=False)
m.caja((0, 0, 0.03), (0.55, 0.22, 0.08), "Musgo")
m.caja((0, -0.055, 0.42), (0.18, 0.01, 0.05), "Musgo")  # inscripción gastada
m.exportar()

# ---------------------------------------------------------------- casona embrujada
m = Modelo("casona")
m.caja((0, 0, 0.1), (2.4, 1.8, 0.2), "Moldura")  # base
m.caja((0, 0, 0.85), (2.0, 1.4, 1.3), "Pared")  # cuerpo
# Techo a dos aguas (prisma), un poco más grande que el cuerpo.
bm = m.bm
w, d, h0, h1 = 1.15, 0.85, 1.5, 2.25
v = [bm.verts.new(p) for p in [(-w, -d, h0), (w, -d, h0), (w, d, h0), (-w, d, h0), (-w, 0, h1), (w, 0, h1)]]
caras = [
    bm.faces.new((v[0], v[1], v[5], v[4])),
    bm.faces.new((v[2], v[3], v[4], v[5])),
    bm.faces.new((v[0], v[4], v[3])),
    bm.faces.new((v[1], v[2], v[5])),
    bm.faces.new((v[0], v[3], v[2], v[1])),
]
m.pintar(caras, "Techo", smooth=False)
# Torre torcida con techo puntiagudo.
torre_rot = rot_y(-6) @ rot_x(4)
m.caja((0.95, 0.55, 1.4), (0.75, 0.75, 2.6), "Pared", rot=torre_rot)
m.cono((1.1, 0.62, 3.25), 0.62, 0.0, 1.2, 4, "Techo", rot=torre_rot @ rot_z(45))
m.caja((1.13, 0.53, 2.3), (0.3, 0.05, 0.4), "Ventana", rot=torre_rot)  # ventana de la torre (hacia +Y)
m.caja((1.1, 0.16, 2.3), (0.3, 0.05, 0.4), "Ventana", rot=torre_rot)
# Chimenea chueca.
m.caja((-0.6, 0.3, 2.2), (0.25, 0.25, 0.7), "Moldura", rot=rot_y(8))
# Ventanas del frente (+Y) y laterales, y la puerta.
for x in (-0.6, 0.2):
    m.caja((x, 0.71, 1.05), (0.32, 0.04, 0.42), "Ventana")
    m.caja((x, 0.72, 1.28), (0.4, 0.05, 0.06), "Moldura")
m.caja((-0.2, 0.71, 0.5), (0.36, 0.05, 0.6), "Puerta")
m.caja((-1.01, -0.2, 1.05), (0.04, 0.32, 0.42), "Ventana")
m.caja((-0.6, -0.71, 1.05), (0.32, 0.04, 0.42), "Ventana")
m.exportar()

# ---------------------------------------------------------------- fantasma
m = Modelo("fantasma")
bm = m.bm
seg = 14
anillos = []
# Perfil (z, radio): cabeza redonda arriba, faldón ondulado abajo.
perfil = [(1.0, 0.0), (0.97, 0.18), (0.88, 0.3), (0.72, 0.36), (0.5, 0.37), (0.3, 0.38), (0.12, 0.42), (0.0, 0.44)]
for i, (z, r) in enumerate(perfil):
    anillo = []
    for k in range(seg):
        a = 2 * math.pi * k / seg
        ondas = 1.0
        dz = 0.0
        if i == len(perfil) - 1:  # borde inferior ondulado
            ondas = 1.0 + 0.12 * math.sin(5 * a)
            dz = 0.08 * math.sin(5 * a)
        anillo.append(bm.verts.new((r * ondas * math.cos(a), r * ondas * math.sin(a), z + dz)))
    anillos.append(anillo)
caras = []
for i in range(len(anillos) - 1):
    for k in range(seg):
        k2 = (k + 1) % seg
        if i == 0:
            caras.append(bm.faces.new((anillos[0][k], anillos[1][k], anillos[1][k2])))
        else:
            caras.append(bm.faces.new((anillos[i][k], anillos[i + 1][k], anillos[i + 1][k2], anillos[i][k2])))
m.pintar(caras, "Fantasma")
# Ojos y boca (al frente, +Y).
m.esfera((-0.12, 0.33, 0.72), (0.06, 0.03, 0.09), 6, 4, "Ojo")
m.esfera((0.12, 0.33, 0.72), (0.06, 0.03, 0.09), 6, 4, "Ojo")
m.esfera((0.0, 0.35, 0.5), (0.07, 0.03, 0.05), 6, 4, "Ojo")
# Bracitos.
m.esfera((0.4, 0.05, 0.45), (0.12, 0.07, 0.07), 6, 4, "Fantasma", rot=rot_y(-30))
m.esfera((-0.4, 0.05, 0.45), (0.12, 0.07, 0.07), 6, 4, "Fantasma", rot=rot_y(30))
m.exportar()
