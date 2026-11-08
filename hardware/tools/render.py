# SPDX-License-Identifier: CERN-OHL-S-2.0
# Run with: blender -b -t 2 -P hardware/tools/render.py
import bpy, math
from pathlib import Path
from mathutils import Vector
R=Path(__file__).resolve().parents[1]
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
def material(name,c,metal=0):
 m=bpy.data.materials.new(name);m.diffuse_color=(*c,1);m.use_nodes=True
 p=m.node_tree.nodes.get('Principled BSDF');p.inputs['Base Color'].default_value=(*c,1);p.inputs['Metallic'].default_value=metal;p.inputs['Roughness'].default_value=.34
 return m
shell=material('Graphite PETG',(.055,.095,.14));accent=material('Teal buttons',(.01,.52,.56));board=material('PCB green',(.035,.22,.13));metal=material('USB metal',(.65,.72,.78),.8);black=material('Display glass',(.004,.012,.017));white=material('OLED ink',(.45,.95,.8));sensor=material('Fingerprint module envelope',(.1,.15,.2));battery=material('Battery envelope',(.56,.62,.67),.5);copper=material('RF illustrative',(.68,.35,.08),.4)
def cube(name,pos,size,mat,bevel=.2):
 bpy.ops.mesh.primitive_cube_add(size=1,location=(pos[0]+size[0]/2,pos[1]+size[1]/2,pos[2]+size[2]/2));o=bpy.context.object;o.name=name;o.dimensions=size;bpy.ops.object.transform_apply(location=False,rotation=False,scale=True);o.data.materials.append(mat)
 if bevel:
  mod=o.modifiers.new('Machined edges','BEVEL');mod.width=bevel;mod.segments=3;o.modifiers.new('Normals','WEIGHTED_NORMAL')
 return o
def stl(name,mat):
 bpy.ops.wm.stl_import(filepath=str(R/'exports'/('OpenKeys-'+name+'.stl')));o=bpy.context.object;o.name=name;o.data.materials.append(mat);
 for poly in o.data.polygons:poly.use_smooth=False
 return o
base=stl('base',shell);lid=stl('lid',shell);lid.rotation_euler=(math.pi,0,0);lid.location=(0,88,16)
baseparts=[]
baseparts.append(cube('PCB envelope (not routed)',(3,3,6.4),(38,82,1),board))
baseparts.append(cube('Protected cell envelope',(12,31,1.8),(20,30,4),battery))
baseparts.append(cube('OLED PCB envelope',(6,7,10.8),(32,25,2.4),black))
screen=cube('OLED glass',(7.5,8.5,13.2),(29,17,.8),black,.35);baseparts.append(screen)
baseparts.append(cube('Trusted fingerprint TBD',(22,35,11.8),(16,16,4),sensor,.8))
baseparts.append(cube('USB-A envelope',(16,-12,6),(12,17,4.5),metal,.35));baseparts.append(cube('USB-A insert',(17,-12.2,7),(10,2,2.5),black,.1))
baseparts.append(cube('USB-C envelope',(37,18,5.5),(7.8,9,4.5),metal,.5));baseparts.append(cube('USB-C recess',(44.8,19,6.4),(.2,7,2.6),black,.6))
for p in [(8,38),(16,38),(8,48),(16,48)]:
 bpy.ops.mesh.primitive_cylinder_add(vertices=48,radius=2.6,depth=2.1,location=(*p,15.25));o=bpy.context.object;o.name='captive button';o.data.materials.append(accent)
for p in [(6,6),(38,6),(6,60),(38,60)]:
 bpy.ops.mesh.primitive_cylinder_add(vertices=32,radius=1.55,depth=.4,location=(*p,15.8));o=bpy.context.object;o.name='M2 screw';o.data.materials.append(metal)
 cube('Screw slot',(p[0]-1,p[1]-.13,16.02),(2,.26,.05),black,0)
font=bpy.data.fonts.load('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf')
def txt(body,x,y,z,size,mat=white):
 c=bpy.data.curves.new(body,'FONT');c.body=body;c.font=font;c.size=size;c.align_x='CENTER';c.extrude=.008
 o=bpy.data.objects.new(body,c);bpy.context.collection.objects.link(o);o.location=(x,y,z);o.rotation_euler=(0,0,math.pi);o.data.materials.append(mat);return o
txt('example.com',22,13,14.05,2.1);txt('123 456',22,19,14.05,3.6);txt('UNLOCKED',22,23,14.05,1.6)
txt('OPENKEYS',22,62,16.05,2.6);txt('NFC',22,77,16.05,3.4)
for i in range(5):
 pts=[]
 for j in range(41):
  a=math.radians(-150+300*j/40);pts.append((30+(3+i*.5)*math.cos(a),43+(4+i*.5)*math.sin(a),15.92))
 curve=bpy.data.curves.new('Fingerprint pattern','CURVE');curve.dimensions='3D';curve.bevel_depth=.07;curve.bevel_resolution=2;sp=curve.splines.new('POLY');sp.points.add(len(pts)-1)
 for p,co in zip(sp.points,pts):p.co=(*co,1)
 ob=bpy.data.objects.new('Fingerprint pattern',curve);bpy.context.collection.objects.link(ob);ob.data.materials.append(metal)
# RF area copper not rendered: the coil has not been tuned/released.
cube('Table',(-100,-100,-2),(250,300,1.9),material('Backdrop',(.82,.87,.9)),.5)
world=bpy.context.scene.world;world.color=(.25,.25,.25)
def light(loc,power,size):
 bpy.ops.object.light_add(type='AREA',location=loc);o=bpy.context.object;o.data.energy=power;o.data.shape='DISK';o.data.size=size
 o.rotation_euler=(Vector((22,40,5))-o.location).to_track_quat('-Z','Y').to_euler()
light((40,0,180),180000,150);light((-80,80,100),110000,130)
bpy.ops.object.camera_add(location=(140,160,220));cam=bpy.context.object;cam.rotation_euler=(Vector((22,37,7))-cam.location).to_track_quat('-Z','Y').to_euler();cam.data.type='ORTHO';cam.data.ortho_scale=130;bpy.context.scene.camera=cam
scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.samples=32;scene.cycles.use_denoising=False;scene.render.resolution_x=1400;scene.render.resolution_y=1400;scene.render.resolution_percentage=100
scene.view_settings.view_transform='AgX';scene.render.image_settings.file_format='PNG';scene.render.filepath=str(R/'exports/OpenKeys-assembly.png')
bpy.ops.wm.save_as_mainfile(filepath=str(R/'mechanical/OpenKeys-assembly.blend'));bpy.ops.render.render(write_still=True)
