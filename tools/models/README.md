# Model preparation (Blender)

Reproducible preparation of the `.glb` models in `assets/models/` from the CC BY 4.0 Sketchfab sources (see `CREDITS.md`).
Download the three source models (GLB) into `assets/models/incoming/` (ignored by git) as `dualsense.glb`, `dualshock4.glb`, `xbox.glb`, then:

```
blender -b -P tools/models/prep_dualsense.py -- --in assets/models/incoming/dualsense.glb --out assets/models/dualsense.glb
blender -b -P tools/models/prep_dualsense.py -- --in assets/models/incoming/dualsense.glb --out assets/models/dualsense_edge.glb --edge 1
blender -b -P tools/models/prep_ds4.py       -- --in assets/models/incoming/dualshock4.glb --out assets/models/dualshock4.glb
blender -b -P tools/models/prep_xbox.py      -- --in assets/models/incoming/xbox.glb --out assets/models/xbox.glb
blender -b -P tools/models/prep_xbox.py      -- --in assets/models/incoming/xbox.glb --out assets/models/xbox_elite.glb --elite 1
```

`prep_lib.py` / `parts_split.py` are shared helpers; `analyze*.py`, `islands.py`, `inspect.py`, `bisect.py` are exploration helpers used to find part boundaries.
