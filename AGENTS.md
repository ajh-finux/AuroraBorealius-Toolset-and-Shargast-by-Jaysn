# rust-neveredit — Agent Status

## Recent Changes (Session May 29 2026)

### Data Loading Fixes
- **Faction Editor**: Added fallback lookups for `"repute"` / `"Repute"` (NWN modules store factions as `repute.fac`, not `faction.fac`).
- **Journal Editor**: Added fallback lookups for `"module"` / `"Module"` (NWN modules store journal as `module.jrl`, not `journal.jrl`).
- **Appearance Picker**: Changed from raw ERF lookup to `resource_manager.get_2da("appearance")`.
- **`get_resource_data`** (`nwn-gamemodel/src/module.rs`): Added hak fallback; added FAC alias swap (`faction` ↔ `repute`) and JRL alias swap (`journal` ↔ `module`).
- **`resource_list`**: Now includes hak resources via `hak_index.all_resources()`.
- **Resource Viewer "Edit" buttons**: Now actually open the respective editors (Faction/Journal).

### Editor Improvements
- **Encounter Editor**: `selected_idx` persisted via `encounter_editor_selected: Option<usize>` field instead of resetting every frame.
- **Area search bar**: Added text filter in left sidebar area list.
- **Transition Editor**: Added ERF fallback for GIT data (was only reading from `raw_git`, which starts empty).

### Rendering
- **Minimap overlay**: Renders actual tile color grid on viewport (bottom-right corner), independent of Navigator bar.
- **Visual Tileset Browser removed** (caused freezing from sync model/texture loading).

### Files Modified
- `aurora-borealis/src/lib.rs` — main app: journal/faction/appearance fixes, encounter selection, area search, minimap, transition editor ERF fallback, removed tileset_browser references
- `aurora-borealis/src/faction_editor.rs` — `"repute"` fallback
- `aurora-borealis/src/encounter_editor.rs` — `&mut Option<usize>` parameter
- `aurora-borealis/src/tileset_browser.rs` — **deleted**
- `nwn-gamemodel/src/module.rs` — `resource_list()` hak merge, `get_resource_data()` alias swaps + hak fallback
- `nwn-gamemodel/src/lib.rs`, `nwn-3d/src/pipeline.rs`, `nwn-3d/src/tile_renderer.rs`, `nwn-3d/src/viewport.rs` — minimap support
- `nwn-3d/Cargo.toml`, `nwn-tileset/Cargo.toml`, `aurora-borealis/Cargo.toml` — dependency updates
- `nwn-tileset/src/lib.rs`, `nwn-gamemodel/src/area.rs` — light support

### Known Issues
- Minimap overlay still shows nothing on some configs (check `show_minimap` checkbox wiring)
- Area Geometry Editor may show empty list if area has no triggers/encounters
- All audio resources (WAV/BMU) expected to be missing from module ERF (game data / haks only)
