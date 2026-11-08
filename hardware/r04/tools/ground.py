#!/usr/bin/env python3
# SPDX-License-Identifier: CERN-OHL-S-2.0
"""Explicit SMD ground fanout to the inner references; never via-in-pad.

The router treats power planes as connections, but an SMD pad still needs
a physical via. Build those short connections and independently run KiCad
DRC afterwards. A failed placement raises, never deletes the missing net.
"""
from pathlib import Path
import collections, heapq, json, math, traceback
import pcbnew as p

R=Path(__file__).resolve().parents[1];PATH=R/'kicad/OpenKeys.kicad_pcb'
def xy(a):return (p.ToMM(a.x)-100,p.ToMM(a.y)-80)
def v(a):return p.VECTOR2I(p.FromMM(a[0]+100),p.FromMM(a[1]+80))
def distance(a,b):return math.hypot(a[0]-b[0],a[1]-b[1])
def main():
 b=p.LoadBoard(str(PATH));gnd=b.FindNet('GND')
 for t in list(b.GetTracks()):
  if t.GetNetname()=='GND':b.RemoveNative(t)
 # The antenna's B.Cu underpass must also exist as an electrical track;
 # footprint graphics alone do not establish KiCad pad connectivity.
 antenna=next(f for f in b.GetFootprints() if f.GetReference()=='J4')
 ends=[z.GetPosition() for z in antenna.Pads() if z.GetNumber()=='2']
 assert len(ends)==2
 z=p.PCB_TRACK(b);z.SetStart(ends[0]);z.SetEnd(ends[1]);z.SetWidth(p.FromMM(.5));z.SetLayer(p.B_Cu);z.SetNet(b.FindNet('ANT2'));b.Add(z)
 # Fast spatial inventory of real copper objects. HitTest uses KiCad's
 # geometry, then the final DRC performs its independent exact checks.
 bins=collections.defaultdict(list);allpads=[];drills=[]
 def add(obj,layer,net):
  bb=obj.GetBoundingBox();a=xy(bb.GetPosition());c=xy(bb.GetEnd())
  for xx in range(math.floor((a[0]-.65)/2),math.floor((c[0]+.65)/2)+1):
   for yy in range(math.floor((a[1]-.65)/2),math.floor((c[1]+.65)/2)+1):bins[layer,xx,yy].append((obj,net))
 enabled=(p.F_Cu,p.In1_Cu,p.In2_Cu,p.B_Cu)
 for f in b.GetFootprints():
  for pad in f.Pads():
   layers=[x for x in enabled if pad.IsOnLayer(x)]
   if layers:allpads.append(pad)
   if pad.GetAttribute()==p.PAD_ATTRIB_NPTH:drills.append(pad)
   for layer in layers:add(pad,layer,pad.GetNetname())
  for shape in f.GraphicalItems():
   if shape.GetLayer() in enabled:add(shape,shape.GetLayer(),'UNASSIGNED_COPPER')
 for t in b.GetTracks():
  for layer in enabled:
   if t.IsOnLayer(layer):add(t,layer,t.GetNetname())
  if t.GetClass()=='PCB_VIA':drills.append(t)
 def nearby(pt,layer):return bins.get((layer,math.floor(pt[0]/2),math.floor(pt[1]/2)),())
 def clear_point(pt,layer,margin):
  pos=v(pt)
  return not any(net!='GND' and item.HitTest(pos,p.FromMM(margin)) for item,net in nearby(pt,layer))
 def legal_via(pt):
  if not (.7<pt[0]<67.3 and .7<pt[1]<72.65):return False
  if not all(clear_point(pt,layer,.48) for layer in enabled):return False
  # Avoid a hole in ANY solderable SMD land, including same-net ground.
  pos=v(pt)
  for layer in (p.F_Cu,p.B_Cu):
   for item,_ in nearby(pt,layer):
    # Same-net copper may meet the via annulus, but the0.30mm plated hole
    # stays at least0.15mm from the SMD land. This is not a hole-in-pad and
    # does not require filled/capped vias. Other nets retain0.18mm Cu gap.
    if item.GetClass()=='PAD' and item.GetAttribute()==p.PAD_ATTRIB_SMD and item.HitTest(pos,p.FromMM(.30)):return False
  for hole in drills:
   if hole.GetClass()=='PCB_VIA':
    if distance(pt,xy(hole.GetPosition()))<.57:return False
   elif hole.HitTest(pos,p.FromMM(.56)):return False
  return True
 def legal_path(path,layer):
  # Sampled with an extra40um conservative clearance, followed by exact DRC.
  for a,c in zip(path,path[1:]):
   steps=max(1,math.ceil(distance(a,c)/.075))
   for i in range(steps+1):
    pt=(a[0]+(c[0]-a[0])*i/steps,a[1]+(c[1]-a[1])*i/steps)
    if not clear_point(pt,layer,.265):return False
  return True
 def paths(a,c):
  # Prefer orthogonal escapes from dense pads, not a diagonal toward a
  # neighbouring signal pin. Inside a large EP an offset anchor is legal.
  yield [a,(c[0],a[1]),c]
  yield [a,(a[0],c[1]),c]
  dx,dy=c[0]-a[0],c[1]-a[1]
  s=min(abs(dx),abs(dy));sx=1 if dx>=0 else -1;sy=1 if dy>=0 else -1
  yield [a,(a[0]+sx*s,a[1]+sy*s),c]
  yield [a,(c[0]-sx*s,c[1]-sy*s),c]
 def trace(path,layer):
  for a,c in zip(path,path[1:]):
   if distance(a,c)<.001:continue
   z=p.PCB_TRACK(b);z.SetStart(v(a));z.SetEnd(v(c));z.SetLayer(layer);z.SetWidth(p.FromMM(.15));z.SetNet(gnd);b.Add(z)
 def maze_escape(center,layer):
  """Short local eight-direction search, including legal paths across EPs."""
  queue=[(0.,(0,0))];cost={(0,0):0.};parent={};seen=set()
  def point(q):return (center[0]+q[0]*.1,center[1]+q[1]*.1)
  while queue:
   score,node=heapq.heappop(queue)
   if node in seen:continue
   seen.add(node);pt=point(node)
   if legal_via(pt):
    chain=[node]
    while chain[-1]!=(0,0):chain.append(parent[chain[-1]])
    chain.reverse();out=[point(chain[0])]
    for i in range(1,len(chain)-1):
     before=(chain[i][0]-chain[i-1][0],chain[i][1]-chain[i-1][1]);after=(chain[i+1][0]-chain[i][0],chain[i+1][1]-chain[i][1])
     if before!=after:out.append(point(chain[i]))
    out.append(pt);return out,pt
   for dx,dy in [(-1,0),(1,0),(0,-1),(0,1),(-1,-1),(1,-1),(-1,1),(1,1)]:
    nxt=(node[0]+dx,node[1]+dy)
    if nxt in seen or abs(nxt[0])>45 or abs(nxt[1])>45:continue
    newcost=score+math.hypot(dx,dy)*.1
    if newcost>=cost.get(nxt,1e6):continue
    dest=point(nxt);mid=((pt[0]+dest[0])/2,(pt[1]+dest[1])/2)
    if not clear_point(dest,layer,.265) or not clear_point(mid,layer,.265):continue
    cost[nxt]=newcost;parent[nxt]=node;heapq.heappush(queue,(newcost,nxt))
  print('No local ground route; visited',len(seen),'nodes; start blockers',[(item.GetClass(),net) for item,net in nearby(center,layer) if net!='GND' and item.HitTest(v(center),p.FromMM(.265))],flush=True)
  return None
 pads=[z for z in allpads if z.GetNetname()=='GND' and z.GetAttribute()==p.PAD_ATTRIB_SMD]
 pads.sort(key=lambda z:z.GetSize().x*z.GetSize().y)
 placed=[];report=[]
 for pad in pads:
  center=xy(pad.GetPosition());layer=next(x for x in (p.F_Cu,p.B_Cu) if pad.IsOnLayer(x));anchors=[center]
  # Offset starts stay inside the same GND copper pad; useful for exposed
  # pads that must escape through a same-net edge pin rather than a hole.
  for dx,dy in [(x,y) for x in (-.4,0,.4) for y in (-.4,0,.4) if x or y]:
   a=(center[0]+dx,center[1]+dy)
   if pad.HitTest(v(a)):anchors.append(a)
  solved=False
  candidates=[(pt,False) for pt in placed if distance(center,pt)<2.2]
  for radius in (.95,1.15,1.4,1.7,2.0,2.4,2.9,3.4,4.0):
   for k in range(16):
    angle=k*math.pi/8;pt=(round(center[0]+radius*math.cos(angle),4),round(center[1]+radius*math.sin(angle),4))
    candidates.append((pt,True))
  candidates.sort(key=lambda q:distance(center,q[0])+(0 if not q[1] else .1))
  for end,new in candidates:
   if new and not legal_via(end):continue
   for a in anchors:
    for path in paths(a,end):
     if not legal_path(path,layer):continue
     if new:
      via=p.PCB_VIA(b);via.SetPosition(v(end));via.SetWidth(p.F_Cu,p.FromMM(.6));via.SetDrill(p.FromMM(.3));via.SetViaType(p.VIATYPE_THROUGH);via.SetLayerPair(p.F_Cu,p.B_Cu);via.SetNet(gnd);b.Add(via);drills.append(via);placed.append(end)
     trace(path,layer);report.append({'ref':pad.GetParentFootprint().GetReference(),'pad':pad.GetNumber(),'via_mm':end,'new_via':new,'trace_mm':sum(distance(x,y) for x,y in zip(path,path[1:]))});solved=True;break
    if solved:break
   if solved:break
  if not solved:
   escape=maze_escape(center,layer)
   if escape:
    path,end=escape
    via=p.PCB_VIA(b);via.SetPosition(v(end));via.SetWidth(p.F_Cu,p.FromMM(.6));via.SetDrill(p.FromMM(.3));via.SetViaType(p.VIATYPE_THROUGH);via.SetLayerPair(p.F_Cu,p.B_Cu);via.SetNet(gnd);b.Add(via);drills.append(via);placed.append(end)
    trace(path,layer);report.append({'ref':pad.GetParentFootprint().GetReference(),'pad':pad.GetNumber(),'via_mm':end,'new_via':True,'trace_mm':sum(distance(x,y) for x,y in zip(path,path[1:]))});solved=True
   else:raise RuntimeError(f'No safe ground escape: {pad.GetParentFootprint().GetReference()}.{pad.GetNumber()} {center}')
 if not p.ZONE_FILLER(b).Fill(b.Zones()):raise RuntimeError('Zone fill failed')
 p.SaveBoard(str(PATH),b)
 (R/'review/ground-fanout.json').write_text(json.dumps({'revision':'0.4','smd_ground_pads':len(pads),'new_vias':len(placed),'connections':report,'requires_independent_drc':True},indent=2)+'\n')
 print(f'Connected {len(pads)} SMD GND lands using {len(placed)} explicit 0.6/0.3mm vias; no via-in-pad; run DRC')
if __name__=='__main__':
 try:main()
 except Exception as error:
  print(type(error).__name__,str(error),flush=True)
  frame=error.__traceback__
  while frame:
   print(frame.tb_frame.f_code.co_filename,frame.tb_lineno,flush=True);frame=frame.tb_next
  raise SystemExit(1)
