"""Writes the made-up Advanced Output presets in this folder, in the shape Resolume Arena 7 saves them.

Run from this folder: python3 make_presets.py .
"""
import math, sys
out = sys.argv[1]

def v(p, ind):
    return "".join(f'{ind}<v x="{round(x,3):g}" y="{round(y,3):g}"/>\n' for x, y in p)

def rect(x0, y0, x1, y1):
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]

def bbox(pts):
    xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
    return rect(min(xs), min(ys), max(xs), max(ys))

def head(name):
    return f'''<?xml version="1.0" encoding="utf-8"?>
<XmlState name="{name}">
	<versionInfo name="Resolume Arena" majorVersion="7" minorVersion="15" microVersion="0" revision="1"/>
	<ScreenSetup name="ScreenSetup">
		<Params name="ScreenSetupParams"/>
		<CurrentCompositionTextureSize width="1920" height="1080"/>
		<screens>
			<Screen name="Screen 1" uniqueId="100">
				<Params name="Params">
					<Param name="Name" T="STRING" default="" value="Screen 1"/>
					<Param name="Enabled" T="BOOL" default="1" value="1"/>
				</Params>
				<layers>
'''

TAIL = '''				</layers>
				<OutputDevice>
					<OutputDeviceDisplay name="Display 2" deviceId="Display 2" idHash="1" fullscreen="1" width="1920" height="1080"/>
				</OutputDevice>
			</Screen>
		</screens>
	</ScreenSetup>
</XmlState>
'''

def slice_xml(uid, name, out_rect, grid, mode="PM_LINEAR", flip=0):
    I = "\t" * 5
    return f'''{I}<Slice uniqueId="{uid}">
{I}	<Params name="Common">
{I}		<Param name="Name" T="STRING" default="Layer" value="{name}"/>
{I}		<Param name="Enabled" T="BOOL" default="1" value="1"/>
{I}	</Params>
{I}	<Params name="Input">
{I}		<ParamChoice name="Input Source" default="0:1" value="0:1" storeChoices="0"/>
{I}		<Param name="SoftEdgeEnable" T="BOOL" default="0" value="0"/>
{I}	</Params>
{I}	<Params name="Output">
{I}		<Param name="Flip" T="UINT8" default="0" value="{flip}"/>
{I}	</Params>
{I}	<InputRect orientation="0">
{v(rect(0, 0, 1920, 1080), I + chr(9) * 2)}{I}	</InputRect>
{I}	<OutputRect orientation="0">
{v(out_rect, I + chr(9) * 2)}{I}	</OutputRect>
{I}	<Warper>
{I}		<Params name="Warper">
{I}			<ParamChoice name="Point Mode" default="PM_LINEAR" value="{mode}" storeChoices="0"/>
{I}			<Param name="Flip" T="UINT8" default="0" value="0"/>
{I}		</Params>
{I}		<BezierWarper controlWidth="4" controlHeight="4">
{I}			<vertices>
{v(grid, I + chr(9) * 4)}{I}			</vertices>
{I}		</BezierWarper>
{I}		<Homography>
{I}			<src>
{v(out_rect, I + chr(9) * 4)}{I}			</src>
{I}			<dst>
{v(out_rect, I + chr(9) * 4)}{I}			</dst>
{I}		</Homography>
{I}	</Warper>
{I}</Slice>
'''

def mask_xml(uid, name, pts, segs, mode="PM_LINEAR"):
    I = "\t" * 5
    # Bezier handles can stick out of the shape, but the rects here only frame the mask.
    box = bbox(pts)
    return f'''{I}<Mask uniqueId="{uid}">
{I}	<Params name="Common">
{I}		<Param name="Name" T="STRING" default="Layer" value="{name}"/>
{I}		<Param name="Enabled" T="BOOL" default="1" value="1"/>
{I}	</Params>
{I}	<Params name="Input">
{I}		<Param name="Invert" T="BOOL" default="1" value="1"/>
{I}	</Params>
{I}	<InputRect orientation="0">
{v(rect(-0.5, -0.5, 0.5, 0.5), I + chr(9) * 2)}{I}	</InputRect>
{I}	<OutputRect orientation="0">
{v(box, I + chr(9) * 2)}{I}	</OutputRect>
{I}	<ShapeObject>
{I}		<Params name="Shape">
{I}			<ParamChoice name="Point Mode" default="PM_LINEAR" value="{mode}" storeChoices="0"/>
{I}		</Params>
{I}		<Rect orientation="0">
{v(box, I + chr(9) * 3)}{I}		</Rect>
{I}		<Shape>
{I}			<Contour closed="1">
{I}				<points>
{v(pts, I + chr(9) * 5)}{I}				</points>
{I}				<segments>{segs}</segments>
{I}			</Contour>
{I}		</Shape>
{I}	</ShapeObject>
{I}</Mask>
'''

def identity_grid(r):
    (x0, y0), _, (x1, y1), _ = r
    return [(x0 + (x1 - x0) * c / 3, y0 + (y1 - y0) * rr / 3) for rr in range(4) for c in range(4)]

# 1. Masked shapes: one full-frame slice under three inverted masks.
full = rect(0, 0, 1920, 1080)
hexagon = [(960 + 300 * math.cos(math.radians(a)), 540 + 260 * math.sin(math.radians(a))) for a in range(0, 360, 60)]
left = [(300, 700), (500, 1000), (100, 1000)]
right = [(1620, 700), (1820, 1000), (1420, 1000)]
xml = head("Masked Shapes") + slice_xml(101, "Slice 1", full, identity_grid(full))
xml += mask_xml(102, "Left", left, "LLL") + mask_xml(103, "Hexagon", hexagon, "LLLLLL") + mask_xml(104, "Right", right, "LLL")
open(f"{out}/MaskedShapes.xml", "w").write(xml + TAIL)

# 2. Bezier bulbs: three 4x4 PM_BEZIER control nets. The output rect is the surface's bounding box,
#    worked out here by sampling the bicubic patch, independently of openVJ's own warp code.
def bern(t):
    s = 1 - t
    return [s ** 3, 3 * s * s * t, 3 * s * t * t, t ** 3]

def surface_bbox(net, n=400):
    pts = []
    for i in range(n + 1):
        bv = bern(i / n)
        for j in range(n + 1):
            bu = bern(j / n)
            x = sum(bv[r] * bu[c] * net[r * 4 + c][0] for r in range(4) for c in range(4))
            y = sum(bv[r] * bu[c] * net[r * 4 + c][1] for r in range(4) for c in range(4))
            pts.append((x, y))
    return bbox(pts)

def bulb(x0):
    base = [
        (90, 90), (230, -30), (370, -30), (510, 90),
        (-30, 300), (220, 330), (380, 360), (630, 300),
        (-30, 600), (200, 640), (400, 700), (630, 600),
        (90, 810), (220, 960), (380, 1010), (510, 810),
    ]
    return [(x + x0, y) for x, y in base]

xml = head("Bezier Bulbs")
for uid, (name, x0, flip) in enumerate([("Left", 0, 0), ("Middle", 650, 2), ("Right", 1300, 0)], start=201):
    net = bulb(x0)
    xml += slice_xml(uid, name, surface_bbox(net), net, "PM_BEZIER", flip)
open(f"{out}/BezierBulbs.xml", "w").write(xml + TAIL)

# 3. Curved mask: an ellipse from six cubic segments (start point + two handles each).
cx, cy, rx, ry = 960, 450, 800, 400
k = 4 / 3 * math.tan(math.radians(60) / 4)
pts = []
for s in range(6):
    a0, a1 = math.radians(60 * s), math.radians(60 * (s + 1))
    p0 = (math.cos(a0), math.sin(a0))
    p1 = (math.cos(a1), math.sin(a1))
    h0 = (p0[0] - k * math.sin(a0), p0[1] + k * math.cos(a0))
    h1 = (p1[0] + k * math.sin(a1), p1[1] - k * math.cos(a1))
    pts += [(cx + rx * x, cy + ry * y) for x, y in (p0, h0, h1)]
xml = head("Curved Mask") + slice_xml(301, "Slice 1", full, identity_grid(full)) + mask_xml(302, "Mask 1", pts, "CCCCCC", "PM_BEZIER")
open(f"{out}/CurvedMask.xml", "w").write(xml + TAIL)
