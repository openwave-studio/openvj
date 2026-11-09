# Fixtures

`UpscaleComp.avc` is a composition saved by Resolume Arena 7.22.4. It comes from the `test-data` folder of
[Resolume Composition Converter](https://github.com/tijnisfijn/Resolume-Composition-Converter) and is used here
under its MIT license (see `LICENSE-UpscaleComp`).

Its media paths point at the original author's machine, so the importer tests only check that clips are placed
and named correctly. They never try to open the media.

`TwoScreens.xml` is an Advanced Output preset saved by Resolume Arena 7.13.2, with hand-warped 7×7 and 25×13 grids
and a corner pin. It comes from [touchdesigner-utils-pack](https://github.com/vecnode/touchdesigner-utils-pack)
(`data-utilities/resolume-xml-parser/example-resolume-export.xml`) and is used under its MIT license (see
`LICENSE-TwoScreens`).

`DisplayAndSpout.xml` is an Advanced Output preset saved by Resolume Avenue 7.22.9 with a display output and two
Spout outputs. It comes from [palette](https://github.com/vizicist/palette)
(`data_default/config/PaletteAdvancedOutputV9.xml`) and is used under its MIT license (see `LICENSE-DisplayAndSpout`).

`MaskedShapes.xml`, `BezierBulbs.xml` and `CurvedMask.xml` are made-up presets in the shape Resolume Arena 7.15
saves them, written by `make_presets.py` for these tests. Masked Shapes is one slice under three inverted screen masks (a
hexagon and two triangles), Bezier Bulbs is three slices bent with 4×4 `PM_BEZIER` control nets, and Curved Mask is a
slice under one inverted mask made of six cubic segments. Bezier Bulbs' output rects are the bounding boxes of the
bezier surfaces, worked out by sampling the patches outside openVJ, so the tests check openVJ's warp maths against
them.
