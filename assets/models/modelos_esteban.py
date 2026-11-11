"""Modelos low-poly de lava y desierto. Ejecutar con Blender en modo background."""
import math
import os

import bmesh
import bpy
from mathutils import Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
bpy.ops.wm.read_factory_settings(use_empty=True)
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


material("Basalto", (0.13, 0.12, 0.16))
material("BasaltoClaro", (0.25, 0.22, 0.25))
material("Obsidiana", (0.24, 0.12, 0.30))
material("Brasa", (1.0, 0.25, 0.03), emisivo=True)
material("Cactus", (0.16, 0.50, 0.24))
material("CactusClaro", (0.28, 0.65, 0.29))
material("Flor", (1.0, 0.35, 0.25), emisivo=True)
material("Arenisca", (0.76, 0.43, 0.23))
material("Estrato", (0.55, 0.25, 0.16))
material("Asteroide", (0.43, 0.29, 0.23))


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


# Columnas hexagonales de basalto con pequeñas grietas encendidas.
m = Modelo("lava_columnas")
for x, y, alto, radio in [(-0.38, 0, 1.05, 0.30), (0.23, 0.15, 1.55, 0.34), (0.25, -0.42, 0.75, 0.26)]:
    m.cono((x, y, alto / 2), radio, radio * 0.91, alto, 6, "BasaltoClaro")
    m.cono((x, y, alto - 0.06), radio * 0.94, radio * 0.91, 0.12, 6, "Basalto")
m.caja((0.05, -0.32, 0.18), (0.08, 0.05, 0.34), "Brasa", rot=rot_y(-15))
m.exportar()

# Fragmentos de obsidiana facetada, sin siluetas de volcán.
m = Modelo("lava_obsidiana")
for x, y, alto, radio, giro in [(0, 0, 1.55, 0.34, -8), (0.38, 0.13, 0.85, 0.24, 20), (-0.32, -0.12, 0.7, 0.22, -25)]:
    m.cono((x, y, alto * 0.36), radio, radio * 0.72, alto * 0.72, 5, "Obsidiana", rot=rot_y(giro))
    m.cono((x, y, alto * 0.82), radio * 0.72, 0, alto * 0.35, 5, "BasaltoClaro", rot=rot_y(giro))
m.exportar()

# Cactus con brazos y una flor para reconocerlo a baja resolución.
m = Modelo("desierto_cactus")
m.cono((0, 0, 0.7), 0.20, 0.18, 1.4, 8, "Cactus")
m.esfera((0, 0, 1.4), (0.18, 0.18, 0.18), 8, 4, "CactusClaro")
for lado, alto in [(-1, 0.62), (1, 0.88)]:
    m.tubo([(0, 0, alto), (lado * 0.38, 0, alto), (lado * 0.42, 0, alto + 0.42)], [0.13, 0.12, 0.10], 6, "Cactus")
    m.esfera((lado * 0.42, 0, alto + 0.42), (0.11, 0.11, 0.12), 6, 4, "CactusClaro")
m.esfera((0, 0, 1.59), (0.16, 0.16, 0.09), 6, 4, "Flor")
m.exportar()

# Arco formado por bloques de arenisca: deja un hueco visible.
m = Modelo("desierto_arco")
for x in (-0.65, 0.65):
    m.caja((x, 0, 0.55), (0.38, 0.46, 1.1), "Arenisca")
    m.caja((x, 0, 0.44), (0.4, 0.48, 0.12), "Estrato")
    m.caja((x * 0.76, 0, 1.22), (0.65, 0.46, 0.34), "Arenisca", rot=rot_y(-28 if x < 0 else 28))
m.caja((0, 0, 1.45), (0.55, 0.5, 0.34), "Estrato")
m.exportar()

# Una roca irregular y achatada para repetir en el anillo.
m = Modelo("desierto_asteroide")
m.esfera((0, 0, 0.45), (0.85, 0.62, 0.45), 8, 5, "Asteroide")
m.cono((0.28, 0.05, 0.7), 0.34, 0.08, 0.38, 5, "Arenisca", rot=rot_y(22))
m.exportar()

# Engranaje de bronce sobre un pedestal, con diez dientes visibles.
material("Metal", (0.30, 0.40, 0.46))
material("MetalOscuro", (0.12, 0.18, 0.23))
material("Bronce", (0.80, 0.47, 0.16))
material("Energia", (0.12, 0.92, 1.0), emisivo=True)

m = Modelo("mecanico_engranaje")
m.cono((0, 0, 0.12), 0.38, 0.38, 0.24, 8, "MetalOscuro")
m.cono((0, 0, 0.45), 0.25, 0.25, 0.50, 8, "Metal")
m.cono((0, 0, 0.78), 0.62, 0.62, 0.16, 10, "Bronce")
for k in range(10):
    a = 2 * math.pi * k / 10
    m.caja((0.66 * math.cos(a), 0.66 * math.sin(a), 0.78), (0.26, 0.22, 0.16), "Bronce", rot=rot_z(math.degrees(a)))
m.cono((0, 0, 0.89), 0.19, 0.19, 0.08, 8, "Energia")
m.exportar()

# Antena de comunicaciones con brazos y una luz en la punta.
m = Modelo("mecanico_antena")
m.cono((0, 0, 0.10), 0.36, 0.28, 0.20, 8, "MetalOscuro")
m.cono((0, 0, 0.78), 0.15, 0.10, 1.16, 8, "Metal")
m.cono((0, 0, 1.43), 0.20, 0.20, 0.14, 8, "Bronce")
for lado in (-1, 1):
    m.caja((lado * 0.32, 0, 0.90), (0.48, 0.12, 0.12), "MetalOscuro")
    m.caja((lado * 0.54, 0, 1.06), (0.12, 0.12, 0.40), "Bronce")
m.esfera((0, 0, 1.62), (0.15, 0.15, 0.15), 6, 4, "Energia")
m.exportar()
