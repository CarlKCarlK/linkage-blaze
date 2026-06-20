; Mini boat — sample G-code for linkage-blaze-printer-wasm tests
; Five-layer boat hull, deck, cabin, and roof
; Build volume: 220 x 220 x 250 mm  Layer height: 0.2 mm
G90       ; absolute XYZ
M82       ; absolute extruder

; ── Layer 1: hull outline at Z=0.2 ───────────────────────────────────────────
G0 Z0.2 F3000
G0 X30 Y20 F6000
G1 X30  Y20  E0    F1500
G1 X90  Y20  E3.6
G1 X110 Y50  E6.6
G1 X90  Y80  E9.6
G1 X30  Y80  E13.2
G1 X10  Y50  E16.2
G1 X30  Y20  E19.2

; ── Layer 2: hull outline at Z=0.4 ───────────────────────────────────────────
G0 Z0.4 F3000
G0 X30 Y20 F6000
G1 X30  Y20  E19.2 F1500
G1 X90  Y20  E22.8
G1 X110 Y50  E25.8
G1 X90  Y80  E28.8
G1 X30  Y80  E32.4
G1 X10  Y50  E35.4
G1 X30  Y20  E38.4

; ── Layer 3: deck at Z=0.6 ───────────────────────────────────────────────────
G0 Z0.6 F3000
G0 X35 Y25 F6000
G1 X35  Y25  E38.4 F1500
G1 X85  Y25  E41.4
G1 X100 Y50  E43.6
G1 X85  Y75  E45.8
G1 X35  Y75  E48.8
G1 X20  Y50  E51.0
G1 X35  Y25  E53.2

; ── Layer 4: cabin at Z=0.8 ──────────────────────────────────────────────────
G0 Z0.8 F3000
G0 X42 Y35 F6000
G1 X42  Y35  E53.2 F1500
G1 X78  Y35  E55.3
G1 X78  Y65  E57.1
G1 X42  Y65  E59.2
G1 X42  Y35  E61.3

; ── Layer 5: cabin roof at Z=1.0 ─────────────────────────────────────────────
G0 Z1.0 F3000
G0 X42 Y35 F6000
G1 X42  Y35  E61.3 F1500
G1 X60  Y25  E63.1
G1 X78  Y35  E64.9
G1 X78  Y65  E66.7
G1 X60  Y75  E68.5
G1 X42  Y65  E70.3
G1 X42  Y35  E72.1
