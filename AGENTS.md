# rust-neveredit — Agent Status

## Toolbar & UI (May 29 2026)
- **4 scrollable bars** (File/View/Tools/Edit) replacing dropdown menus — each with `ScrollArea::horizontal` and tight 1px spacing.
- **Text+icon buttons**: `label_toggle` (highlighted when active) and `label_button` (momentary) in `icons.rs`.
- **File bar**: Open, Recent (dropdown), Save, SaveAs, Props, Minimap export, Launch NWN, Quit, Modified indicator.
- **View bar**: Res, BP, Items, Search, Tree, Con(contents), Palette, Graph, Height, Nav, Mini, Sky toggles; then render toggles (Tiles, Grid, Light, Fog, Walk, WEdges, Paint, Sprite, Water, Snap); Marks popup; "..." popup (Generate Walls, Water/Grid/Texture sliders).
- **Tools bar**: Env, Factions, Journal, Enc, Tex, Sound, Store, Whois, Geom, Find, Trans toggles; Undo/Redo; Camera bookmarks + NWSync export (right-aligned).
- **Edit bar**: New, Copy, Paste, Del (area-conditional).

## Rendering Fixes
- **Minimap overlay**: Fixed — was hidden behind 3D viewport paint callback. Now renders in `Order::Foreground` layer (above the 3D scene).
- **Ghost placement preview**: Same fix — rendered in foreground layer.

## Data Loading Fixes (previous session)
- **Faction Editor**: Fallback lookups for `"repute"` / `"Repute"` (NWN modules store factions as `repute.fac`).
- **Journal Editor**: Fallback lookups for `"module"` / `"Module"` (NWN modules store journal as `module.jrl`).
- **Appearance Picker**: Uses `resource_manager.get_2da("appearance")` instead of raw ERF.
- **`get_resource_data`** (`nwn-gamemodel/src/module.rs`): Hak fallback + FAC/JRL alias swaps.
- **`resource_list`**: Includes hak resources via `hak_index.all_resources()`.
- **Transition Editor**: ERF fallback for GIT data.
- **Encounter Editor**: `selected_idx` persisted.
- **Area search bar**: Text filter in left sidebar.

## Known Issues
- Area Geometry Editor may show empty list if area has no triggers/encounters
- All audio resources (WAV/BMU) expected to be missing from module ERF (game data / haks only)

## Relevant Files
- `aurora-borealis/src/lib.rs` — main app, all toolbars + windows
- `aurora-borealis/src/icons.rs` — `label_toggle`, `label_button`, `text_icon_layout`, `icon_toggle`
- `nwn-gamemodel/src/module.rs` — `get_resource_data`, `resource_list`
- `aurora-borealis/src/faction_editor.rs` — `"repute"` fallback
- `aurora-borealis/src/encounter_editor.rs` — `&mut Option<usize>` parameter
