// SPDX-License-Identifier: CERN-OHL-S-2.0
// Millimetres. PCB coordinates are translated by [3,3,11].
// Source-based fit candidate: qualify actual LCD/FP/flex, switch travel and
// printed tolerances before machining three final enclosures.
part="assembly"; $fn=64;
W=74; H=98; D=20; wall=1.8; floor_t=1.6; lid_t=1.6; board_z=11;
screws=[[6,6],[68,6],[6,70],[68,70]];
buttons=[[13,41.5],[26,41.5],[13,62],[26,62]];
fp=[52.3,51.3]; fp_bosses=[[52.3,40.3],[52.3,62.3]];
// Nominal E-Switch TL3342 actuator top=PCB top+1.54mm. Set the
// plunger in a real assembly jig; the moulded stop is not a travel guarantee.
switch_top=board_z+1+1.54;
plunger_tip=switch_top+.10;

module rr(w,h,r,z){hull(){for(x=[r,w-r])for(y=[r,h-r])translate([x,y,0])cylinder(r=r,h=z);}}
module board_shape(h=1){linear_extrude(height=h)polygon([[0,0],[28,0],[28,-2.5],[40,-2.5],[40,0],[68,0],[68,92],[0,92]]);}
module base(){
 difference(){
  union(){
   difference(){rr(W,H,5,D-lid_t);translate([wall,wall,floor_t])rr(W-2*wall,H-2*wall,3.2,D);}
   for(p=screws)translate([p[0],p[1],floor_t])cylinder(d=5.2,h=board_z-floor_t);
   // Pouch support ribs stay out of the RF region and do not compress it.
   for(x=[18,50])translate([x,36,floor_t])cube([.8,38,4]);
   translate([18,35.5,floor_t])cube([32.8,.8,3]);
   translate([18,74,floor_t])cube([32.8,.8,3]);
   // Receptacle rear baffle; no open service passage into the secure area.
   translate([31,8.1,12.8])cube([12,1.2,5]);
   for(x=[31,42])translate([x,1.7,12.8])cube([1,7.6,5]);
  }
  for(p=screws)translate([p[0],p[1],floor_t+1])cylinder(d=1.6,h=board_z);
  // Single external data/charging socket; no debug/battery apertures.
  translate([32.1,-1,11.8])cube([9.8,4,3.7]);
  // Mechanical power switch actuator; not an external electrical port.
  translate([-1,19,11.8])cube([4,8,3.2]);
 }
}
module lid(){
 difference(){
  union(){
   translate([0,0,D-lid_t])rr(W,H,5,lid_t);
   translate([2.05,2.05,D-lid_t-1.2])difference(){rr(W-4.1,H-4.1,3,1.4);translate([1,1,-.1])rr(W-6.1,H-6.1,2,1.6);}
   for(p=screws)translate([p[0],p[1],board_z+1])cylinder(d=4.6,h=D-lid_t-board_z-1);
   // Captive pushrod guides terminate above the switch actuator.
   for(p=buttons){
    translate([p[0],p[1],13.95])difference(){cylinder(d=4.4,h=4.25);translate([0,0,-.1])cylinder(d=2.7,h=5);}
    // Annular bridge attaches the guide to the lid outside the key bore;
    // the inner stop remains at18.2mm,0.30mm below the neutral flange.
    translate([p[0],p[1],17.7])difference(){cylinder(d=9,h=.8);translate([0,0,-.1])cylinder(d=2.7,h=1);}
   }
   // SW6 lid contact, adjustable M2 insulating set screw in blind pilot.
   translate([9,56,15.8])cylinder(d=5.2,h=2.6);
   for(p=fp_bosses)translate([p[0],p[1],18.1])cylinder(d=4.6,h=.5);
  }
  // Clear PC window:49.0x17.6x0.8, hard-coated, bonded on the ledge.
  translate([13.65,11.5,17.8])cube([46.7,15.3,4]);
  translate([12.5,10.35,19.2])cube([49,17.6,1.2]);
  // FPC2534AM:14.0mm flange,12.1mm raised face,1.5mm maximum body.
  translate([fp[0],fp[1],18.25])cylinder(d=12.4,h=3);
  translate([fp[0],fp[1],18.25])cylinder(d=14.6,h=1.05);
  for(p=buttons){
   translate([p[0],p[1],18.2])cylinder(d=6.2,h=3);
   translate([p[0],p[1],18.2])cylinder(d=7.1,h=.95);
  }
  for(p=screws){translate([p[0],p[1],11.9])cylinder(d=2.4,h=9);translate([p[0],p[1],18.8])cylinder(d=4.3,h=1.4);}
  for(p=fp_bosses)translate([p[0],p[1],18.05])cylinder(d=1.25,h=1.85);
  translate([9,56,15.7])cylinder(d=1.6,h=3.3);
 }
}
module fp_retainer(){
 difference(){
  union(){
   translate([fp[0],fp[1],16.9])difference(){cylinder(d=16.6,h=1.2);translate([0,0,-.1])cylinder(d=10.5,h=1.4);}
   for(p=fp_bosses)hull(){translate([p[0],p[1],16.9])cylinder(d=4.4,h=1.2);translate([fp[0],fp[1],16.9])cylinder(d=4.4,h=1.2);}
  }
  // Leftward flex exit; never crease, clamp or glue the active flex bend.
  translate([37,49.3,16.8])cube([15.5,4,2]);
  for(p=fp_bosses)translate([p[0],p[1],16.8])cylinder(d=1.8,h=1.5);
 }
}
module plunger(){
 // Neutral top20.25, tip13.64; mechanical-stop target0.30mm.
 union(){
  translate([0,0,19.05])cylinder(d=5.6,h=1.2);
  translate([0,0,18.5])cylinder(d=6.8,h=.55);
  translate([0,0,plunger_tip])cylinder(d=2.2,h=19.1-plunger_tip);
 }
}
module components(){
 color("seagreen")translate([3,3,board_z])board_shape();
 color("silver")translate([19.5,37,1.8])cube([29,36,4.75]);
 // LCD overall top lip begins0.8mm left of the lower glass edge.
 color("ivory")translate([11.35,10,12.5])cube([56.8,18.9,5.5]);
 color("black")translate([12.15,28.9,17.5])cube([49.7,7.45,.4]);
 color([.93,.97,.94])translate([14.15,12.9,18.05])cube([45.7,14.3,.1]);
 color([.8,.9,.95,.45])translate([12.5,10.35,19.2])cube([49,17.6,.8]);
 color("black")translate([fp[0],fp[1],18.6])cylinder(d=14,h=.7);
 color("gray")translate([fp[0],fp[1],19.3])cylinder(d=12.1,h=.8);
 color("silver")translate([32.53,-.15,12])cube([8.94,7.35,3.16]);
 // Connector body/exclusion envelopes, not substitutes for vendor models.
 color("black")translate([21.8,46.65,12])cube([5.6,9.3,2]);
 color([.8,.4,.12])translate([27.4,48.05,16])cube([17.9,6.5,.12]);
}
// KiCad/dimension drawings use Y down. Manufacturing 3D models use a
// right-handed frame with Z out of the front: X=x, Y=H-y, Z=z.
module selected_part(){
if(part=="base")base();
else if(part=="lid")lid();
else if(part=="lid-print")translate([0,H,D])rotate([180,0,0])lid();
else if(part=="retainer")translate([-44,-38,-16.9])fp_retainer();
else if(part=="plungers")for(i=[0:3])translate([i*10,0,-plunger_tip])plunger();
else{
 color([.12,.18,.24])base();components();
 color([.12,.18,.24])lid();color([.1,.55,.6])for(p=buttons)translate([p[0],p[1],0])plunger();
 color("gray")fp_retainer();
}
}
translate([0,H,0])mirror([0,1,0])selected_part();
