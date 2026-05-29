# rust-neveredit — Agent Status

## Toolbar (3 scrollable bars)
- **File**: Open, Recent, Save, SaveAs, Props, Minimap export, Launch NWN, Quit, Modified indicator
- **View**: Window toggles (Res, BP, Items, Search, Tree, Con, Palette, Graph, Height, Nav, Mini, Sky) | render toggles (Tiles, Grid, Light, Fog, Walk, WEdges, Paint, Sprite, Water, Snap) | marks toggles (Creature, Doors, Placeable, Trigger, Waypoint, Encounter, Sound, Light) | "..." overflow
- **Tools**: Editor toggles (Env, Factions, Journal, Enc, Tex, Sound, Store, Whois, Geom, Find, Trans) | edit actions (New, Copy, Paste, Del) | Undo, Redo | bookmarks + NWSync (right-aligned)
- **Pattern**: `TopBottomPanel::top` + `ScrollArea::horizontal`, 1px item spacing, `label_toggle`/`label_button` (text + icon at 12px), `icon_button` (16px) for utility buttons.

## Rendering Fixes
- **Minimap overlay** & **ghost preview**: Fixed — were hidden behind 3D paint callback (egui renders shapes before paint callbacks in same layer). Now render in `Order::Foreground` layer.

## Data Loading
- **Faction/Journal editors**: Fallback lookups (`repute.fac`, `module.jrl`) since NWN modules use these names.
- **Appearance Picker**: Uses `resource_manager.get_2da("appearance")`.
- **`get_resource_data`**: Hak fallback + FAC/JRL alias swaps.
- **`resource_list`**: Includes hak resources.
- **Transition Editor**: ERF fallback for GIT data.
- **Encounter Editor**: `selected_idx` persisted.

## Known Issues
- Area Geometry Editor empty if area has no triggers/encounters
- Audio resources (WAV/BMU) missing from ERF (game data / haks only)

## Key Files
- `aurora-borealis/src/lib.rs` — main app
- `aurora-borealis/src/icons.rs` — `label_toggle`, `label_button`, `text_icon_layout`
- `nwn-gamemodel/src/module.rs` — `get_resource_data`, `resource_list`
