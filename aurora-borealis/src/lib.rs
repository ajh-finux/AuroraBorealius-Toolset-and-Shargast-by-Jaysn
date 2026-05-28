#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
use std::sync::Arc;
use eframe::egui;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use wgpu;
use nwn_gamemodel::FactionList;
use nwn_tlk::TlkFile;
use nwn_3d::{ObjectType, VisibilityFilter};
use nwn_gamemodel::PropertyBag;
use crate::inspector::{SelectedObject, show_inspector, show_bag_editor};
use nwn_gamemodel::resource_types;
use nwn_gamemodel::resource_types::type_to_ext;
use crate::script_editor::show_script_editor;
use crate::conversation_editor::{ConversationEditor, show_conversation_editor};
use crate::resource_viewer::show_resource_viewer;
use crate::blueprint_palette::show_blueprint_palette;
use crate::item_editor::show_item_browser;
use crate::two_da_editor::{TwoDaEditorState, show_two_da_editor};
use crate::texture_browser::{TextureBrowserState, show_texture_browser};
use crate::tileset_browser::{TilesetBrowserState, show_tileset_browser};
use crate::area_geometry_editor::{GeometryEditorState, show_geometry_editor};

mod inspector;
mod script_editor;
mod conversation_editor;
mod resource_viewer;
mod blueprint_palette;
mod item_editor;
mod faction_editor;
mod journal_editor;
mod two_da_editor;
mod encounter_editor;
mod texture_browser;
mod tileset_browser;
mod area_geometry_editor;

// ── Platform-specific file dialog helpers ──────────────────────────────
#[cfg(not(target_arch = "wasm32"))]
fn pick_open_module() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("NWN Module", &["mod", "MOD"])
        .pick_file()
        .map(|p| p.to_string_lossy().to_string())
}
#[cfg(not(target_arch = "wasm32"))]
fn pick_save_path(name: &str) -> Option<String> {
    rfd::FileDialog::new()
        .set_file_name(name)
        .add_filter("NWN Module", &["mod", "MOD"])
        .save_file()
        .map(|p| p.to_string_lossy().to_string())
}
#[cfg(not(target_arch = "wasm32"))]
fn pick_save_path_filtered(name: &str, filter_name: &str, exts: &[&str]) -> Option<String> {
    rfd::FileDialog::new()
        .set_file_name(name)
        .add_filter(filter_name, exts)
        .save_file()
        .map(|p| p.to_string_lossy().to_string())
}
#[cfg(target_arch = "wasm32")]
fn pick_open_module() -> Option<String> {
    None
}
#[cfg(target_arch = "wasm32")]
fn pick_save_path(_name: &str) -> Option<String> {
    None
}
#[cfg(target_arch = "wasm32")]
fn pick_save_path_filtered(_name: &str, _filter_name: &str, _exts: &[&str]) -> Option<String> {
    None
}

#[derive(Clone)]
#[allow(dead_code)]
struct ObjectClipboard {
    obj_type: String,
    resref: String,
    tag: String,
    bearing: f32,
    bag: nwn_gamemodel::PropertyBag,
}

#[allow(dead_code)]
struct CameraBookmark {
    name: String,
    target: [f32; 3],
    yaw: f32,
    pitch: f32,
    distance: f32,
}

enum PlacementState {
    Inactive,
    Pending(String, String),
}

#[allow(dead_code)]
enum ManipMode {
    None,
    Move,
    Rotate,
}

#[derive(Clone)]
#[allow(dead_code)]
struct DragState {
    obj_type: String,
    obj_idx: usize,
    orig_x: f32,
    orig_y: f32,
    orig_z: f32,
    orig_bearing: f32,
    start_render_x: f32,
    start_render_z: f32,
}

#[allow(dead_code)]
fn nwn_modules_dir() -> String {
    if let Ok(home) = std::env::var("HOME") {
        format!("{}/Documents/Neverwinter Nights/modules", home)
    } else {
        ".".to_string()
    }
}

struct AuroraBorealisApp {
    viewport: nwn_3d::WgpuViewport,
    current_area: Option<Arc<nwn_gamemodel::Area>>,
    area_version: u64,
    open_areas: Vec<String>,
    active_area_idx: usize,
    selected_object: Option<SelectedObject>,
    selected_obj_info: Option<(String, usize)>,
    selection_list: Vec<(String, usize)>,

    module: Option<nwn_gamemodel::Module>,
    module_name_edit: String,
    module_tag_edit: String,
    module_entry_edit: String,
    status: String,
    clear_override_enabled: bool,
    clear_override: Option<[u8; 3]>,
    show_resource_viewer: bool,
    texture_browser: TextureBrowserState,
    tileset_browser: TilesetBrowserState,
    geometry_editor: GeometryEditorState,
    show_blueprint_palette: bool,
    show_sky_override: bool,
    nwn_process_handle: Option<std::process::Child>,
    #[allow(dead_code)]
    module_save_handle: Option<std::thread::JoinHandle<()>>,
    clipboard: Option<ObjectClipboard>,
    camera_bookmarks: Vec<CameraBookmark>,
    #[allow(dead_code)]
    faction_list: FactionList,
    conversation_editors: Vec<(String, ConversationEditor)>,
    two_da_editors: Vec<(String, TwoDaEditorState)>,
    script_editor_scripts: Vec<(String, String)>,
    blueprint_editors: Vec<(String, u16, nwn_gamemodel::PropertyBag)>,
    item_browser_open: bool,
    undo_stack: Vec<Arc<nwn_gamemodel::Area>>,
    redo_stack: Vec<Arc<nwn_gamemodel::Area>>,
    #[allow(dead_code)]
    inspector_expanded: std::collections::HashSet<String>,
    #[allow(dead_code)]
    snap_to_grid_value: f32,
    snap_to_grid_enabled: bool,
    show_navigator: bool,
    recent_files: Vec<String>,
    #[allow(dead_code)]
    tlk_file: Option<std::sync::Arc<TlkFile>>,
    #[allow(dead_code)]
    area_list_cache: Vec<(String, String)>,
    show_minimap: bool,
    graph_view_open: bool,
    graph_view_state: GraphViewState,
    height_map_open: bool,
    height_map_state: HeightMapState,
    show_walkmesh: bool,
    show_walkmesh_edges: bool,
    show_paint: bool,
    show_creatures: bool,
    show_doors: bool,
    show_placeables: bool,
    show_triggers: bool,
    show_waypoints: bool,
    show_encounters: bool,
    show_sound_emitters: bool,
    water_level: i32,
    show_water: bool,
    module_props_popup: bool,
    module_description_edit: String,
    area_props_popup: bool,
    area_props_resref: String,
    area_props_name: String,
    area_props_tag: String,
    area_props_tileset: String,
    local_var_editor: Option<LocalVarEditorState>,
    appearance_picker: Option<AppearancePickerState>,
    object_comment_edit: Option<(String, usize, String)>,
    inventory_editor: Option<InventoryEditorState>,
    placement_state: PlacementState,
    manip_mode: ManipMode,
    drag_state: Option<DragState>,
    manip_nudge_speed: f32,
    ghost_screen_pos: Option<egui::Pos2>,
    faction_editor_open: bool,
    journal_editor_open: bool,
    encounter_editor_open: bool,
    env_settings_open: bool,
    transition_editor_open: bool,
    module_tree_open: bool,
    find_replace_open: bool,
    find_replace_state: FindReplaceState,
    search_term: String,
    search_results: Vec<String>,
    sound_browser_open: bool,
    store_editor_open: bool,
    id_whois_open: bool,
    env_sun_fog_amount: u8,
    env_sun_fog_color: [u8; 3],
    env_moon_fog_amount: u8,
    env_moon_fog_color: [u8; 3],
    env_fog_clip_dist: f32,
    env_sun_ambient_color: [u8; 3],
    walkmesh_wall_mode: bool,
    walkmesh_gen_threshold: i32,
    terrain_edit_mode: bool,
    terrain_edit_raise_amount: i32,
    terrain_smooth_mode: bool,
    terrain_smooth_radius: i32,
    show_contents_panel: bool,
    new_area_open: bool,
    new_area_resref: String,
    new_area_tileset: String,
    new_area_width: i32,
    new_area_height: i32,
    show_tiles: bool,
    show_grid: bool,
    lighting_enabled: bool,
    show_fog: bool,
    selected_tile: Option<(usize, usize)>,
    tile_palette_open: bool,
    tile_palette_filter: String,
    new_object_open: bool,
    new_object_type: String,
    new_object_resref: String,
    new_object_tag: String,
    new_object_x: f32,
    new_object_y: f32,
    new_object_z: f32,
    new_object_bearing: f32,
}

#[derive(Debug, Clone)]
struct InventoryEditorState {
    obj_type: String,
    obj_idx: usize,
    items: Vec<PropertyBag>,
    editing_item: Option<usize>,
    add_blueprint_filter: String,
    add_blueprint_results: Vec<(String, u16)>,
}

#[derive(Debug, Clone)]
struct LocalVarEditorState {
    obj_type: String,
    obj_idx: usize,
    vars: Vec<LocalVar>,
}

#[derive(Debug, Clone)]
struct AppearancePickerState {
    obj_type: String,
    obj_idx: usize,
    filter: String,
    appearances: Vec<AppearanceEntry>,
}

#[derive(Debug, Clone)]
struct LocalVar {
    name: String,
    var_type: String,
    value: String,
}

#[derive(Debug, Clone)]
struct AppearanceEntry {
    id: i32,
    label: String,
    model: String,
}

struct FindReplaceState {
    find_text: String,
    replace_text: String,
    results: Vec<FindResult>,
    show_results: bool,
}

struct FindResult {
    script_name: String,
    line_number: usize,
    line_text: String,
}

impl Default for FindReplaceState {
    fn default() -> Self { Self { find_text: String::new(), replace_text: String::new(), results: Vec::new(), show_results: false } }
}

struct GraphViewState {
    pan_x: f32,
    pan_y: f32,
    zoom: f32,
    filter_creatures: bool,
    filter_doors: bool,
    filter_placeables: bool,
    filter_triggers: bool,
    filter_waypoints: bool,
    filter_encounters: bool,
    filter_sound_emitters: bool,
}

impl Default for GraphViewState {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 8.0,
            filter_creatures: true,
            filter_doors: true,
            filter_placeables: true,
            filter_triggers: true,
            filter_waypoints: true,
            filter_encounters: true,
            filter_sound_emitters: true,
        }
    }
}

struct HeightMapState {
    pan_x: f32,
    pan_y: f32,
    zoom: f32,
    brush_size: i32,
    brush_amount: i32,
    show_heights: bool,
}

impl Default for HeightMapState {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 20.0,
            brush_size: 1,
            brush_amount: 1,
            show_heights: false,
        }
    }
}

trait GraphLabel {
    fn graph_label(&self) -> String;
}

impl GraphLabel for nwn_gamemodel::CreatureInstance {
    fn graph_label(&self) -> String { format!("{} ({})", self.resref, self.tag) }
}
impl GraphLabel for nwn_gamemodel::DoorInstance {
    fn graph_label(&self) -> String { format!("{} ({})", self.resref, self.tag) }
}
impl GraphLabel for nwn_gamemodel::PlaceableInstance {
    fn graph_label(&self) -> String { format!("{} ({})", self.resref, self.tag) }
}
impl GraphLabel for nwn_gamemodel::TriggerInstance {
    fn graph_label(&self) -> String { format!("Trigger ({})", self.tag) }
}
impl GraphLabel for nwn_gamemodel::WayPointInstance {
    fn graph_label(&self) -> String { format!("Waypoint ({})", self.tag) }
}
impl GraphLabel for nwn_gamemodel::EncounterInstance {
    fn graph_label(&self) -> String { format!("{} ({})", self.name, self.tag) }
}
impl GraphLabel for nwn_gamemodel::SoundEmitterInstance {
    fn graph_label(&self) -> String { format!("Sound: {}", self.tag) }
}

fn load_local_vars(module: &nwn_gamemodel::Module, area_resref: &str, obj_type: &str, obj_idx: usize) -> Vec<LocalVar> {
    let git_key = area_resref.trim().to_lowercase();
    let git_data = module.raw_git.get(&git_key);
    let git_gff = git_data.and_then(|d| nwn_gff::GffFile::from_bytes(d).ok());
    let fv_list_name = git_list_name(obj_type);
    git_gff.as_ref().and_then(|gff| {
        let list_fv = gff.root().find(fv_list_name)?;
        let list = match list_fv {
            nwn_gff::FieldValue::List(l) => l,
            _ => return None,
        };
        let obj_struct = list.get(obj_idx)?;
        let vt_fv = obj_struct.find("VarTable")?;
        let vt_list = match vt_fv {
            nwn_gff::FieldValue::List(l) => l,
            _ => return None,
        };
        let mut vars = Vec::new();
        for entry in vt_list.iter() {
            let name = entry.find("Name").and_then(|fv| match fv {
                nwn_gff::FieldValue::String(s) => Some(s.clone()),
                nwn_gff::FieldValue::ResRef(s) => Some(s.clone()),
                _ => None,
            }).unwrap_or_default();
            let vtype = entry.find("Type").and_then(|fv| match fv {
                nwn_gff::FieldValue::Char(c) => Some(*c),
                nwn_gff::FieldValue::Byte(b) => Some(*b as i8),
                _ => None,
            }).unwrap_or(0);
            let type_str = match vtype {
                1 => "int",
                2 => "float",
                3 => "string",
                4 => "object",
                _ => "int",
            };
            let val = match vtype {
                1 => entry.find("Value").and_then(|fv| match fv { nwn_gff::FieldValue::Int(i) => Some(i.to_string()), _ => None }).unwrap_or_else(|| "0".into()),
                2 => entry.find("Value").and_then(|fv| match fv { nwn_gff::FieldValue::Float(f) => Some(f.to_string()), _ => None }).unwrap_or_else(|| "0.0".into()),
                3 => entry.find("Value").and_then(|fv| match fv { nwn_gff::FieldValue::String(s) => Some(s.clone()), _ => None }).unwrap_or_default(),
                _ => entry.find("Value").and_then(|fv| match fv { nwn_gff::FieldValue::Int(i) => Some(i.to_string()), _ => None }).unwrap_or_else(|| "0".into()),
            };
            vars.push(LocalVar { name, var_type: type_str.to_string(), value: val });
        }
        Some(vars)
    }).unwrap_or_default()
}

fn save_local_vars_to_git(module: &mut nwn_gamemodel::Module, area_resref: &str, obj_type: &str, obj_idx: usize, vars: &[LocalVar]) {
    use nwn_gamemodel::{PropertyBag, PropertyValue, Property};
    let git_key = area_resref.trim().to_lowercase();
    let data = match module.raw_git.get(&git_key) {
        Some(d) => d.clone(),
        None => return,
    };
    let mut gff = match nwn_gff::GffFile::from_bytes(&data) {
        Ok(g) => g,
        Err(_) => return,
    };
    let fv_list_name = git_list_name(obj_type);
    let struct_id = git_struct_id(obj_type);
    let bag = {
        // Build a bag from the existing struct + our modifications
        // We need to serialize the target struct to bytes, re-parse, then build bag
        // But we can't clone Struct. Instead, serialize the whole GFF, re-parse,
        // extract the struct reference, build bag, then write back.
        let gff_bytes = gff.to_bytes();
        let gff_copy = match nwn_gff::GffFile::from_bytes(&gff_bytes) {
            Ok(g) => g,
            Err(_) => return,
        };
        let list_fv = match gff_copy.root().find(fv_list_name) {
            Some(l) => l,
            None => return,
        };
        let list = match list_fv {
            nwn_gff::FieldValue::List(l) => l,
            _ => return,
        };
        let obj_struct = match list.get(obj_idx) {
            Some(s) => s,
            None => return,
        };
        let mut bag = PropertyBag::from_gff_struct(obj_struct);
        bag.properties.retain(|p| p.label != "VarTable");

        let mut vt_items: Vec<Box<PropertyBag>> = Vec::new();
        for v in vars {
            let mut entry = PropertyBag::new();
            entry.properties.push(Property { label: "Name".into(), value: PropertyValue::CExoString(v.name.clone()), field_type: 10, category: String::new(), description: String::new() });
            let (vtype_int, val_pv) = match v.var_type.as_str() {
                "float" => (2i8, v.value.parse::<f32>().map(PropertyValue::Float).unwrap_or(PropertyValue::Float(0.0))),
                "string" => (3i8, PropertyValue::CExoString(v.value.clone())),
                "object" => (4i8, PropertyValue::CExoString(v.value.clone())),
                _ => (1i8, v.value.parse::<i32>().map(PropertyValue::Int).unwrap_or(PropertyValue::Int(0))),
            };
            let type_field_type = match vtype_int { 2 => 8, 3 => 10, 4 => 10, _ => 5 };
            entry.properties.push(Property { label: "Type".into(), value: PropertyValue::Char(vtype_int), field_type: 1, category: String::new(), description: String::new() });
            entry.properties.push(Property { label: "Value".into(), value: val_pv, field_type: type_field_type, category: String::new(), description: String::new() });
            vt_items.push(Box::new(entry));
        }
        bag.properties.push(Property { label: "VarTable".into(), value: PropertyValue::List(vt_items), field_type: 15, category: String::new(), description: String::new() });
        bag
    };
    let new_struct = bag.to_gff_struct(struct_id);
    match gff.root_mut().find_mut(fv_list_name) {
        Some(nwn_gff::FieldValue::List(list)) => {
            if obj_idx < list.len() {
                list[obj_idx] = new_struct;
            }
        }
        _ => return,
    }
    module.raw_git.insert(git_key, gff.to_bytes());
    module.modified = true;
}

fn load_object_comment(module: &nwn_gamemodel::Module, area_resref: &str, obj_type: &str, obj_idx: usize) -> String {
    let git_key = area_resref.trim().to_lowercase();
    let git_data = module.raw_git.get(&git_key);
    let git_gff = git_data.and_then(|d| nwn_gff::GffFile::from_bytes(d).ok());
    let fv_list_name = git_list_name(obj_type);
    git_gff.as_ref().and_then(|gff| {
        let list_fv = gff.root().find(fv_list_name)?;
        let list = match list_fv {
            nwn_gff::FieldValue::List(l) => l,
            _ => return None,
        };
        let obj_struct = list.get(obj_idx)?;
        obj_struct.find("Comment").and_then(|fv| match fv {
            nwn_gff::FieldValue::String(s) => Some(s.clone()),
            nwn_gff::FieldValue::ResRef(s) => Some(s.clone()),
            _ => None,
        })
    }).unwrap_or_default()
}

fn save_object_comment(module: &mut nwn_gamemodel::Module, area_resref: &str, obj_type: &str, obj_idx: usize, comment: &str) {
    use nwn_gamemodel::{PropertyBag, PropertyValue, Property};
    let git_key = area_resref.trim().to_lowercase();
    let data = match module.raw_git.get(&git_key) {
        Some(d) => d.clone(),
        None => return,
    };
    let mut gff = match nwn_gff::GffFile::from_bytes(&data) {
        Ok(g) => g,
        Err(_) => return,
    };
    let fv_list_name = git_list_name(obj_type);
    let struct_id = git_struct_id(obj_type);
    let bag = {
        let gff_bytes = gff.to_bytes();
        let gff_copy = match nwn_gff::GffFile::from_bytes(&gff_bytes) {
            Ok(g) => g,
            Err(_) => return,
        };
        let list_fv = match gff_copy.root().find(fv_list_name) {
            Some(l) => l,
            None => return,
        };
        let list = match list_fv {
            nwn_gff::FieldValue::List(l) => l,
            _ => return,
        };
        let obj_struct = match list.get(obj_idx) {
            Some(s) => s,
            None => return,
        };
        let mut bag = PropertyBag::from_gff_struct(obj_struct);
        let mut found = false;
        for p in bag.properties.iter_mut() {
            if p.label == "Comment" {
                p.value = PropertyValue::CExoString(comment.to_string());
                found = true;
                break;
            }
        }
        if !found {
            bag.properties.push(Property { label: "Comment".into(), value: PropertyValue::CExoString(comment.to_string()), field_type: 10, category: String::new(), description: String::new() });
        }
        bag
    };
    let new_struct = bag.to_gff_struct(struct_id);
    match gff.root_mut().find_mut(fv_list_name) {
        Some(nwn_gff::FieldValue::List(list)) => {
            if obj_idx < list.len() {
                list[obj_idx] = new_struct;
            }
        }
        _ => return,
    }
    module.raw_git.insert(git_key, gff.to_bytes());
    module.modified = true;
}

fn set_object_appearance(module: &mut nwn_gamemodel::Module, area_resref: &str, obj_type: &str, obj_idx: usize, appearance_id: i32) {
    use nwn_gamemodel::{PropertyBag, PropertyValue, Property};
    let git_key = area_resref.trim().to_lowercase();
    let data = match module.raw_git.get(&git_key) {
        Some(d) => d.clone(),
        None => return,
    };
    let mut gff = match nwn_gff::GffFile::from_bytes(&data) {
        Ok(g) => g,
        Err(_) => return,
    };
    let fv_list_name = git_list_name(obj_type);
    let struct_id = git_struct_id(obj_type);
    let bag = {
        let gff_bytes = gff.to_bytes();
        let gff_copy = match nwn_gff::GffFile::from_bytes(&gff_bytes) {
            Ok(g) => g,
            Err(_) => return,
        };
        let list_fv = match gff_copy.root().find(fv_list_name) {
            Some(l) => l,
            None => return,
        };
        let list = match list_fv {
            nwn_gff::FieldValue::List(l) => l,
            _ => return,
        };
        let obj_struct = match list.get(obj_idx) {
            Some(s) => s,
            None => return,
        };
        let mut bag = PropertyBag::from_gff_struct(obj_struct);
        let mut found = false;
        for p in bag.properties.iter_mut() {
            if p.label == "Appearance" {
                p.value = PropertyValue::Int(appearance_id);
                found = true;
                break;
            }
        }
        if !found {
            bag.properties.push(Property { label: "Appearance".into(), value: PropertyValue::Int(appearance_id), field_type: 5, category: String::new(), description: String::new() });
        }
        bag
    };
    let new_struct = bag.to_gff_struct(struct_id);
    match gff.root_mut().find_mut(fv_list_name) {
        Some(nwn_gff::FieldValue::List(list)) => {
            if obj_idx < list.len() {
                list[obj_idx] = new_struct;
            }
        }
        _ => return,
    }
    module.raw_git.insert(git_key, gff.to_bytes());
    module.modified = true;
}

fn load_item_list(module: Option<&nwn_gamemodel::Module>, area_resref: &str, obj_type: &str, obj_idx: usize) -> Vec<PropertyBag> {
    let module = match module { Some(m) => m, None => return vec![] };
    let git_key = area_resref.trim().to_lowercase();
    let git_data = module.raw_git.get(&git_key);
    let git_gff = git_data.and_then(|d| nwn_gff::GffFile::from_bytes(d).ok());
    let fv_list_name = git_list_name(obj_type);
    git_gff.as_ref().and_then(|gff| {
        let list_fv = gff.root().find(fv_list_name)?;
        let list = match list_fv {
            nwn_gff::FieldValue::List(l) => l,
            _ => return None,
        };
        let obj_struct = list.get(obj_idx)?;
        let item_list_fv = obj_struct.find("ItemList")?;
        let item_list = match item_list_fv {
            nwn_gff::FieldValue::List(l) => l,
            _ => return None,
        };
        let items: Vec<PropertyBag> = item_list.iter().map(|s| PropertyBag::from_gff_struct(s)).collect();
        Some(items)
    }).unwrap_or_default()
}

fn save_item_list(module: Option<&mut nwn_gamemodel::Module>, area_resref: &str, obj_type: &str, obj_idx: usize, items: &[PropertyBag]) {
    let module = match module { Some(m) => m, None => return };
    let git_key = area_resref.trim().to_lowercase();
    let data = match module.raw_git.get(&git_key) {
        Some(d) => d.clone(),
        None => return,
    };
    let mut gff = match nwn_gff::GffFile::from_bytes(&data) {
        Ok(g) => g,
        Err(_) => return,
    };
    let fv_list_name = git_list_name(obj_type);
    let struct_id = git_struct_id(obj_type);
    let bag = {
        let gff_bytes = gff.to_bytes();
        let gff_copy = match nwn_gff::GffFile::from_bytes(&gff_bytes) {
            Ok(g) => g,
            Err(_) => return,
        };
        let list_fv = match gff_copy.root().find(fv_list_name) {
            Some(l) => l,
            None => return,
        };
        let list = match list_fv {
            nwn_gff::FieldValue::List(l) => l,
            _ => return,
        };
        let obj_struct = match list.get(obj_idx) {
            Some(s) => s,
            None => return,
        };
        let mut bag = PropertyBag::from_gff_struct(obj_struct);
        bag.properties.retain(|p| p.label != "ItemList");
        let item_bags: Vec<Box<PropertyBag>> = items.iter().map(|i| Box::new(i.clone())).collect();
        bag.properties.push(nwn_gamemodel::Property {
            label: "ItemList".into(),
            value: nwn_gamemodel::PropertyValue::List(item_bags),
            field_type: 15,
            category: String::new(),
            description: String::new(),
        });
        bag
    };
    let new_struct = bag.to_gff_struct(struct_id);
    match gff.root_mut().find_mut(fv_list_name) {
        Some(nwn_gff::FieldValue::List(list)) => {
            if obj_idx < list.len() {
                list[obj_idx] = new_struct;
            }
        }
        _ => return,
    }
    module.raw_git.insert(git_key, gff.to_bytes());
    module.modified = true;
}

impl AuroraBorealisApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let _ = cc; // adapter/surface created by wgpu render state
        let viewport = nwn_3d::WgpuViewport::new();
        let mut recent_files: Vec<String> = load_recent_files();
        recent_files.retain(|f| std::path::Path::new(f).exists());
        Self {
            viewport,
            current_area: None,
            area_version: 0,
            open_areas: Vec::new(),
            active_area_idx: 0,
            selected_object: None,
            selected_obj_info: None,
            selection_list: Vec::new(),

            module: None,
            module_name_edit: String::new(),
            module_tag_edit: String::new(),
            module_entry_edit: String::new(),
            status: "Ready.".to_string(),
            clear_override_enabled: false,
            clear_override: None,
            show_resource_viewer: false,
            texture_browser: TextureBrowserState::default(),
            tileset_browser: TilesetBrowserState::default(),
            geometry_editor: GeometryEditorState::default(),
            show_blueprint_palette: false,
            show_sky_override: false,
            nwn_process_handle: None,
            module_save_handle: None,
            clipboard: None,
            camera_bookmarks: Vec::new(),
            faction_list: FactionList { factions: Vec::new() },
            conversation_editors: Vec::new(),
            two_da_editors: Vec::new(),
            script_editor_scripts: Vec::new(),
            blueprint_editors: Vec::new(),
            item_browser_open: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            inspector_expanded: std::collections::HashSet::new(),
            snap_to_grid_value: 10.0,
            snap_to_grid_enabled: true,
            show_navigator: true,
            recent_files,
            tlk_file: None,
            area_list_cache: Vec::new(),
            show_minimap: false,
            graph_view_open: false,
            graph_view_state: GraphViewState::default(),
            height_map_open: false,
            height_map_state: HeightMapState::default(),
            show_walkmesh: false,
            show_walkmesh_edges: false,
            show_fog: false,
            show_paint: false,
            show_creatures: true,
            show_doors: true,
            show_placeables: true,
            show_triggers: true,
            show_waypoints: true,
            show_encounters: true,
            show_sound_emitters: true,
            water_level: 0,
            show_water: false,
            module_props_popup: false,
            module_description_edit: String::new(),
            area_props_popup: false,
            area_props_resref: String::new(),
            area_props_name: String::new(),
            area_props_tag: String::new(),
            area_props_tileset: String::new(),
            local_var_editor: None,
            appearance_picker: None,
            object_comment_edit: None,
            inventory_editor: None,
            placement_state: PlacementState::Inactive,
            manip_mode: ManipMode::None,
            drag_state: None,
            manip_nudge_speed: 0.5,
            ghost_screen_pos: None,
            faction_editor_open: false,
            journal_editor_open: false,
            encounter_editor_open: false,
            env_settings_open: false,
            transition_editor_open: false,
            module_tree_open: false,
            find_replace_open: false,
            find_replace_state: FindReplaceState::default(),
            search_term: String::new(),
            search_results: Vec::new(),
            sound_browser_open: false,
            store_editor_open: false,
            id_whois_open: false,
            env_sun_fog_amount: 0,
            env_sun_fog_color: [0, 0, 0],
            env_moon_fog_amount: 0,
            env_moon_fog_color: [0, 0, 0],
            env_fog_clip_dist: 45.0,
            env_sun_ambient_color: [0, 0, 0],
            walkmesh_wall_mode: false,
            walkmesh_gen_threshold: 3,
            terrain_edit_mode: false,
            terrain_edit_raise_amount: 1,
            terrain_smooth_mode: false,
            terrain_smooth_radius: 2,
            show_contents_panel: false,
            new_area_open: false,
            new_area_resref: String::new(),
            new_area_tileset: String::new(),
            new_area_width: 16,
            new_area_height: 16,
            show_tiles: true,
            selected_tile: None,
            tile_palette_open: false,
            show_grid: true,
            lighting_enabled: true,
            tile_palette_filter: String::new(),
            new_object_open: false,
            new_object_type: "creature".to_string(),
            new_object_resref: String::new(),
            new_object_tag: String::new(),
            new_object_x: 0.0,
            new_object_y: 0.0,
            new_object_z: 0.0,
            new_object_bearing: 0.0,
        }
    }
}

impl eframe::App for AuroraBorealisApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Callbacks from editors
        let mut save_2da: Option<(String, nwn_2da::TwoDa)> = None;
        let mut open_script: Option<String> = None;
        let mut open_conversation: Option<String> = None;
        let mut edit_blueprint: Option<(String, u16, Vec<u8>)> = None;
        let mut edit_tlk: Option<Vec<u8>> = None;
        let mut open_faction: Option<String> = None;
        let mut open_journal: Option<String> = None;
        let mut module_props_changed = false;

        // Compute render target format once for reuse
        let target_format = _frame.wgpu_render_state().map(|rs| rs.target_format).unwrap_or(wgpu::TextureFormat::Bgra8Unorm);

        // ===================== TOP MENU BAR =====================
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                // ---- File menu ----
                ui.menu_button("File", |ui| {
                    if ui.button("Open Module...").clicked() {
                        if let Some(path) = pick_open_module() {
                            self.load_module_at_path(&path);
                            add_recent_file(&mut self.recent_files, &path);
                        }
                        ui.close_menu();
                    }
                    ui.menu_button("Open Recent", |ui| {
                        for f in self.recent_files.clone() {
                            if ui.button(&f).clicked() {
                                self.load_module_at_path(&f);
                                add_recent_file(&mut self.recent_files, &f);
                                ui.close_menu();
                            }
                        }
                        if self.recent_files.is_empty() {
                            ui.label("No recent files");
                        }
                    });
                    ui.separator();
                    if ui.button("Module Properties").clicked() {
                        if let Some(ref module) = self.module {
                            self.module_name_edit = module.name.clone();
                            self.module_tag_edit = module.tag.clone();
                            self.module_entry_edit = module.entry_area.clone();
                            self.module_description_edit = module.description.clone();
                            self.module_props_popup = true;
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Save Module").clicked() {
                        self.save_module();
                        ui.close_menu();
                    }
                    if let Some(ref module) = self.module {
                        if ui.button("Save As...").clicked() {
                            if let Some(path) = pick_save_path(&module.name) {
                                self.save_module_as(&path);
                                self.status = format!("Module saved to {}", path);
                            }
                            ui.close_menu();
                        }
                    }
                    if self.module.is_some() {
                        if ui.button("Launch in NWN:EE").clicked() {
                            self.launch_nwn();
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui.button("Export Minimap PNG...").clicked() {
                        if let Some(ref area) = self.current_area {
                            if let Some(path) = pick_save_path_filtered(
                                &format!("{}_minimap.png", area.resref), "PNG", &["png"]
                            ) {
                                match export_minimap_png(area, &path) {
                                    Ok(()) => self.status = format!("Minimap saved to {}", path),
                                    Err(e) => self.status = format!("Minimap export error: {}", e),
                                }
                            }
                        }
                        ui.close_menu();
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        ui.separator();
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                });
                // ---- View menu ----
                ui.menu_button("View", |ui| {
                    let mut rv = self.show_resource_viewer;
                    ui.checkbox(&mut rv, "Resource Viewer");
                    if rv != self.show_resource_viewer {
                        self.show_resource_viewer = rv;
                        ui.close_menu();
                    }
                    let mut bp = self.show_blueprint_palette;
                    ui.checkbox(&mut bp, "Blueprint Palette");
                    if bp != self.show_blueprint_palette {
                        self.show_blueprint_palette = bp;
                        ui.close_menu();
                    }
                    let mut ib = self.item_browser_open;
                    ui.checkbox(&mut ib, "Item Browser");
                    if ib != self.item_browser_open {
                        self.item_browser_open = ib;
                        ui.close_menu();
                    }
                    ui.checkbox(&mut self.show_sky_override, "Sky Color Override");
                    ui.checkbox(&mut self.show_navigator, "Navigator Panel");
                    ui.checkbox(&mut self.show_minimap, "Minimap Overlay");
                    ui.checkbox(&mut self.graph_view_open, "Graph Area View");
                    ui.checkbox(&mut self.height_map_open, "Height Map View");
                    ui.checkbox(&mut self.module_tree_open, "Module Tree View");
                    ui.checkbox(&mut self.show_contents_panel, "Contents Panel");
                    ui.separator();
                    let mut ww = self.walkmesh_wall_mode;
                    ui.checkbox(&mut ww, "Walkmesh Wall Edit Mode");
                    if ww != self.walkmesh_wall_mode { self.walkmesh_wall_mode = ww; self.status = if ww { "Walkmesh wall edit mode ON. Click walkmesh edges to toggle walls.".to_string() } else { "Walkmesh wall edit mode OFF.".to_string() }; }
                    let mut te = self.terrain_edit_mode;
                    ui.checkbox(&mut te, "Terrain Edit Mode");
                    if te != self.terrain_edit_mode { self.terrain_edit_mode = te; if te { self.terrain_smooth_mode = false; } self.status = if te { "Terrain edit mode ON. Click to raise, Shift+click to lower.".to_string() } else { "Terrain edit mode OFF.".to_string() }; }
                    let mut ts = self.terrain_smooth_mode;
                    ui.checkbox(&mut ts, "Terrain Smooth Mode");
                    if ts != self.terrain_smooth_mode { self.terrain_smooth_mode = ts; if ts { self.terrain_edit_mode = false; } self.status = if ts { "Terrain smooth mode ON. Click to smooth heights.".to_string() } else { "Terrain smooth mode OFF.".to_string() }; }
                    if self.terrain_smooth_mode {
                        ui.add(egui::Slider::new(&mut self.terrain_smooth_radius, 1..=5).text("Smooth Radius"));
                    }
                    let mut st = self.show_tiles;
                    ui.checkbox(&mut st, "Show Tiles");
                    if st != self.show_tiles { self.show_tiles = st; self.viewport.set_show_tiles(st); }
                    let mut sg = self.show_grid;
                    ui.checkbox(&mut sg, "Show Grid");
                    if sg != self.show_grid { self.show_grid = sg; self.viewport.set_show_grid(sg); }
                    let mut le = self.lighting_enabled;
                    ui.checkbox(&mut le, "Full Bright");
                    if le != self.lighting_enabled { self.lighting_enabled = le; self.viewport.set_lighting_enabled(le); }
                    let mut ff = self.show_fog;
                    ui.checkbox(&mut ff, "Show Fog");
                    if ff != self.show_fog { self.show_fog = ff; self.viewport.set_show_fog(ff); }
                    ui.separator();
                    let mut sg = self.snap_to_grid_enabled;
                    ui.checkbox(&mut sg, "Snap to Grid");
                    if sg != self.snap_to_grid_enabled { self.snap_to_grid_enabled = sg; }
                    if self.snap_to_grid_enabled {
                        let mut sv = self.snap_to_grid_value;
                        ui.add(egui::Slider::new(&mut sv, 0.5..=50.0).text("Grid Size"));
                        if sv != self.snap_to_grid_value { self.snap_to_grid_value = sv; }
                    }
                    ui.separator();
                    let mut wm = self.show_walkmesh;
                    ui.checkbox(&mut wm, "Show Walkmesh");
                    if wm != self.show_walkmesh { self.show_walkmesh = wm; self.viewport.set_show_walkmesh(wm); }
                    let mut we = self.show_walkmesh_edges;
                    ui.checkbox(&mut we, "Walkmesh Edges");
                    if we != self.show_walkmesh_edges { self.show_walkmesh_edges = we; self.viewport.set_show_walkmesh_edges(we); }
                    let gen_threshold = self.walkmesh_gen_threshold;
                    let has_area = self.current_area.is_some();
                    if ui.button("Generate Walls").clicked() && has_area {
                        self.push_undo();
                        if let Some(ref mut area) = self.current_area {
                            let area_mut = Arc::make_mut(area);
                            let resref = area_mut.resref.clone();
                            let count = generate_walkmesh_walls(area_mut, gen_threshold);
                            if let Some(ref mut module) = self.module {
                                module.update_area_are(&resref, area_mut);
                            }
                            self.area_version = self.area_version.wrapping_add(1);
                            self.viewport.set_area(area.clone(), self.area_version, target_format);
                            self.status = format!("Generated {} walkmesh walls (threshold={})", count, gen_threshold);
                        }
                    }
                    ui.add(egui::Slider::new(&mut self.walkmesh_gen_threshold, 1..=10).text("Wall Threshold"));
                    let mut pt = self.show_paint;
                    ui.checkbox(&mut pt, "Show Paint Overlay");
                    if pt != self.show_paint { self.show_paint = pt; self.viewport.set_show_paint(pt); }
                    let mut filter_changed = false;
                    ui.menu_button("Show Object Markers", |ui| {
                        let mut sc = self.show_creatures;
                        ui.checkbox(&mut sc, "Creatures");
                        if sc != self.show_creatures { self.show_creatures = sc; filter_changed = true; }
                        let mut sd = self.show_doors;
                        ui.checkbox(&mut sd, "Doors");
                        if sd != self.show_doors { self.show_doors = sd; filter_changed = true; }
                        let mut sp = self.show_placeables;
                        ui.checkbox(&mut sp, "Placeables");
                        if sp != self.show_placeables { self.show_placeables = sp; filter_changed = true; }
                        let mut st = self.show_triggers;
                        ui.checkbox(&mut st, "Triggers");
                        if st != self.show_triggers { self.show_triggers = st; filter_changed = true; }
                        let mut swp = self.show_waypoints;
                        ui.checkbox(&mut swp, "Waypoints");
                        if swp != self.show_waypoints { self.show_waypoints = swp; filter_changed = true; }
                        let mut se = self.show_encounters;
                        ui.checkbox(&mut se, "Encounters");
                        if se != self.show_encounters { self.show_encounters = se; filter_changed = true; }
                        let mut sse = self.show_sound_emitters;
                        ui.checkbox(&mut sse, "Sound Emitters");
                        if sse != self.show_sound_emitters { self.show_sound_emitters = sse; filter_changed = true; }
                    });
                    if filter_changed {
                        self.update_visibility_filter(target_format);
                        ui.close_menu();
                    }
                    let mut sw = self.show_water;
                    ui.checkbox(&mut sw, "Show Water");
                    if sw != self.show_water { self.show_water = sw; if sw { self.viewport.set_water(self.water_level, true, [40, 80, 200, 160]); } else { self.viewport.set_water(0, false, [0;4]); } }
                    if self.show_water {
                        let mut wl = self.water_level;
                        ui.add(egui::Slider::new(&mut wl, -10..=50).text("Water Level"));
                        if wl != self.water_level { self.water_level = wl; self.viewport.set_water(wl, true, [40, 80, 200, 160]); }
                    }
                });
                // ---- Edit menu ----
                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo\tCtrl+Z").clicked() {
                        self.undo(target_format);
                        ui.close_menu();
                    }
                    if ui.button("Redo\tCtrl+Shift+Z").clicked() {
                        self.redo(target_format);
                        ui.close_menu();
                    }
                });
                // ---- Object menu ----
                if self.current_area.is_some() {
                    ui.menu_button("Object", |ui| {
                        if ui.button("New...\tCtrl+N").clicked() {
                            self.new_object_open = true;
                            self.new_object_type = "creature".to_string();
                            self.new_object_resref.clear();
                            self.new_object_tag.clear();
                            self.new_object_x = 0.0;
                            self.new_object_y = 0.0;
                            self.new_object_z = 0.0;
                            self.new_object_bearing = 0.0;
                            ui.close_menu();
                        }
                        if ui.button("Copy\tCtrl+C").clicked() {
                            self.copy_selected_object();
                            ui.close_menu();
                        }
                        if ui.button("Paste\tCtrl+V").clicked() {
                            self.paste_object();
                            ui.close_menu();
                        }
                        if ui.button("Delete\tDel").clicked() {
                            self.delete_selected_object();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Select All\tCtrl+A").clicked() {
                            // Select all not implemented yet
                            self.status = "Select All not implemented.".to_string();
                            ui.close_menu();
                        }
                    });
                }
                // ---- Tools menu ----
                ui.menu_button("Tools", |ui| {
                    if self.current_area.is_some() {
                        if ui.button("Environment Settings...").clicked() {
                            if let Some(ref area) = self.current_area {
                                self.env_sun_fog_amount = area.sun_fog_amount;
                                self.env_sun_fog_color = area.sun_fog_color;
                                self.env_moon_fog_amount = area.moon_fog_amount;
                                self.env_moon_fog_color = area.moon_fog_color;
                                self.env_fog_clip_dist = area.fog_clip_dist;
                                self.env_sun_ambient_color = area.sun_ambient_color;
                            }
                            self.env_settings_open = true;
                            ui.close_menu();
                        }
                    }
                    if ui.button("Faction Editor").clicked() {
                        self.faction_editor_open = true;
                        ui.close_menu();
                    }
                    if ui.button("Journal Editor").clicked() {
                        self.journal_editor_open = true;
                        ui.close_menu();
                    }
                    if ui.button("Encounter Editor").clicked() {
                        self.encounter_editor_open = true;
                        ui.close_menu();
                    }
                    if ui.button("Texture Browser").clicked() {
                        self.texture_browser.open = true;
                        ui.close_menu();
                    }
                    if ui.button("Visual Tileset Browser").clicked() {
                        self.tileset_browser.open = true;
                        ui.close_menu();
                    }
                    if self.module.is_some() {
                        if ui.button("Sound Browser").clicked() {
                            self.sound_browser_open = true;
                            ui.close_menu();
                        }
                    }
                    if self.module.is_some() {
                        if ui.button("Store Editor").clicked() {
                            self.store_editor_open = true;
                            ui.close_menu();
                        }
                    }
                    if self.module.is_some() {
                        if ui.button("ID Whois Utility").clicked() {
                            self.id_whois_open = true;
                            ui.close_menu();
                        }
                    }
                    if self.current_area.is_some() {
                        if ui.button("Area Geometry Editor").clicked() {
                            self.geometry_editor.open = true;
                            ui.close_menu();
                        }
                    }
                    if ui.button("Find/Replace in Scripts...").clicked() {
                        self.find_replace_open = true;
                        ui.close_menu();
                    }
                    if self.module.is_some() {
                        if ui.button("Area Transition Editor...").clicked() {
                            self.transition_editor_open = true;
                            self.status = "Scanned transitions. Open the Transition Editor window.".to_string();
                            ui.close_menu();
                        }
                    }
                });
                // ---- Camera bookmarks ----
                if self.current_area.is_some() {
                    ui.separator();
                    if ui.button("+ Bookmark").clicked() {
                        let idx = self.camera_bookmarks.len() + 1;
                        if let Some(cam) = self.viewport.camera_state() {
                            self.camera_bookmarks.push(CameraBookmark {
                                name: format!("Bookmark {}", idx),
                                target: cam.target,
                                yaw: cam.theta,
                                pitch: cam.phi,
                                distance: cam.radius,
                            });
                            self.status = format!("Camera bookmark '{}' added.", idx);
                        } else {
                            self.status = "Could not capture camera state.".to_string();
                        }
                    }
                    if !self.camera_bookmarks.is_empty() {
                        ui.menu_button("Bookmarks", |ui| {
                            let mut to_remove = None;
                            for (i, bm) in self.camera_bookmarks.iter().enumerate() {
                                if ui.button(&bm.name).clicked() {
                                    let cam = nwn_3d::OrbitCamera {
                                        target: bm.target,
                                        theta: bm.yaw,
                                        phi: bm.pitch,
                                        radius: bm.distance,
                                        aspect: 1.0,
                                    };
                                    self.viewport.set_camera(&cam);
                                    self.status = format!("Jumped to '{}'.", bm.name);
                                    ui.close_menu();
                                }
                                if ui.button(format!("Del {}", bm.name)).clicked() {
                                    to_remove = Some(i);
                                }
                            }
                            if let Some(i) = to_remove {
                                self.camera_bookmarks.remove(i);
                                self.status = "Bookmark removed.".to_string();
                            }
                        });
                    }
                }
                // ---- Right side: Undo/Redo ----
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Redo").clicked() {
                        if let Some(snapshot) = self.redo_stack.pop() {
                            if let Some(ref current) = self.current_area {
                                self.undo_stack.push(current.clone());
                            }
                            self.current_area = Some(snapshot.clone());
                            self.area_version = self.area_version.wrapping_add(1);
                            self.viewport.set_area(snapshot, self.area_version, target_format);
                            self.selected_object = None;
                            self.selected_obj_info = None;
                            self.selection_list.clear();
                            self.status = "Redo".to_string();
                        }
                    }
                    if ui.button("Undo").clicked() {
                        if let Some(snapshot) = self.undo_stack.pop() {
                            if let Some(ref current) = self.current_area {
                                self.redo_stack.push(current.clone());
                            }
                            self.current_area = Some(snapshot.clone());
                            self.area_version = self.area_version.wrapping_add(1);
                            self.viewport.set_area(snapshot, self.area_version, target_format);
                            self.selected_object = None;
                            self.selected_obj_info = None;
                            self.selection_list.clear();
                            self.status = "Undo".to_string();
                        }
                    }
                    if let Some(ref module) = self.module {
                        if module.modified {
                            ui.colored_label(egui::Color32::YELLOW, "● Modified");
                        }
                    }
                });
            });
        });

        // ===================== FLOATING WINDOWS =====================
        // Module properties popup
        if self.module_props_popup {
            egui::Window::new("Module Properties")
                .default_size([400.0, 300.0])
                .show(ctx, |ui| {
                    egui::Grid::new("module_props_grid").num_columns(2).show(ui, |ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.module_name_edit);
                        ui.end_row();

                        ui.label("Tag:");
                        ui.text_edit_singleline(&mut self.module_tag_edit);
                        ui.end_row();

                        ui.label("Entry Area:");
                        ui.text_edit_singleline(&mut self.module_entry_edit);
                        ui.end_row();

                        ui.label("Description:");
                        ui.text_edit_multiline(&mut self.module_description_edit);
                        ui.end_row();
                    });
                    if ui.button("Save").clicked() {
                        module_props_changed = true;
                        self.module_props_popup = false;
                    }
                });
        }

        // Area properties popup
        if self.area_props_popup {
            let mut changed = false;
            egui::Window::new("Area Properties")
                .default_size([400.0, 350.0])
                .show(ctx, |ui| {
                    egui::Grid::new("area_props_grid").num_columns(2).show(ui, |ui| {
                        ui.label("ResRef (read-only):");
                        ui.label(&self.area_props_resref);
                        ui.end_row();

                        ui.label("Tileset (read-only):");
                        ui.label(&self.area_props_tileset);
                        ui.end_row();

                        ui.label("Name:");
                        changed |= ui.text_edit_singleline(&mut self.area_props_name).changed();
                        ui.end_row();

                        ui.label("Tag:");
                        changed |= ui.text_edit_singleline(&mut self.area_props_tag).changed();
                        ui.end_row();
                    });
                    ui.separator();
                    ui.heading("Lighting");
                    if let Some(ref mut area) = self.current_area {
                        let area_mut = Arc::make_mut(area);
                        egui::Grid::new("area_lighting_grid").num_columns(2).show(ui, |ui| {
                            ui.label("Sun Ambient:");
                            changed |= ui.color_edit_button_srgb(&mut area_mut.sun_ambient_color).changed();
                            ui.end_row();
                            ui.label("Sun Diffuse:");
                            changed |= ui.color_edit_button_srgb(&mut area_mut.sun_diffuse_color).changed();
                            ui.end_row();
                            ui.label("Sun Fog Amount:");
                            changed |= ui.add(egui::Slider::new(&mut area_mut.sun_fog_amount, 0..=255)).changed();
                            ui.end_row();
                            ui.label("Sun Fog Color:");
                            changed |= ui.color_edit_button_srgb(&mut area_mut.sun_fog_color).changed();
                            ui.end_row();
                            ui.label("Moon Ambient:");
                            changed |= ui.color_edit_button_srgb(&mut area_mut.moon_ambient_color).changed();
                            ui.end_row();
                            ui.label("Moon Diffuse:");
                            changed |= ui.color_edit_button_srgb(&mut area_mut.moon_diffuse_color).changed();
                            ui.end_row();
                            ui.label("Moon Fog Amount:");
                            changed |= ui.add(egui::Slider::new(&mut area_mut.moon_fog_amount, 0..=255)).changed();
                            ui.end_row();
                            ui.label("Moon Fog Color:");
                            changed |= ui.color_edit_button_srgb(&mut area_mut.moon_fog_color).changed();
                            ui.end_row();
                            ui.label("Fog Clip Distance:");
                            changed |= ui.add(egui::Slider::new(&mut area_mut.fog_clip_dist, 0.0..=500.0)).changed();
                            ui.end_row();
                        });
                    }
                    if changed {
                        if let Some(ref mut area) = self.current_area {
                            if area.name != self.area_props_name || area.tag != self.area_props_tag {
                                let area_mut = Arc::make_mut(area);
                                area_mut.name = self.area_props_name.clone();
                                area_mut.tag = self.area_props_tag.clone();
                            }
                            let resref = area.resref.clone();
                            if let Some(ref mut module) = self.module {
                                module.update_area_are(&resref, area);
                            }
                            self.area_version = self.area_version.wrapping_add(1);
                            self.viewport.set_area(area.clone(), self.area_version, target_format);
                        }
                    }
                    if ui.button("Close").clicked() {
                        self.area_props_popup = false;
                    }
                });
        }

        // Local Variables editor
        if let Some(ref mut lve) = self.local_var_editor {
            let mut close = false;
            egui::Window::new("Local Variables")
                .default_size([500.0, 300.0])
                .show(ctx, |ui| {
                    egui::ScrollArea::both().show(ui, |ui| {
                        let mut ops: Vec<(usize, &str)> = Vec::new();
                        egui::Grid::new("local_vars_grid").num_columns(5).striped(true).show(ui, |ui| {
                            ui.label("Name"); ui.label("Type"); ui.label("Value"); ui.label(""); ui.label("");
                            ui.end_row();
                            let len = lve.vars.len();
                            for i in 0..len {
                                {
                                    let v = &mut lve.vars[i];
                                    ui.text_edit_singleline(&mut v.name);
                                    egui::ComboBox::from_id_salt(("var_type", i))
                                        .selected_text(&v.var_type.clone())
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut v.var_type, "int".to_string(), "int");
                                            ui.selectable_value(&mut v.var_type, "float".to_string(), "float");
                                            ui.selectable_value(&mut v.var_type, "string".to_string(), "string");
                                            ui.selectable_value(&mut v.var_type, "object".to_string(), "object");
                                        });
                                    ui.text_edit_singleline(&mut v.value);
                                }
                                if ui.button("+").clicked() {
                                    ops.push((i, "insert"));
                                }
                                if ui.button("−").clicked() {
                                    ops.push((i, "remove"));
                                }
                                ui.end_row();
                            }
                        });
                        for op in ops.iter().rev() {
                            match op.1 {
                                "insert" => lve.vars.insert(op.0 + 1, LocalVar { name: String::new(), var_type: "int".to_string(), value: "0".to_string() }),
                                "remove" => { lve.vars.remove(op.0); }
                                _ => {}
                            }
                        }
                        if ui.button("+ Add Variable").clicked() {
                            lve.vars.push(LocalVar { name: String::new(), var_type: "int".to_string(), value: "0".to_string() });
                        }
                    });
                    ui.separator();
                    if ui.button("Save").clicked() {
                        if let (Some(area), Some(ref mut module)) = (self.current_area.as_ref(), self.module.as_mut()) {
                            save_local_vars_to_git(module, &area.resref, &lve.obj_type, lve.obj_idx, &lve.vars);
                        }
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            if close {
                self.local_var_editor = None;
            }
        }

        // Appearance picker
        if let Some(ref mut ap) = self.appearance_picker {
            let mut close = false;
            egui::Window::new("Appearance Picker")
                .default_size([500.0, 400.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Filter:");
                        ui.text_edit_singleline(&mut ap.filter);
                    });
                    ui.separator();
                    egui::ScrollArea::both().show(ui, |ui| {
                        egui::Grid::new("appearance_grid").num_columns(2).striped(true).show(ui, |ui| {
                            for entry in &ap.appearances {
                                if !ap.filter.is_empty() && !entry.label.to_lowercase().contains(&ap.filter.to_lowercase()) && !entry.model.to_lowercase().contains(&ap.filter.to_lowercase()) {
                                    continue;
                                }
                                if ui.selectable_label(false, &entry.label).clicked() {
                                    let area_resref = self.current_area.as_ref().map(|a| a.resref.clone()).unwrap_or_default();
                                    if let Some(ref mut module) = self.module {
                                        set_object_appearance(module, &area_resref, &ap.obj_type, ap.obj_idx, entry.id);
                                    }
                                    self.status = format!("Appearance set to {} ({})", entry.label, entry.id);
                                    close = true;
                                }
                                ui.label(&entry.model);
                                ui.end_row();
                            }
                        });
                    });
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            if close {
                self.appearance_picker = None;
            }
        }

        // Object comment editor
        if let Some(ref mut comment_state) = self.object_comment_edit {
            let mut close = false;
            let (ref obj_type, obj_idx, ref mut comment) = *comment_state;
            egui::Window::new(format!("Comment - {} #{}", obj_type, obj_idx))
                .default_size([400.0, 200.0])
                .show(ctx, |ui| {
                    ui.label("Edit object comment/notes:");
                    ui.text_edit_multiline(comment);
                    if ui.button("Save").clicked() {
                        if let Some(ref mut module) = self.module {
                            save_object_comment(module, &self.current_area.as_ref().map(|a| a.resref.clone()).unwrap_or_default(), obj_type, obj_idx, comment);
                        }
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            if close {
                self.object_comment_edit = None;
            }
        }

        // Inventory editor
        if let Some(ref mut inv) = self.inventory_editor {
            let mut close = false;
            egui::Window::new(format!("Inventory - {} #{}", inv.obj_type, inv.obj_idx))
                .default_size([500.0, 400.0])
                .show(ctx, |ui| {
                    if let Some(edit_idx) = inv.editing_item {
                        if let Some(item) = inv.items.get_mut(edit_idx) {
                            ui.heading(format!("Item #{}", edit_idx));
                            ui.separator();
                            let modified = show_bag_editor(ui, item);
                            if modified {
                                let resref = self.current_area.as_ref().map(|a| a.resref.clone()).unwrap_or_default();
                                save_item_list(self.module.as_mut(), &resref, &inv.obj_type, inv.obj_idx, &inv.items);
                            }
                            if ui.button("← Back to List").clicked() {
                                inv.editing_item = None;
                            }
                        } else {
                            inv.editing_item = None;
                        }
                    } else {
                        ui.horizontal(|ui| {
                            ui.heading("Items");
                            if ui.button("+ Add from Blueprint").clicked() {
                                let mut results = Vec::new();
                                if let Some(ref module) = self.module {
                                    if let Some(ref erf) = module.erf {
                                        for r in erf.resources() {
                                            if r.restype == nwn_gamemodel::resource_types::UTI {
                                                results.push((r.resref.clone(), r.restype));
                                            }
                                        }
                                    }
                                }
                                inv.add_blueprint_filter = String::new();
                                inv.add_blueprint_results = results;
                            }
                        });
                        ui.separator();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let mut remove_idx: Option<usize> = None;
                            for (i, item) in inv.items.iter().enumerate() {
                                let res = item.properties.iter().find(|p| p.label == "InventoryRes")
                                    .map(|p| match &p.value { nwn_gamemodel::PropertyValue::ResRef(s) => s.clone(), nwn_gamemodel::PropertyValue::CExoString(s) => s.clone(), _ => format!("{:?}", p.value) })
                                    .unwrap_or_else(|| format!("Item #{}", i));
                                let extra = item.properties.iter().find(|p| p.label == "StackSize")
                                    .and_then(|p| match &p.value { nwn_gamemodel::PropertyValue::Int(v) => Some(*v), _ => None })
                                    .map(|s| format!(" (x{})", s))
                                    .unwrap_or_default();
                                ui.horizontal(|ui| {
                                    if ui.button("✏").clicked() {
                                        inv.editing_item = Some(i);
                                    }
                                    ui.label(format!("{}{}", res, extra));
                                    if ui.button("🗑").clicked() {
                                        remove_idx = Some(i);
                                    }
                                });
                            }
                            if let Some(idx) = remove_idx {
                                inv.items.remove(idx);
                                let resref = self.current_area.as_ref().map(|a| a.resref.clone()).unwrap_or_default();
                                save_item_list(self.module.as_mut(), &resref, &inv.obj_type, inv.obj_idx, &inv.items);
                            }
                        });
                        if !inv.add_blueprint_filter.is_empty() || !inv.add_blueprint_results.is_empty() {
                            ui.separator();
                            ui.label("Search blueprints:");
                            ui.text_edit_singleline(&mut inv.add_blueprint_filter);
                            let mut selected: Option<String> = None;
                            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                                let filter = inv.add_blueprint_filter.clone();
                                for (name, _rt) in &inv.add_blueprint_results {
                                    if !filter.is_empty() && !name.to_lowercase().contains(&filter.to_lowercase()) {
                                        continue;
                                    }
                                    if ui.selectable_label(false, name).clicked() {
                                        selected = Some(name.clone());
                                    }
                                }
                            });
                            if let Some(ref blueprint_name) = selected {
                                let mut item = PropertyBag::new();
                                item.properties.push(nwn_gamemodel::Property {
                                    label: "InventoryRes".into(),
                                    value: nwn_gamemodel::PropertyValue::ResRef(blueprint_name.clone()),
                                    field_type: 11,
                                    category: String::new(),
                                    description: String::new(),
                                });
                                item.properties.push(nwn_gamemodel::Property {
                                    label: "StackSize".into(),
                                    value: nwn_gamemodel::PropertyValue::Int(1),
                                    field_type: 5,
                                    category: String::new(),
                                    description: String::new(),
                                });
                                inv.items.push(item);
                                let resref = self.current_area.as_ref().map(|a| a.resref.clone()).unwrap_or_default();
                                save_item_list(self.module.as_mut(), &resref, &inv.obj_type, inv.obj_idx, &inv.items);
                                inv.add_blueprint_results.clear();
                                inv.add_blueprint_filter.clear();
                            }
                            if ui.button("Close Blueprint Picker").clicked() {
                                inv.add_blueprint_results.clear();
                                inv.add_blueprint_filter.clear();
                            }
                        }
                        ui.separator();
                        if ui.button("Close Inventory").clicked() {
                            close = true;
                        }
                    }
                });
            if close {
                self.inventory_editor = None;
            }
        }

        // ===================== GRAPH AREA VIEW =====================
        if self.graph_view_open {
            let mut open = true;
            // Clone area for read-only use inside the window closure
            let area_clone = self.current_area.clone();
            let mut out_selection: Option<(String, usize)> = None;
            let mut out_select_tile: Option<(usize, usize)> = None;
            let mut area_update: Option<(String, String, usize, f32, f32)> = None;
            egui::Window::new("Graph Area View")
                .default_size([500.0, 400.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    if let Some(ref area) = area_clone {
                        ui.horizontal(|ui| {
                            ui.label(format!("Area: {}  {}x{}", area.resref, area.width, area.height));
                            if ui.button("Reset View").clicked() {
                                self.graph_view_state.pan_x = 0.0;
                                self.graph_view_state.pan_y = 0.0;
                                self.graph_view_state.zoom = 8.0;
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.checkbox(&mut self.graph_view_state.filter_creatures, "Creatures");
                            ui.checkbox(&mut self.graph_view_state.filter_doors, "Doors");
                            ui.checkbox(&mut self.graph_view_state.filter_placeables, "Placeables");
                            ui.checkbox(&mut self.graph_view_state.filter_triggers, "Triggers");
                            ui.checkbox(&mut self.graph_view_state.filter_waypoints, "Waypoints");
                            ui.checkbox(&mut self.graph_view_state.filter_encounters, "Encounters");
                            ui.checkbox(&mut self.graph_view_state.filter_sound_emitters, "Sound");
                        });
                        ui.separator();
                        let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
                        let rect = resp.rect;
                        let aw = area.width as f32 * 10.0;
                        let ah = area.height as f32 * 10.0;
                        let zoom = self.graph_view_state.zoom.max(1.0);
                        let cx = rect.center().x + self.graph_view_state.pan_x;
                        let cy = rect.center().y + self.graph_view_state.pan_y;
                        let to_screen = |wx: f32, wy: f32| -> egui::Pos2 {
                            egui::pos2(
                                cx + (wx - aw / 2.0) * zoom,
                                cy - (wy - ah / 2.0) * zoom,
                            )
                        };
                        let to_world = |sx: f32, sy: f32| -> (f32, f32) {
                            let wx = (sx - cx) / zoom + aw / 2.0;
                            let wy = -(sy - cy) / zoom + ah / 2.0;
                            (wx, wy)
                        };
                        // Draw tiles
                        let tile_start_col = ((rect.left() - cx) / zoom + aw / 2.0 / 10.0).floor() as i32 - 1;
                        let tile_end_col = ((rect.right() - cx) / zoom + aw / 2.0 / 10.0).ceil() as i32 + 1;
                        let tile_start_row = (-(rect.bottom() - cy) / zoom + ah / 2.0 / 10.0).floor() as i32 - 1;
                        let tile_end_row = (-(rect.top() - cy) / zoom + ah / 2.0 / 10.0).ceil() as i32 + 1;
                        for row in tile_start_row.max(0)..tile_end_row.min(area.height as i32) {
                            for col in tile_start_col.max(0)..tile_end_col.min(area.width as i32) {
                                let idx = (row as usize) * area.width as usize + (col as usize);
                                let color = area.tiles.get(idx).map(|t| {
                                    let id = t.id as u32;
                                    let r = ((id * 137 + 53) % 200 + 55) as u8;
                                    let g = ((id * 251 + 97) % 200 + 55) as u8;
                                    let b = ((id * 199 + 173) % 200 + 55) as u8;
                                    egui::Color32::from_rgb(r, g, b)
                                }).unwrap_or(egui::Color32::from_gray(40));
                                let p0 = to_screen(col as f32 * 10.0, row as f32 * 10.0);
                                let p1 = to_screen((col + 1) as f32 * 10.0, (row + 1) as f32 * 10.0);
                                let t_rect = egui::Rect::from_min_max(
                                    egui::pos2(p0.x.min(p1.x), p0.y.min(p1.y)),
                                    egui::pos2(p0.x.max(p1.x), p0.y.max(p1.y)),
                                );
                                painter.rect_filled(t_rect, 0.0, color);
                                painter.rect_stroke(t_rect, 0.0, egui::Stroke::new(0.5, egui::Color32::from_gray(60)), egui::StrokeKind::Outside);
                            }
                        }
                        let selected = self.selected_obj_info.clone();
                        macro_rules! draw_objects {
                            ($list:expr, $typ:expr, $color:expr) => {{
                                for (i, obj) in $list.iter().enumerate() {
                                    let sp = to_screen(obj.x, obj.y);
                                    if !rect.contains(sp) { continue; }
                                    let is_sel = selected.as_ref().map_or(false, |(t, idx)| t == $typ && *idx == i);
                                    let radius = if is_sel { 6.0 } else { 4.0 };
                                    let fill = if is_sel { egui::Color32::WHITE } else { $color };
                                    painter.circle_filled(sp, radius, fill);
                                    if is_sel {
                                        painter.circle_stroke(sp, radius + 2.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
                                    }
                                    if zoom > 12.0 {
                                        let label_pos = egui::pos2(sp.x + 6.0, sp.y - 4.0);
                                        painter.text(label_pos, egui::Align2::LEFT_CENTER, &obj.graph_label(), egui::FontId::proportional(10.0), egui::Color32::WHITE);
                                    }
                                }
                            }};
                        }
                        macro_rules! hit_test_objects {
                            ($list:expr, $typ:expr, $hp:expr, $closest:expr) => {
                                for (i, obj) in $list.iter().enumerate() {
                                    let sp = to_screen(obj.x, obj.y);
                                    let dist = $hp.distance(sp);
                                    if dist < 12.0 {
                                        if $closest.as_ref().map_or(true, |(_, _, d)| dist < *d) {
                                            $closest = Some(($typ.to_string(), i, dist));
                                        }
                                    }
                                }
                            };
                        }
                        if self.graph_view_state.filter_creatures {
                            draw_objects!(area.creatures, "creature", egui::Color32::RED);
                        }
                        if self.graph_view_state.filter_doors {
                            draw_objects!(area.doors, "door", egui::Color32::YELLOW);
                        }
                        if self.graph_view_state.filter_placeables {
                            draw_objects!(area.placeables, "placeable", egui::Color32::BLUE);
                        }
                        if self.graph_view_state.filter_triggers {
                            draw_objects!(area.triggers, "trigger", egui::Color32::GREEN);
                        }
                        if self.graph_view_state.filter_waypoints {
                            draw_objects!(area.waypoints, "waypoint", egui::Color32::WHITE);
                        }
                        if self.graph_view_state.filter_encounters {
                            draw_objects!(area.encounters, "encounter", egui::Color32::from_rgb(255, 128, 0));
                        }
                        if self.graph_view_state.filter_sound_emitters {
                            draw_objects!(area.sound_emitters, "sound_emitter", egui::Color32::CYAN);
                        }
                        // Input handling
                        let hover_pos = resp.hover_pos();
                        let clicked = resp.clicked();
                        let drag_delta = resp.drag_delta();
                        let scroll = ui.input(|i| i.raw_scroll_delta);
                        if resp.hovered() && scroll.y != 0.0 {
                            let old_zoom = self.graph_view_state.zoom;
                            self.graph_view_state.zoom = (self.graph_view_state.zoom * (1.0 + scroll.y * 0.002)).clamp(1.0, 100.0);
                            if let Some(hp) = hover_pos {
                                let factor = self.graph_view_state.zoom / old_zoom;
                                let mx = hp.x - cx;
                                let my = hp.y - cy;
                                self.graph_view_state.pan_x = self.graph_view_state.pan_x * factor - mx * (factor - 1.0);
                                self.graph_view_state.pan_y = self.graph_view_state.pan_y * factor - my * (factor - 1.0);
                            }
                        }
                        if resp.dragged_by(egui::PointerButton::Secondary) || resp.dragged_by(egui::PointerButton::Middle) {
                            self.graph_view_state.pan_x += drag_delta.x;
                            self.graph_view_state.pan_y += drag_delta.y;
                        }
                        if clicked {
                            if let Some(hp) = hover_pos {
                                let mut closest: Option<(String, usize, f32)> = None;
                                if self.graph_view_state.filter_creatures {
                                    hit_test_objects!(area.creatures, "creature", hp, closest);
                                }
                                if self.graph_view_state.filter_doors {
                                    hit_test_objects!(area.doors, "door", hp, closest);
                                }
                                if self.graph_view_state.filter_placeables {
                                    hit_test_objects!(area.placeables, "placeable", hp, closest);
                                }
                                if self.graph_view_state.filter_triggers {
                                    hit_test_objects!(area.triggers, "trigger", hp, closest);
                                }
                                if self.graph_view_state.filter_waypoints {
                                    hit_test_objects!(area.waypoints, "waypoint", hp, closest);
                                }
                                if self.graph_view_state.filter_encounters {
                                    hit_test_objects!(area.encounters, "encounter", hp, closest);
                                }
                                if self.graph_view_state.filter_sound_emitters {
                                    hit_test_objects!(area.sound_emitters, "sound_emitter", hp, closest);
                                }
                                if let Some((typ, idx, _)) = closest {
                                    out_selection = Some((typ, idx));
                                } else {
                                    let (wx, wy) = to_world(hp.x, hp.y);
                                    let col = (wx / 10.0).floor() as usize;
                                    let row = (wy / 10.0).floor() as usize;
                                    if col < area.width as usize && row < area.height as usize {
                                        out_select_tile = Some((col, row));
                                    }
                                }
                            }
                        }
                        // Drag object to move (left-click drag on selected object)
                        if resp.dragged_by(egui::PointerButton::Primary) && resp.hovered() && self.selected_obj_info.is_some() {
                            if let Some(hp) = hover_pos {
                                if let Some((ref obj_type, obj_idx)) = self.selected_obj_info.clone() {
                                    let (wx, wy) = to_world(hp.x, hp.y);
                                    let (nx, ny) = snap_to_grid_pos(wx, wy, true, 10.0);
                                    area_update = Some((obj_type.clone(), area.resref.clone(), obj_idx, nx, ny));
                                }
                            }
                        }
                    } else {
                        ui.label("No area loaded.");
                    }
                });
            if !open {
                self.graph_view_open = false;
            }
            // Apply selection changes outside closure
            if let Some((ref typ, idx)) = out_selection {
                if let Some(ref area) = self.current_area {
                    let bag = match typ.as_str() {
                        "creature" => area.creatures.get(idx).map(|c| c.to_property_bag()),
                        "door" => area.doors.get(idx).map(|d| d.to_property_bag()),
                        "placeable" => area.placeables.get(idx).map(|p| p.to_property_bag()),
                        "trigger" => area.triggers.get(idx).map(|t| t.to_property_bag()),
                        "waypoint" => area.waypoints.get(idx).map(|w| w.to_property_bag()),
                        "encounter" => area.encounters.get(idx).map(|e| e.to_property_bag()),
                        "sound_emitter" => area.sound_emitters.get(idx).map(|s| s.to_property_bag()),
                        _ => None,
                    };
                    if let Some(bag) = bag {
                        self.selected_object = Some(SelectedObject { title: format!("{} #{}", typ, idx), bag });
                        self.selected_obj_info = Some((typ.clone(), idx));
                        self.selection_list.clear();
                        self.status = format!("Selected {} #{}", typ, idx);
                    }
                }
            }
            if let Some((col, row)) = out_select_tile {
                self.selected_tile = Some((col, row));
                self.status = format!("Selected tile ({}, {})", col, row);
            }
            // Apply drag-move updates outside closure
            if let Some((obj_type, area_resref, obj_idx, nx, ny)) = area_update {
                if let Some(ref mut area) = self.current_area {
                    let area_mut = Arc::make_mut(area);
                    match obj_type.as_str() {
                        "creature" => if let Some(obj) = area_mut.creatures.get_mut(obj_idx) { obj.x = nx; obj.y = ny; }
                        "door" => if let Some(obj) = area_mut.doors.get_mut(obj_idx) { obj.x = nx; obj.y = ny; }
                        "placeable" => if let Some(obj) = area_mut.placeables.get_mut(obj_idx) { obj.x = nx; obj.y = ny; }
                        "trigger" => if let Some(obj) = area_mut.triggers.get_mut(obj_idx) { obj.x = nx; obj.y = ny; }
                        "waypoint" => if let Some(obj) = area_mut.waypoints.get_mut(obj_idx) { obj.x = nx; obj.y = ny; }
                        "encounter" => if let Some(obj) = area_mut.encounters.get_mut(obj_idx) { obj.x = nx; obj.y = ny; }
                        "sound_emitter" => if let Some(obj) = area_mut.sound_emitters.get_mut(obj_idx) { obj.x = nx; obj.y = ny; }
                        _ => {}
                    }
                    if let Some(ref mut module) = self.module {
                        module.update_area_git(&area_resref, area);
                    }
                    self.area_version = self.area_version.wrapping_add(1);
                }
            }
        }

        // ===================== HEIGHT MAP VIEW =====================
        if self.height_map_open {
            let mut open = true;
            let mut area_modified = false;
            egui::Window::new("Height Map")
                .default_size([500.0, 400.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    let hms = &mut self.height_map_state;
                    ui.horizontal(|ui| {
                        ui.label("Brush:");
                        ui.add(egui::Slider::new(&mut hms.brush_size, 1..=10).text("Size"));
                        ui.add(egui::Slider::new(&mut hms.brush_amount, 1..=50).text("Amount"));
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Reset View").clicked() {
                            hms.pan_x = 0.0;
                            hms.pan_y = 0.0;
                            hms.zoom = 20.0;
                        }
                        ui.checkbox(&mut hms.show_heights, "Show Values");
                        ui.label("Left-click: raise  Shift+click: lower");
                    });
                    ui.separator();
                    if let Some(ref area) = self.current_area.clone() {
                        let (resp, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
                        let rect = resp.rect;
                        let aw = area.width as f32;
                        let ah = area.height as f32;
                        let zoom = hms.zoom.max(2.0);
                        let tile_px = zoom;
                        let cx = rect.center().x + hms.pan_x;
                        let cy = rect.center().y + hms.pan_y;
                        let to_screen = |col: f32, row: f32| -> egui::Pos2 {
                            egui::pos2(
                                cx + (col - aw / 2.0) * tile_px,
                                cy + (row - ah / 2.0) * tile_px,
                            )
                        };
                        let to_grid = |sx: f32, sy: f32| -> (i32, i32) {
                            let col = ((sx - cx) / tile_px + aw / 2.0).floor() as i32;
                            let row = ((sy - cy) / tile_px + ah / 2.0).floor() as i32;
                            (col, row)
                        };
                        let col_start = ((rect.left() - cx) / tile_px + aw / 2.0).floor() as i32 - 1;
                        let col_end = ((rect.right() - cx) / tile_px + aw / 2.0).ceil() as i32 + 1;
                        let row_start = ((rect.top() - cy) / tile_px + ah / 2.0).floor() as i32 - 1;
                        let row_end = ((rect.bottom() - cy) / tile_px + ah / 2.0).ceil() as i32 + 1;
                        for row in row_start.max(0)..row_end.min(area.height as i32) {
                            for col in col_start.max(0)..col_end.min(area.width as i32) {
                                let idx = (row as usize) * area.width as usize + (col as usize);
                                let height = area.tiles.get(idx).map(|t| t.height).unwrap_or(0);
                                // Color gradient: 0=blue, 64=green, 128=yellow, 192=orange, 255=red
                                let (r, g, b) = height_to_color(height);
                                let p0 = to_screen(col as f32, row as f32);
                                let p1 = to_screen((col + 1) as f32, (row + 1) as f32);
                                let t_rect = egui::Rect::from_min_max(p0, p1);
                                painter.rect_filled(t_rect, 0.0, egui::Color32::from_rgb(r, g, b));
                                painter.rect_stroke(t_rect, 0.0, egui::Stroke::new(0.5, egui::Color32::from_gray(40)), egui::StrokeKind::Outside);
                                if hms.show_heights && zoom > 8.0 {
                                    let center = t_rect.center();
                                    painter.text(center, egui::Align2::CENTER_CENTER, &format!("{}", height), egui::FontId::proportional(8.0), egui::Color32::WHITE);
                                }
                            }
                        }
                        // Handle click to raise/lower
                        let clicked = resp.clicked();
                        let dragged = resp.dragged_by(egui::PointerButton::Primary);
                        let scroll = ui.input(|i| i.raw_scroll_delta);
                        if resp.hovered() && scroll.y != 0.0 {
                            hms.zoom = (hms.zoom * (1.0 + scroll.y * 0.003)).clamp(2.0, 200.0);
                        }
                        if resp.dragged_by(egui::PointerButton::Secondary) || resp.dragged_by(egui::PointerButton::Middle) {
                            hms.pan_x += resp.drag_delta().x;
                            hms.pan_y += resp.drag_delta().y;
                        }
                        if (clicked || dragged) && resp.hovered() {
                            if let Some(hp) = resp.hover_pos() {
                                let (gcol, grow) = to_grid(hp.x, hp.y);
                                let half = hms.brush_size / 2;
                                let shift = ui.input(|i| i.modifiers.shift);
                                let amount = if shift { -hms.brush_amount } else { hms.brush_amount };
                                self.push_undo();
                                if let Some(ref mut area_mut) = self.current_area {
                                    let area_mut = Arc::make_mut(area_mut);
                                    for dr in -half..=half {
                                        for dc in -half..=half {
                                            let r = grow + dr;
                                            let c = gcol + dc;
                                            if r >= 0 && r < area_mut.height && c >= 0 && c < area_mut.width {
                                                let idx = (r as usize) * area_mut.width as usize + (c as usize);
                                                let new_h = (area_mut.tiles[idx].height + amount).clamp(0, 255);
                                                area_mut.tiles[idx].height = new_h;
                                            }
                                        }
                                    }
                                    area_modified = true;
                                }
                            }
                        }
                    } else {
                        ui.label("No area loaded.");
                    }
                });
            if !open {
                self.height_map_open = false;
            }
            if area_modified {
                if let Some(ref area) = self.current_area {
                    let resref = area.resref.clone();
                    if let Some(ref mut module) = self.module {
                        module.update_area_are(&resref, area);
                    }
                    self.area_version = self.area_version.wrapping_add(1);
                    self.viewport.set_area(area.clone(), self.area_version, target_format);
                    self.status = "Height map edited.".to_string();
                }
            }
        }

        // ===================== AREA TRANSITION EDITOR =====================
        if self.transition_editor_open {
            let mut open = true;
            let mut saved_any = false;
            let mut save_results: Vec<(String, String, String, String)> = Vec::new();
            egui::Window::new("Area Transition Editor")
                .default_size([700.0, 500.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    if let Some(ref module) = self.module {
                        ui.label("Objects with transitions (doors/triggers that link to another area).");
                        ui.separator();
                        let area_names = module.area_names();
                        if area_names.is_empty() {
                            ui.label("No areas in module.");
                            return;
                        }
                        egui::ScrollArea::both().show(ui, |ui| {
                            for area_resref in area_names {
                                let git_key = area_resref.trim().to_lowercase();
                                let git_data = module.raw_git.get(&git_key);
                                let gff = git_data.and_then(|d| nwn_gff::GffFile::from_bytes(d).ok());
                                if gff.is_none() { continue; }
                                let gff = gff.unwrap();
                                let mut has_any = false;
                                // Check Door_List for transitions
                                if let Some(door_list) = gff.root().find("Door_List").and_then(|fv| match fv { nwn_gff::FieldValue::List(l) => Some(l), _ => None }) {
                                    for (i, door) in door_list.iter().enumerate() {
                                        let is_transition = door.find("Transition").and_then(|fv| match fv { nwn_gff::FieldValue::Int(v) => Some(*v != 0), _ => None }).unwrap_or(false);
                                        if !is_transition { continue; }
                                        has_any = true;
                                        let tag = door.find("Tag").and_then(|fv| match fv { nwn_gff::FieldValue::String(s) => Some(s.clone()), _ => None }).unwrap_or_default();
                                        let dest_area = door.find("TransitionDestin").and_then(|fv| match fv { nwn_gff::FieldValue::ResRef(s) => Some(s.clone()), nwn_gff::FieldValue::String(s) => Some(s.clone()), _ => None }).unwrap_or_default();
                                        let dest_tag = door.find("TransitionDestinTag").and_then(|fv| match fv { nwn_gff::FieldValue::String(s) => Some(s.clone()), _ => None }).unwrap_or_default();
                                        let header = format!("Door \"{}\" -> {} ({})", tag, dest_area, dest_tag);
                                        let id = egui::Id::new(("trans_door", area_resref.clone(), i));
                                        egui::CollapsingHeader::new(header)
                                            .id_salt(id)
                                            .default_open(false)
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label("Destination Area:");
                                                    let area_combo_id = egui::Id::new(("trans_area_combo", area_resref.clone(), i));
                                                    egui::ComboBox::from_id_salt(area_combo_id)
                                                        .selected_text(&dest_area)
                                                        .show_ui(ui, |ui| {
                                                            for an in area_names {
                                                                if ui.selectable_label(false, an).clicked() && an != &dest_area {
                                                                    save_results.push((area_resref.clone(), "Door_List".to_string(), i.to_string(), format!("TransitionDestin:{}", an)));
                                                                }
                                                            }
                                                        });
                                                });
                                                ui.horizontal(|ui| {
                                                    ui.label("To Waypoint:");
                                                    let new_tag = &mut dest_tag.clone();
                                                    if ui.text_edit_singleline(new_tag).changed() {
                                                    }
                                                    if ui.button("Save Waypoint").clicked() {
                                                        let val = new_tag.clone();
                                                        save_results.push((area_resref.clone(), "Door_List".to_string(), i.to_string(), format!("TransitionDestinTag:{}", val)));
                                                    }
                                                });
                                            });
                                    }
                                }
                                // Check Trigger_List for transitions
                                if let Some(trigger_list) = gff.root().find("Trigger_List").and_then(|fv| match fv { nwn_gff::FieldValue::List(l) => Some(l), _ => None }) {
                                    for (i, trigger) in trigger_list.iter().enumerate() {
                                        let transition = trigger.find("Transition").and_then(|fv| match fv { nwn_gff::FieldValue::Int(v) => Some(*v != 0), _ => None }).unwrap_or(false);
                                        let has_destin = trigger.find("TransitionDestin").is_some();
                                        if !transition && !has_destin { continue; }
                                        has_any = true;
                                        let tag = trigger.find("Tag").and_then(|fv| match fv { nwn_gff::FieldValue::String(s) => Some(s.clone()), _ => None }).unwrap_or_default();
                                        let dest_area = trigger.find("TransitionDestin").and_then(|fv| match fv { nwn_gff::FieldValue::ResRef(s) => Some(s.clone()), nwn_gff::FieldValue::String(s) => Some(s.clone()), _ => None }).unwrap_or_default();
                                        let dest_tag = trigger.find("TransitionDestinTag").and_then(|fv| match fv { nwn_gff::FieldValue::String(s) => Some(s.clone()), _ => None }).unwrap_or_default();
                                        let header = format!("Trigger \"{}\" -> {} ({})", tag, dest_area, dest_tag);
                                        let id = egui::Id::new(("trans_trigger", area_resref.clone(), i));
                                        egui::CollapsingHeader::new(header)
                                            .id_salt(id)
                                            .default_open(false)
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label("Destination Area:");
                                                    let area_combo_id = egui::Id::new(("trans_trig_combo", area_resref.clone(), i));
                                                    egui::ComboBox::from_id_salt(area_combo_id)
                                                        .selected_text(&dest_area)
                                                        .show_ui(ui, |ui| {
                                                            for an in area_names {
                                                                if ui.selectable_label(false, an).clicked() && an != &dest_area {
                                                                    save_results.push((area_resref.clone(), "Trigger_List".to_string(), i.to_string(), format!("TransitionDestin:{}", an)));
                                                                }
                                                            }
                                                        });
                                                });
                                                ui.horizontal(|ui| {
                                                    ui.label("To Waypoint:");
                                                    let new_tag = &mut dest_tag.clone();
                                                    ui.text_edit_singleline(new_tag);
                                                    if ui.button("Save Waypoint").clicked() {
                                                        let val = new_tag.clone();
                                                        save_results.push((area_resref.clone(), "Trigger_List".to_string(), i.to_string(), format!("TransitionDestinTag:{}", val)));
                                                    }
                                                });
                                            });
                                    }
                                }
                                if has_any {
                                    ui.separator();
                                }
                            }
                        });
                    } else {
                        ui.label("No module loaded.");
                    }
                });
            if !open {
                self.transition_editor_open = false;
            }
            // Apply saves outside closure
            for (area_resref, list_name, idx_str, action) in save_results {
                let idx: usize = match idx_str.parse() { Ok(i) => i, Err(_) => continue };
                let git_key = area_resref.trim().to_lowercase();
                let data = match self.module.as_ref().and_then(|m| m.raw_git.get(&git_key)) {
                    Some(d) => d.clone(),
                    None => continue,
                };
                let mut gff = match nwn_gff::GffFile::from_bytes(&data) { Ok(g) => g, Err(_) => continue };
                let list_fv = match gff.root_mut().find_mut(&list_name) {
                    Some(fv) => fv,
                    None => continue,
                };
                let list = match list_fv {
                    nwn_gff::FieldValue::List(l) => l,
                    _ => continue,
                };
                let obj_struct = match list.get_mut(idx) {
                    Some(s) => s,
                    None => continue,
                };
                let parts: Vec<&str> = action.splitn(2, ':').collect();
                if parts.len() != 2 { continue; }
                let field_name = parts[0];
                let field_value = parts[1];
                let new_field_value = if field_name == "TransitionDestin" { nwn_gff::FieldValue::ResRef(field_value.to_string()) }
                                 else { nwn_gff::FieldValue::String(field_value.to_string()) };
                // Look for existing field and update, or push new
                let mut found = false;
                for f in obj_struct.fields.iter_mut() {
                    let label_str: String = f.label.to_string();
                    if label_str == field_name {
                        f.value = new_field_value;
                        found = true;
                        break;
                    }
                }
                if !found {
                    use std::str::FromStr;
                    let fss: nwn_lib_rs::parsing::FixedSizeString<16> = FromStr::from_str(field_name).unwrap_or_default();
                    obj_struct.fields.push(nwn_gff::Field { label: std::rc::Rc::new(fss), value: if field_name == "TransitionDestin" { nwn_gff::FieldValue::ResRef(field_value.to_string()) } else { nwn_gff::FieldValue::String(field_value.to_string()) } });
                }
                let new_data = gff.to_bytes();
                if let Some(ref mut module) = self.module {
                    module.raw_git.insert(git_key.to_string(), new_data);
                    module.modified = true;
                }
                saved_any = true;
            }
            if saved_any {
                // Reload current area if it's the one we edited
                if let Some(ref area) = self.current_area {
                    let key = area.resref.trim().to_lowercase();
                    if let Some(ref mut module) = self.module {
                        if let Some(git_data) = module.raw_git.get(&key) {
                            if let Ok(git_gff) = nwn_gff::GffFile::from_bytes(git_data) {
                                let area_resref = area.resref.clone();
                                let area_mut = Arc::make_mut(self.current_area.as_mut().unwrap());
                                nwn_gamemodel::reload_from_git(area_mut, &git_gff);
                                module.update_area_git(&area_resref, area_mut);
                            }
                        }
                    }
                    self.area_version = self.area_version.wrapping_add(1);
                    self.viewport.set_area(self.current_area.clone().unwrap(), self.area_version, target_format);
                }
                self.status = "Transition(s) saved.".to_string();
            }
        }

        // ===================== MODULE TREE VIEW =====================
        if self.module_tree_open {
            let mut open = true;
            egui::Window::new("Module Tree View")
                .default_size([350.0, 600.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    if let Some(ref module) = self.module {
                        egui::ScrollArea::both().show(ui, |ui| {
                            // Areas
                            let area_names = module.area_names();
                            egui::CollapsingHeader::new(format!("Areas ({})", area_names.len()))
                                .default_open(true)
                                .show(ui, |ui| {
                                    for name in area_names {
                                        let git_key = name.trim().to_lowercase();
                                        let counts = module.raw_git.get(&git_key).and_then(|d| {
                                            nwn_gff::GffFile::from_bytes(d).ok()
                                        }).map(|gff| {
                                            let cats = ["Creature_List", "Door_List", "Placeable_List", "Trigger_List", "Waypoint_List", "Encounter_List", "SoundEmitter_List"];
                                            let names = ["creatures", "doors", "placeables", "triggers", "waypoints", "encounters", "sounds"];
                                            let mut parts = Vec::new();
                                            for (i, cat) in cats.iter().enumerate() {
                                                let count = gff.root().find(cat).and_then(|fv| match fv { nwn_gff::FieldValue::List(l) => Some(l.len()), _ => None }).unwrap_or(0);
                                                if count > 0 { parts.push(format!("{}{}", count, &names[i][..1])); }
                                            }
                                            parts.join(" ")
                                        }).unwrap_or_default();
                                        if ui.selectable_label(false, format!("  {}  [{}]", name, counts)).clicked() {
                                            // We can't switch areas here easily due to borrows, just show a status
                                            self.status = format!("Double-click area '{}' in area list to switch.", name);
                                        }
                                    }
                                });
                            // Scripts
                            let scripts: Vec<String> = module.erf.as_ref().map(|erf| {
                                erf.resources().iter().filter(|r| r.restype == resource_types::NSS).map(|r| format!("{}.nss", r.resref)).collect()
                            }).unwrap_or_default();
                            egui::CollapsingHeader::new(format!("Scripts ({})", scripts.len()))
                                .default_open(false)
                                .show(ui, |ui| {
                                    for s in &scripts {
                                        if ui.selectable_label(false, format!("  {}", s)).clicked() {
                                            self.status = format!("Open '{}' in Script Editor via Tools menu.", s);
                                        }
                                    }
                                });
                            // Conversations
                            let convos: Vec<String> = module.erf.as_ref().map(|erf| {
                                erf.resources().iter().filter(|r| r.restype == resource_types::DLG).map(|r| format!("{}.dlg", r.resref)).collect()
                            }).unwrap_or_default();
                            egui::CollapsingHeader::new(format!("Conversations ({})", convos.len()))
                                .default_open(false)
                                .show(ui, |ui| {
                                    for c in &convos {
                                        ui.label(format!("  {}", c));
                                    }
                                });
                            // Blueprints grouped by type
                            let blueprint_types = [
                                (resource_types::UTC, "Creatures (UTC)"),
                                (resource_types::UTD, "Doors (UTD)"),
                                (resource_types::UTP, "Placeables (UTP)"),
                                (resource_types::UTT, "Triggers (UTT)"),
                                (resource_types::UTW, "Waypoints (UTW)"),
                                (resource_types::UTE, "Encounters (UTE)"),
                                (resource_types::UTS, "Sounds (UTS)"),
                                (resource_types::UTI, "Items (UTI)"),
                                (resource_types::UTM, "Stores (UTM)"),
                            ];
                            for &(restype, header) in &blueprint_types {
                                let items: Vec<String> = module.erf.as_ref().map(|erf| {
                                    erf.resources().iter().filter(|r| r.restype == restype).map(|r| r.resref.clone()).collect()
                                }).unwrap_or_default();
                                if items.is_empty() { continue; }
                                egui::CollapsingHeader::new(format!("{} ({})", header, items.len()))
                                    .default_open(false)
                                    .show(ui, |ui| {
                                        for item in &items {
                                            ui.label(format!("  {}", item));
                                        }
                                    });
                            }
                            // 2DAs
                            let twodas: Vec<String> = module.erf.as_ref().map(|erf| {
                                erf.resources().iter().filter(|r| r.restype == resource_types::TWO_DA).map(|r| r.resref.clone()).collect()
                            }).unwrap_or_default();
                            egui::CollapsingHeader::new(format!("2DAs ({})", twodas.len()))
                                .default_open(false)
                                .show(ui, |ui| {
                                    for t in &twodas {
                                        ui.label(format!("  {}", t));
                                    }
                                });
                        });
                    } else {
                        ui.label("No module loaded.");
                    }
                });
            if !open {
                self.module_tree_open = false;
            }
        }

        // ===================== FIND/REPLACE IN SCRIPTS =====================
        if self.find_replace_open {
            let mut open = true;
            egui::Window::new("Find/Replace in Scripts")
                .default_size([600.0, 400.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    let fr = &mut self.find_replace_state;
                    ui.horizontal(|ui| {
                        ui.label("Find:");
                        ui.text_edit_singleline(&mut fr.find_text);
                        if ui.button("Search").clicked() && !fr.find_text.is_empty() {
                            fr.results.clear();
                            if let Some(ref module) = self.module {
                                if let Some(ref erf) = module.erf {
                                    for res in erf.resources() {
                                        if res.restype != resource_types::NSS { continue; }
                                        if let Some(data) = erf.get(&res.resref, resource_types::NSS) {
                                            if let Ok(text) = String::from_utf8(data.clone()) {
                                                for (ln, line) in text.lines().enumerate() {
                                                    if line.to_lowercase().contains(&fr.find_text.to_lowercase()) {
                                                        fr.results.push(FindResult {
                                                            script_name: res.resref.clone(),
                                                            line_number: ln + 1,
                                                            line_text: line.to_string(),
                                                        });
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            fr.show_results = true;
                        }
                    });
                    if !fr.replace_text.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("Replace:");
                            ui.text_edit_singleline(&mut fr.replace_text);
                            if ui.button("Replace All").clicked() && !fr.find_text.is_empty() {
                                if let Some(ref mut module) = self.module {
                                    let find_lower = fr.find_text.to_lowercase();
                                    let mut count = 0;
                                    let new_resources: Vec<(String, u16, Vec<u8>)> = module.erf.as_ref().map(|erf| {
                                        erf.resources().iter().filter_map(|r| {
                                            if r.restype != resource_types::NSS { return None; }
                                            let data = erf.get(&r.resref, resource_types::NSS)?;
                                            let text = String::from_utf8(data.clone()).ok()?;
                                            if !text.to_lowercase().contains(&find_lower) { return None; }
                                            let new_text = text.replace(&fr.find_text, &fr.replace_text);
                                            count += 1;
                                            Some((r.resref.clone(), r.restype, new_text.into_bytes()))
                                        }).collect()
                                    }).unwrap_or_default();
                                    for (resref, _restype, data) in new_resources {
                                        module.update_script(&resref, &String::from_utf8_lossy(&data));
                                    }
                                    fr.show_results = false;
                                    fr.results.clear();
                                    self.status = format!("Replaced {} occurrence(s) in {} script(s).", count, count);
                                }
                            }
                        });
                    }
                    ui.separator();
                    if fr.show_results {
                        ui.label(format!("{} match(es)", fr.results.len()));
                        egui::ScrollArea::both().show(ui, |ui| {
                            egui::Grid::new("find_results").striped(true).show(ui, |ui| {
                                for r in &fr.results {
                                    ui.label(&r.script_name);
                                    ui.label(r.line_number.to_string());
                                    ui.label(&r.line_text);
                                    ui.end_row();
                                }
                            });
                        });
                    } else if self.module.is_some() {
                        ui.label("Enter search text and click 'Search'.");
                    } else {
                        ui.label("No module loaded.");
                    }
                });
            if !open {
                self.find_replace_open = false;
            }
        }

        // ===================== SOUND BROWSER =====================
        if self.sound_browser_open {
            let mut open = true;
            let audio_types = [resource_types::WAV];
            egui::Window::new("Sound Browser")
                .default_size([400.0, 500.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    if let Some(ref module) = self.module {
                        if let Some(ref erf) = module.erf {
                            let resources = erf.resources();
                            let sounds: Vec<&nwn_erf::ErfResource> = resources.iter().filter(|r| audio_types.contains(&r.restype)).collect();
                            if sounds.is_empty() {
                                ui.label("No audio resources found in module.");
                                return;
                            }
                            ui.label(format!("{} audio resources", sounds.len()));
                            ui.separator();
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for res in &sounds {
                                    let ext = if res.restype == resource_types::WAV { "wav" } else { "bmu" };
                                    let label = format!("{}.{}", res.resref, ext);
                                    ui.horizontal(|ui| {
                                        ui.label(&label);
                                        if ui.button("▶ Play").clicked() {
                                            if let Some(data) = erf.get(&res.resref, res.restype) {
                                                let tmp_path = format!("/tmp/opencode/{}.{}", res.resref, ext);
                                                let _ = std::fs::write(&tmp_path, &data);
                                                // Try aplay, fallback to ffplay
                                                let result = std::process::Command::new("aplay")
                                                    .arg(&tmp_path).arg("-q").spawn();
                                                if result.is_err() {
                                                    let _ = std::process::Command::new("ffplay")
                                                        .arg(&tmp_path).arg("-nodisp").arg("-autoexit").arg("-loglevel").arg("quiet").spawn();
                                                }
                                            }
                                        }
                                        if ui.button("Export...").clicked() {
                                            if let Some(data) = erf.get(&res.resref, res.restype) {
                                                let ext = "wav";
                                                if let Some(path) = pick_save_path_filtered(&format!("{}.{}", res.resref, ext), "Audio", &[ext]) {
                                                    let _ = std::fs::write(&path, &data);
                                                }
                                            }
                                        }
                                    });
                                }
                            });
                        } else {
                            ui.label("No ERF loaded.");
                        }
                    } else {
                        ui.label("No module loaded.");
                    }
                });
            if !open {
                self.sound_browser_open = false;
            }
        }

        // ===================== STORE EDITOR =====================
        if self.store_editor_open {
            let mut open = true;
            let mut open_bag_editor: Option<(String, u16, PropertyBag)> = None;
            egui::Window::new("Store Editor")
                .default_size([500.0, 400.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    if let Some(ref module) = self.module {
                        if let Some(ref erf) = module.erf {
                            let resources = erf.resources();
                            let store_res: Vec<&nwn_erf::ErfResource> = resources.iter().filter(|r| r.restype == resource_types::UTM).collect();
                            if store_res.is_empty() {
                                ui.label("No store blueprints (UTM) found.");
                                return;
                            }
                            ui.label(format!("{} store(s)", store_res.len()));
                            ui.separator();
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for store in &store_res {
                                    let data = erf.get(&store.resref, resource_types::UTM);
                                    let bag = data.and_then(|d| nwn_gff::GffFile::from_bytes(&d).ok()).map(|gff| PropertyBag::from_gff_struct(gff.root()));
                                    let store_name = bag.as_ref().and_then(|b| b.get_string("StoreName")).unwrap_or(&store.resref).to_string();
                                    if ui.button(format!("{} ({})", store_name, store.resref)).clicked() {
                                        if let Some(b) = bag {
                                            open_bag_editor = Some((store.resref.clone(), resource_types::UTM, b));
                                        }
                                    }
                                }
                            });
                        }
                    } else {
                        ui.label("No module loaded.");
                    }
                });
            if !open {
                self.store_editor_open = false;
            }
        }

        // ===================== ID WHOIS UTILITY =====================
        if self.id_whois_open {
            let mut open = true;
            egui::Window::new("ID Whois Utility")
                .default_size([500.0, 500.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    if let Some(ref module) = self.module {
                        ui.label("Enter a ResRef to find where it is used across the module:");
                        ui.text_edit_singleline(&mut self.search_term);
                        if ui.button("Search").clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            self.search_results.clear();
                            let term = self.search_term.to_lowercase();
                            if term.is_empty() { return; }
                            let area_names = module.area_names();

                            // Search in loaded area names/refs
                            for aname in area_names {
                                if aname.to_lowercase().contains(&term) {
                                    self.search_results.push(format!("Area ResRef: {}", aname));
                                }
                                if let Some(area) = module.get_area(aname) {
                                    if area.tag.to_lowercase().contains(&term) {
                                        self.search_results.push(format!("Area '{}' (tag: {})", aname, area.tag));
                                    }
                                    if area.name.to_lowercase().contains(&term) {
                                        self.search_results.push(format!("Area '{}' (name: {})", aname, area.name));
                                    }
                                    // Check object resrefs and tags
                                    for c in &area.creatures {
                                        if c.resref.to_lowercase().contains(&term) || c.tag.to_lowercase().contains(&term) {
                                            self.search_results.push(format!("Area {} / Creature '{}' (tag: {})", aname, c.resref, c.tag));
                                        }
                                    }
                                    for d in &area.doors {
                                        if d.resref.to_lowercase().contains(&term) || d.tag.to_lowercase().contains(&term) {
                                            self.search_results.push(format!("Area {} / Door '{}' (tag: {})", aname, d.resref, d.tag));
                                        }
                                    }
                                    for p in &area.placeables {
                                        if p.resref.to_lowercase().contains(&term) || p.tag.to_lowercase().contains(&term) {
                                            self.search_results.push(format!("Area {} / Placeable '{}' (tag: {})", aname, p.resref, p.tag));
                                        }
                                    }
                                    for t in &area.triggers {
                                        if t.resref.to_lowercase().contains(&term) || t.tag.to_lowercase().contains(&term) {
                                            self.search_results.push(format!("Area {} / Trigger '{}' (tag: {})", aname, t.resref, t.tag));
                                        }
                                    }
                                    for w in &area.waypoints {
                                        if w.tag.to_lowercase().contains(&term) {
                                            self.search_results.push(format!("Area {} / Waypoint '{}' (tag: {})", aname, w.resref, w.tag));
                                        }
                                    }
                                    for e in &area.encounters {
                                        if e.tag.to_lowercase().contains(&term) {
                                            self.search_results.push(format!("Area {} / Encounter (tag: {})", aname, e.tag));
                                        }
                                    }
                                    for s in &area.sound_emitters {
                                        if s.tag.to_lowercase().contains(&term) {
                                            self.search_results.push(format!("Area {} / Sound Emitter (tag: {})", aname, s.tag));
                                        }
                                    }
                                }
                            }

                            // Search ERF resource names
                            if let Some(ref erf) = module.erf {
                                for res in erf.resources() {
                                    let resref_lower = res.resref.to_lowercase();
                                    if resref_lower.contains(&term) {
                                        let ext = resource_types::type_to_ext(res.restype).unwrap_or("?");
                                        self.search_results.push(format!("Resource: {}.{}", res.resref, ext));
                                    }
                                    // Search script contents
                                    if res.restype == resource_types::NSS {
                                        if let Some(data) = erf.get(&res.resref, resource_types::NSS) {
                                            if let Ok(content) = String::from_utf8(data) {
                                                for (lineno, line) in content.lines().enumerate() {
                                                    if line.to_lowercase().contains(&term) {
                                                        self.search_results.push(format!("Script {}:{} — {}", res.resref, lineno + 1, line.trim()));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        ui.separator();
                        ui.label(format!("{} result(s)", self.search_results.len()));
                        ui.separator();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for result in &self.search_results {
                                ui.label(result);
                            }
                        });
                    } else {
                        ui.label("No module loaded.");
                    }
                });
            if !open {
                self.id_whois_open = false;
            }
        }

        // Area Geometry Editor
        if self.geometry_editor.open {
            show_geometry_editor(&mut self.geometry_editor, ctx, self.current_area.as_ref());
            // Apply geometry changes from editor
            if let Some((typ, idx, new_geom)) = self.geometry_editor.commit.take() {
                self.push_undo();
                if let Some(ref mut module) = self.module {
                    if let Some(area) = self.current_area.as_mut() {
                        let area = Arc::make_mut(area);
                        match typ.as_str() {
                            "trigger" => {
                                if idx < area.triggers.len() {
                                    area.triggers[idx].geometry = new_geom;
                                    module.update_area_git(&area.resref, area);
                                }
                            }
                            "encounter" => {
                                if idx < area.encounters.len() {
                                    area.encounters[idx].geometry = new_geom;
                                    module.update_area_git(&area.resref, area);
                                }
                            }
                            _ => {}
                        }
                        self.area_version = self.area_version.wrapping_add(1);
                        self.viewport.set_area(Arc::new(area.clone()), self.area_version, target_format);
                    }
                }
            }
        }

        // Sky color override window
        if self.show_sky_override || self.clear_override_enabled {
            egui::Window::new("Sky Color Override")
                .default_size([200.0, 100.0])
                .show(ctx, |ui| {
                    let mut col = self.clear_override.unwrap_or([128, 150, 200]);
                    if ui.color_edit_button_srgb(&mut col).changed() {
                        self.clear_override = Some(col);
                        let c = self.clear_override.unwrap_or([128, 150, 200]);
                        self.viewport.set_clear_color(wgpu::Color {
                            r: c[0] as f64 / 255.0,
                            g: c[1] as f64 / 255.0,
                            b: c[2] as f64 / 255.0,
                            a: 1.0,
                        });
                    }
                    if ui.button("Reset to Area Default").clicked() {
                        self.clear_override = None;
                        self.clear_override_enabled = false;
                        self.area_version = self.area_version.wrapping_add(1);
                    }
                });
        }

        // Environment Settings dialog
        if self.env_settings_open {
            egui::Window::new("Environment Settings")
                .default_size([280.0, 400.0])
                .show(ctx, |ui| {
                    ui.label("Sun (Day)");
                    ui.horizontal(|ui| {
                        ui.label("Fog Amount");
                        ui.add(egui::Slider::new(&mut self.env_sun_fog_amount, 0..=255));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Fog Color");
                        ui.color_edit_button_srgb(&mut self.env_sun_fog_color);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Ambient");
                        ui.color_edit_button_srgb(&mut self.env_sun_ambient_color);
                    });
                    ui.separator();
                    ui.label("Moon (Night)");
                    ui.horizontal(|ui| {
                        ui.label("Fog Amount");
                        ui.add(egui::Slider::new(&mut self.env_moon_fog_amount, 0..=255));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Fog Color");
                        ui.color_edit_button_srgb(&mut self.env_moon_fog_color);
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label("Fog Clip Distance");
                        ui.add(egui::Slider::new(&mut self.env_fog_clip_dist, 1.0..=500.0));
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            self.push_undo();
                            if let Some(ref mut area) = self.current_area {
                                let area_mut = Arc::make_mut(area);
                                area_mut.sun_fog_amount = self.env_sun_fog_amount;
                                area_mut.sun_fog_color = self.env_sun_fog_color;
                                area_mut.moon_fog_amount = self.env_moon_fog_amount;
                                area_mut.moon_fog_color = self.env_moon_fog_color;
                                area_mut.fog_clip_dist = self.env_fog_clip_dist;
                                area_mut.sun_ambient_color = self.env_sun_ambient_color;
                                let resref = area_mut.resref.clone();
                                if let Some(ref mut module) = self.module {
                                    module.update_area_are(&resref, area_mut);
                                }
                                self.area_version = self.area_version.wrapping_add(1);
                                self.viewport.set_area(area.clone(), self.area_version, target_format);
                                self.status = "Environment settings applied.".to_string();
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.env_settings_open = false;
                        }
                    });
                });
        }

        // Texture Browser window
        if self.texture_browser.open {
            show_texture_browser(ctx, &mut self.texture_browser, &self.module);
        }

        // Tileset Browser window
        if self.tileset_browser.open {
            show_tileset_browser(&mut self.tileset_browser, ctx, self.module.as_ref());
        }

        // Resource Viewer window
        if self.show_resource_viewer {
            egui::Window::new("Resource Viewer")
                .default_size([500.0, 400.0])
                .show(ctx, |ui| {
                    if let Some(ref module) = self.module {
                        show_resource_viewer(
                            ui, module,
                            &mut save_2da,
                            &mut open_script,
                            &mut open_conversation,
                            &mut edit_blueprint,
                            &mut edit_tlk,
                            &mut open_faction,
                            &mut open_journal,
                        );
                    } else {
                        ui.label("No module loaded.");
                    }
                });
        }

        // Handle blueprint editor requests from resource viewer
        if let Some((resref, restype, bytes)) = edit_blueprint {
            if let Ok(gff) = nwn_gff::GffFile::from_bytes(&bytes) {
                let bag = nwn_gamemodel::PropertyBag::from_gff_struct(gff.root());
                self.blueprint_editors.push((resref.clone(), restype, bag));
                self.status = format!("Opened blueprint editor for {}.{}", resref, type_to_ext(restype).unwrap_or("?"));
            }
        }

        // Blueprint Palette window
        if self.show_blueprint_palette {
            if let Some(ref module) = self.module {
                let mut place_cmd: Option<(String, String)> = None;
                egui::Window::new("Blueprint Palette")
                    .default_size([300.0, 500.0])
                    .show(ctx, |ui| {
                        show_blueprint_palette(ui, &module.resource_manager, &mut place_cmd);
                    });
                if let Some((typ, resref)) = place_cmd {
                    self.status = format!("Placement mode: {} {}. Click in viewport to place.", typ, resref);
                    self.placement_state = PlacementState::Pending(typ, resref);
                }
            }
        }

        // Item Browser window
        if self.item_browser_open {
            if let Some(ref module) = self.module {
                egui::Window::new("Item Browser")
                    .default_size([400.0, 500.0])
                    .show(ctx, |ui| {
                        show_item_browser(ui, &module.resource_manager);
                    });
            }
        }

        // Script editors
        let mut scripts_to_remove = Vec::new();
        let mut scripts_to_compile: Vec<(String, String)> = Vec::new();
        for (i, (name, source)) in self.script_editor_scripts.iter_mut().enumerate() {
            let mut open = true;
            egui::Window::new(format!("Script: {}", name))
                .id(egui::Id::new(("script_editor", i)))
                .default_size([600.0, 400.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    let mut modified = false;
                    if show_script_editor(ui, name, source, &mut modified) {
                        scripts_to_compile.push((name.clone(), source.clone()));
                    }
                    if modified {
                        if let Some(ref mut module) = self.module {
                            module.update_script(name, source);
                            module.modified = true;
                            self.status = format!("Script '{}' updated.", name);
                        }
                    }
                });
            if !open {
                scripts_to_remove.push(i);
            }
        }
        for (name, source) in scripts_to_compile {
            self.compile_script(&name, &source);
        }
        for i in scripts_to_remove.into_iter().rev() {
            self.script_editor_scripts.remove(i);
        }

        // Conversation editors
        let mut convos_to_remove = Vec::new();
        for (i, (name, editor)) in self.conversation_editors.iter_mut().enumerate() {
            let mut open = true;
            egui::Window::new(format!("Conversation: {}", name))
                .id(egui::Id::new(("conversation_editor", i)))
                .default_size([500.0, 400.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    if show_conversation_editor(ui, name, &mut editor.convo) {
                        editor.modified = true;
                        if let Some(ref mut module) = self.module {
                            module.update_conversation(name, &editor.convo);
                            module.modified = true;
                            self.status = format!("Conversation '{}' updated.", name);
                        }
                    }
                });
            if !open {
                convos_to_remove.push(i);
            }
        }
        for i in convos_to_remove.into_iter().rev() {
            self.conversation_editors.remove(i);
        }

        // 2DA editors
        let mut twodas_to_remove = Vec::new();
        for (i, (name, state)) in self.two_da_editors.iter_mut().enumerate() {
            let mut open = true;
            egui::Window::new(format!("2DA: {}", name))
                .id(egui::Id::new(("two_da_editor", i)))
                .default_size([500.0, 400.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    let mut on_save: Option<(String, String)> = None;
                    show_two_da_editor(ui, state, &mut on_save);
                    if let Some((_name, data)) = on_save {
                        if let Some(ref mut module) = self.module {
                            module.update_2da(name, &data);
                            module.modified = true;
                            self.status = format!("2DA '{}' updated.", name);
                        }
                    }
                });
            if !open {
                twodas_to_remove.push(i);
            }
        }
        for i in twodas_to_remove.into_iter().rev() {
            self.two_da_editors.remove(i);
        }

        // Blueprint editors
        let mut bp_to_remove = Vec::new();
        for i in 0..self.blueprint_editors.len() {
            let (resref, restype, bag) = &mut self.blueprint_editors[i];
            let (resref_clone, restype_clone) = (resref.clone(), *restype);
            let ext = type_to_ext(restype_clone).unwrap_or("?");
            let mut open = true;
            egui::Window::new(format!("Blueprint: {}.{}", resref_clone, ext))
                .id(egui::Id::new(("blueprint_editor", i)))
                .default_size([400.0, 500.0])
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.label(format!("ResRef: {}  Type: {}", resref_clone, ext));
                    ui.separator();
                    show_bag_editor(ui, bag);
                    ui.separator();
                    if ui.button("Save to Module").clicked() {
                        if let Some(ref mut module) = self.module {
                            let struct_id = match restype_clone {
                                rt if rt == resource_types::UTC => 4,
                                rt if rt == resource_types::UTD => 5,
                                rt if rt == resource_types::UTP => 6,
                                rt if rt == resource_types::UTT => 7,
                                rt if rt == resource_types::UTW => 8,
                                rt if rt == resource_types::UTE => 9,
                                rt if rt == resource_types::UTS => 10,
                                _ => 0,
                            };
                            let ft = format!("{:<4}", ext);
                            let gff = bag.to_gff(&ft, "V3.2", struct_id);
                            let data = gff.to_bytes();
                            module.update_blueprint(&resref_clone, restype_clone, data);
                            self.status = format!("Blueprint '{}.{}' saved.", resref_clone, ext);
                        }
                    }
                });
            if !open {
                bp_to_remove.push(i);
            }
        }
        for i in bp_to_remove.into_iter().rev() {
            self.blueprint_editors.remove(i);
        }

        // ===================== AREA TABS =====================
        if !self.open_areas.is_empty() {
            let tab_resrefs = self.open_areas.clone();
            let active_idx = self.active_area_idx;
            let current_resref = self.current_area.as_ref().map(|a| a.resref.clone());
            egui::TopBottomPanel::top("area_tabs")
                .min_height(28.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        for (i, name) in tab_resrefs.iter().enumerate() {
                            let is_active = i == active_idx || current_resref.as_ref().map_or(false, |r| r == name);
                            let btn = egui::Button::new(
                                egui::RichText::new(name).size(14.0)
                            )
                            .fill(if is_active { ui.style().visuals.selection.bg_fill } else { ui.style().visuals.window_fill() })
                            .min_size(egui::vec2(60.0, 24.0));
                            let resp = ui.add(btn);
                            if resp.clicked() {
                                let name_to_switch = name.clone();
                                self.switch_to_area(&name_to_switch, target_format);
                            }
                            if ui.button("×").on_hover_text("Close tab").clicked() {
                                self.close_area_tab(i);
                                if self.current_area.is_none() && !self.open_areas.is_empty() {
                                    let next = self.open_areas[self.active_area_idx].clone();
                                    self.switch_to_area(&next, target_format);
                                }
                            }
                        }
                        ui.allocate_space(ui.available_size());
                    });
                });
        }

        // ===================== LEFT NAVIGATOR =====================
        let current_resref = self.current_area.as_ref().map(|a| a.resref.clone());
        egui::SidePanel::left("navigator")
            .resizable(true)
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Areas");
                    if self.module.is_some() {
                        if ui.button("+").on_hover_text("New Area").clicked() {
                            self.new_area_open = true;
                        }
                    }
                });
                ui.separator();
                let names: Vec<String> = self.module.as_ref().map(|m| m.area_names().iter().map(|s| s.to_string()).collect()).unwrap_or_default();
                if names.is_empty() {
                    ui.label("No module loaded.");
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut delete_name: Option<String> = None;
                        for name in &names {
                            let selected = current_resref.as_ref().map(|r| r == name).unwrap_or(false);
                            ui.horizontal(|ui| {
                                if ui.selectable_label(selected, name).clicked() {
                                    self.switch_to_area(name, target_format);
                                }
                                if selected {
                                    if ui.button("🗑").on_hover_text("Delete Area").clicked() {
                                        delete_name = Some(name.clone());
                                    }
                                    if ui.button("⚙").on_hover_text("Area Properties").clicked() {
                                        if let Some(ref area) = self.current_area {
                                            self.area_props_resref = area.resref.clone();
                                            self.area_props_name = area.name.clone();
                                            self.area_props_tag = area.tag.clone();
                                            self.area_props_tileset = area.tileset.clone();
                                            self.area_props_popup = true;
                                        }
                                    }
                                }
                            });
                        }
                        if let Some(name) = delete_name {
                            // Close tab before module mutable borrow
                            if let Some(tab_idx) = self.open_areas.iter().position(|r| r == &name) {
                                self.close_area_tab(tab_idx);
                            }
                            if let Some(ref mut module) = self.module {
                                module.area_names_list.retain(|n| n != &name);
                                module.modified_area_data.remove(&name.to_lowercase());
                                module.raw_git.remove(&name.to_lowercase());
                                module.modified = true;
                            }
                            // Switch to next open tab if one exists
                            if self.current_area.is_none() || self.current_area.as_ref().map_or(false, |a| a.resref == name) {
                                self.current_area = None;
                                self.selected_object = None;
                                self.selected_obj_info = None;
                                self.selected_tile = None;
                                if !self.open_areas.is_empty() {
                                    let next = self.open_areas[self.active_area_idx].clone();
                                    if let Some(ref module) = self.module {
                                        if let Some(area) = module.get_area(&next) {
                                            self.area_version = self.area_version.wrapping_add(1);
                                            self.viewport.set_area(area.clone(), self.area_version, target_format);
                                            self.current_area = Some(area);
                                        }
                                    }
                                    self.status = format!("Deleted '{}', switched to '{}'.", name, next);
                                } else {
                                    self.status = format!("Deleted area '{}'.", name);
                                }
                            } else {
                                self.status = format!("Deleted area '{}'.", name);
                            }
                        }
                    });
                }
            });

        // ===================== CENTRAL PANEL (3D VIEWPORT) =====================
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(ref area) = self.current_area.clone() {
                if self.show_navigator {
                    ui.horizontal(|ui| {
                        ui.heading(format!("Area: {}", area.resref));
                        if self.show_minimap {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let mini_size = egui::vec2(160.0, 120.0);
                                let (_id, rect) = ui.allocate_space(mini_size);
                                let color = egui::Color32::from_rgba_unmultiplied(20, 20, 30, 200);
                                ui.painter().rect_filled(rect, 0.0, color);
                            });
                        }
                    });
                    ui.separator();
                }
                let (_id, rect) = ui.allocate_space(ui.available_size());

                // Ghost preview: track mouse over viewport during placement mode
                if matches!(self.placement_state, PlacementState::Pending(..)) {
                    let hover_pos = ctx.input(|i| i.pointer.hover_pos());
                    self.ghost_screen_pos = hover_pos.filter(|p| rect.contains(*p));
                    if self.ghost_screen_pos.is_some() {
                        ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
                    }
                } else {
                    self.ghost_screen_pos = None;
                }

                let (is_hovered, has_w, has_s, has_a, has_d, delta, secondary_down, middle_down, primary_clicked, primary_down, click_pos, scroll) = ctx.input(|i| {
                    let hovered = i.pointer.hover_pos().map_or(false, |p| rect.contains(p));
                    let w = i.keys_down.contains(&egui::Key::W);
                    let s = i.keys_down.contains(&egui::Key::S);
                    let a = i.keys_down.contains(&egui::Key::A);
                    let d = i.keys_down.contains(&egui::Key::D);
                    (hovered, w, s, a, d, i.pointer.delta(), i.pointer.secondary_down(), i.pointer.middle_down(), i.pointer.primary_clicked(), i.pointer.primary_down(), i.pointer.interact_pos(), i.raw_scroll_delta)
                });
                let vp_w = rect.width();
                let vp_h = rect.height();

                // Handle DnD drop from blueprint palette over viewport
                if is_hovered {
                    if let Some(payload) = egui::DragAndDrop::take_payload::<String>(ctx) {
                        if let Some((typ, resref)) = payload.split_once(':') {
                            self.status = format!("Placement mode: {} {}. Click in viewport to place.", typ, resref);
                            self.placement_state = PlacementState::Pending(typ.to_string(), resref.to_string());
                        }
                    }
                }

                // Walkmesh wall toggle on click
                if primary_clicked && is_hovered && self.walkmesh_wall_mode {
                    if let Some(pos) = click_pos {
                        let vp_x = pos.x - rect.left();
                        let vp_y = pos.y - rect.top();
                        if let Some(world) = self.viewport.pick_ground(vp_x, vp_y, vp_w, vp_h) {
                            let wall_x = (world[0] / 10.0).round() as usize;
                            let _wall_z = (world[2] / 10.0).round() as usize;
                            let edge = 0u8;
                            if let Some(ref mut area) = self.current_area {
                                let area_mut = Arc::make_mut(area);
                                toggle_walkmesh_wall(area_mut, (wall_x, edge), &mut self.status);
                                self.area_version = self.area_version.wrapping_add(1);
                            }
                        }
                    }
                }

                // Terrain smoothing on click
                if primary_clicked && is_hovered && self.terrain_smooth_mode {
                    if let Some(pos) = click_pos {
                        let vp_x = pos.x - rect.left();
                        let vp_y = pos.y - rect.top();
                        if let Some(world) = self.viewport.pick_ground(vp_x, vp_y, vp_w, vp_h) {
                            let center_col = (world[0] / 10.0).floor() as i32;
                            let center_row = (-world[2] / 10.0).floor() as i32;
                            let in_bounds = self.current_area.as_ref().map_or(false, |area| {
                                center_col >= 0 && center_col < area.width && center_row >= 0 && center_row < area.height
                            });
                            if in_bounds {
                                self.push_undo();
                                if let Some(ref mut area) = self.current_area {
                                    let area_mut = Arc::make_mut(area);
                                    let w = area_mut.width;
                                    let h = area_mut.height;
                                    let radius = self.terrain_smooth_radius;

                                    let mut new_heights: Vec<i32> = area_mut.tiles.iter().map(|t| t.height).collect();

                                    for r in 0..h {
                                        for c in 0..w {
                                            if (c - center_col).abs() > radius || (r - center_row).abs() > radius {
                                                continue;
                                            }
                                            let mut sum = 0i64;
                                            let mut count = 0i64;
                                            for dr in -radius..=radius {
                                                for dc in -radius..=radius {
                                                    let nr = r + dr;
                                                    let nc = c + dc;
                                                    if nr >= 0 && nr < h && nc >= 0 && nc < w {
                                                        let idx = (nr * w + nc) as usize;
                                                        sum += area_mut.tiles[idx].height as i64;
                                                        count += 1;
                                                    }
                                                }
                                            }
                                            if count > 0 {
                                                let idx = (r * w + c) as usize;
                                                new_heights[idx] = ((sum / count) as i32).max(0).min(255);
                                            }
                                        }
                                    }

                                    for (i, h) in new_heights.iter().enumerate() {
                                        area_mut.tiles[i].height = *h;
                                    }

                                    let resref = area_mut.resref.clone();
                                    if let Some(ref mut module) = self.module {
                                        module.update_area_are(&resref, area_mut);
                                    }
                                    self.area_version = self.area_version.wrapping_add(1);
                                    self.viewport.set_area(area.clone(), self.area_version, target_format);
                                    self.status = format!("Smoothed terrain at ({},{}) radius {}", center_col, center_row, radius);
                                }
                            }
                        }
                    }
                }

                // Terrain height editing on click
                if primary_clicked && is_hovered && self.terrain_edit_mode {
                    if let Some(pos) = click_pos {
                        let vp_x = pos.x - rect.left();
                        let vp_y = pos.y - rect.top();
                        if let Some(world) = self.viewport.pick_ground(vp_x, vp_y, vp_w, vp_h) {
                            let tile_col = (world[0] / 10.0).floor() as usize;
                            let tile_row = (-world[2] / 10.0).floor() as usize;
                            let in_bounds = self.current_area.as_ref().map_or(false, |area| {
                                tile_col < area.width as usize && tile_row < area.height as usize
                            });
                            if in_bounds {
                                let shift = ctx.input(|i| i.modifiers.shift);
                                let amount = if shift { -self.terrain_edit_raise_amount } else { self.terrain_edit_raise_amount };
                                self.push_undo();
                                if let Some(ref mut area) = self.current_area {
                                    let area_mut = Arc::make_mut(area);
                                    let idx = tile_row * area_mut.width as usize + tile_col;
                                    area_mut.tiles[idx].height = (area_mut.tiles[idx].height + amount).max(0).min(255);
                                    let new_h = area_mut.tiles[idx].height;
                                    let resref = area_mut.resref.clone();
                                    if let Some(ref mut module) = self.module {
                                        module.update_area_are(&resref, area_mut);
                                    }
                                    self.area_version = self.area_version.wrapping_add(1);
                                    self.viewport.set_area(area.clone(), self.area_version, target_format);
                                    self.status = format!("Tile ({},{}) height: {}", tile_col, tile_row, new_h);
                                }
                            }
                        }
                    }
                }

                if is_hovered && self.drag_state.is_none() {
                    let mut fwd = 0.0f32;
                    let mut rgt = 0.0f32;
                    if has_w { fwd += 1.0; }
                    if has_s { fwd -= 1.0; }
                    if has_a { rgt -= 1.0; }
                    if has_d { rgt += 1.0; }
                    if fwd != 0.0 || rgt != 0.0 {
                        self.viewport.walk_camera(fwd, rgt);
                    }
                    if secondary_down {
                        self.viewport.rotate_camera(delta.x, delta.y);
                    }
                    if middle_down {
                        self.viewport.pan_camera(delta.x, delta.y);
                    }
                    if scroll.y != 0.0 {
                        self.viewport.zoom_camera(scroll.y);
                    }
                    if primary_clicked {
                        if let Some(pos) = click_pos {
                            let vp_x = pos.x - rect.left();
                            let vp_y = pos.y - rect.top();

                            // Placement mode: place object at ground hit
                            let is_placement = matches!(self.placement_state, PlacementState::Pending(..));
                            if is_placement {
                                let (typ, resref) = match &self.placement_state { PlacementState::Pending(t, r) => (t.clone(), r.clone()), _ => unreachable!() };
                                if let Some(world) = self.viewport.pick_ground(vp_x, vp_y, vp_w, vp_h) {
                                    let (nwn_x, nwn_y) = snap_to_grid_pos(world[0], -world[2], self.snap_to_grid_enabled, self.snap_to_grid_value);
                                    let nwn_z = world[1].max(0.0);
                                    let mut placed = false;
                                    self.push_undo();
                                    if let Some(area) = self.current_area.as_mut() {
                                        let area = Arc::make_mut(area);
                                        let tag = format!("{}_{}", typ, resref);
                                        match typ.as_str() {
                                            "creature" => {
                                                area.creatures.push(nwn_gamemodel::CreatureInstance {
                                                    resref: resref.clone(), tag: tag.clone(),
                                                    x: nwn_x, y: nwn_y, z: nwn_z, bearing: 0.0,
                                                    appearance: 0, model_name: None,
                                                });
                                                placed = true;
                                            }
                                            "placeable" => {
                                                area.placeables.push(nwn_gamemodel::PlaceableInstance {
                                                    resref: resref.clone(), tag: tag.clone(),
                                                    x: nwn_x, y: nwn_y, z: nwn_z, bearing: 0.0,
                                                    appearance: 0, model_name: None,
                                                });
                                                placed = true;
                                            }
                                            "door" => {
                                                area.doors.push(nwn_gamemodel::DoorInstance {
                                                    resref: resref.clone(), tag: tag.clone(),
                                                    x: nwn_x, y: nwn_y, z: nwn_z, bearing: 0.0,
                                                    appearance: 0, model_name: None,
                                                });
                                                placed = true;
                                            }
                                            "item" => {
                                                area.placeables.push(nwn_gamemodel::PlaceableInstance {
                                                    resref: resref.clone(), tag: tag.clone(),
                                                    x: nwn_x, y: nwn_y, z: nwn_z, bearing: 0.0,
                                                    appearance: 0, model_name: None,
                                                });
                                                placed = true;
                                            }
                                            _ => {}
                                        }
                                        if placed {
                                            let resref_name = area.resref.clone();
                                            if let Some(ref mut module) = self.module {
                                                module.update_area_git(&resref_name, area);
                                            }
                                            self.area_version = self.area_version.wrapping_add(1);
                                            self.status = format!("Placed {} '{}' at ({:.1}, {:.1})", typ, resref, nwn_x, nwn_y);
                                        }
                                    }
                                }
                                self.placement_state = PlacementState::Inactive;
                                self.manip_mode = ManipMode::None;
                            } else if let Some((_idx, marker)) = self.viewport.raypick(vp_x, vp_y, vp_w, vp_h) {
                                let ctrl_held = ctx.input(|i| i.modifiers.ctrl);
                                if let Some(ref area) = self.current_area {
                                    let obj_type_str = match marker.obj_type {
                                        ObjectType::Creature => "creature",
                                        ObjectType::Door => "door",
                                        ObjectType::Placeable => "placeable",
                                        ObjectType::Trigger => "trigger",
                                        ObjectType::Waypoint => "waypoint",
                                        ObjectType::Encounter => "encounter",
                                        ObjectType::SoundEmitter => "sound_emitter",
                                    };
                                    let key = (obj_type_str.to_string(), marker.index);

                                    if ctrl_held && self.selection_list.iter().any(|x| *x == key) {
                                        // Ctrl+Click on an already-selected object: deselect it
                                        self.selection_list.retain(|x| *x != key);
                                        if let Some(last) = self.selection_list.last().cloned() {
                                            select_single_object(area, self.module.as_ref(), &mut self.selected_object, &mut self.selected_obj_info, &last.0, last.1);
                                        } else {
                                            self.selected_object = None;
                                            self.selected_obj_info = None;
                                        }
                                        self.status = format!("Deselected. {} object(s) remain.", self.selection_list.len());
                                    } else {
                                        if ctrl_held {
                                            self.selection_list.push(key.clone());
                                        } else {
                                            self.selection_list.clear();
                                        }
                                        let bag = match marker.obj_type {
                                            ObjectType::Creature => {
                                                area.creatures.get(marker.index).map(|c| (format!("Creature: {}", c.resref), c.to_property_bag()))
                                            }
                                            ObjectType::Door => {
                                                area.doors.get(marker.index).map(|d| (format!("Door: {}", d.resref), d.to_property_bag()))
                                            }
                                            ObjectType::Placeable => {
                                                area.placeables.get(marker.index).map(|p| (format!("Placeable: {}", p.resref), p.to_property_bag()))
                                            }
                                            ObjectType::Trigger => {
                                                area.triggers.get(marker.index).map(|t| (format!("Trigger: {}", t.tag), t.to_property_bag()))
                                            }
                                            ObjectType::Waypoint => {
                                                area.waypoints.get(marker.index).map(|w| (format!("Waypoint: {}", w.tag), w.to_property_bag()))
                                            }
                                            ObjectType::Encounter => {
                                                area.encounters.get(marker.index).map(|e| (format!("Encounter: {}", e.name), e.to_property_bag()))
                                            }
                                            ObjectType::SoundEmitter => {
                                                area.sound_emitters.get(marker.index).map(|s| (format!("Sound: {}", s.tag), s.to_property_bag()))
                                            }
                                        };
                                        if let Some((title, bag)) = bag {
                                            let full_bag = self.module.as_ref().and_then(|module| {
                                                let area_key = area.resref.trim().to_lowercase();
                                                let git_bytes = module.raw_git.get(&area_key)?;
                                                let gff = nwn_gff::GffFile::from_bytes(git_bytes).ok()?;
                                                let list_name = git_list_name(obj_type_str);
                                                let s = gff.root().find(list_name)?.unwrap_list().get(marker.index)?;
                                                Some(PropertyBag::from_gff_struct(s))
                                            });
                                            self.selected_object = Some(SelectedObject { title, bag: full_bag.unwrap_or(bag) });
                                            self.selected_obj_info = Some(key);

                                            // Start 3D drag-to-move
                                            if !ctrl_held && !self.walkmesh_wall_mode && !self.terrain_smooth_mode && !self.terrain_edit_mode {
                                                if let Some(world) = self.viewport.pick_ground(vp_x, vp_y, vp_w, vp_h) {
                                                    let orig = match marker.obj_type {
                                                        ObjectType::Creature => area.creatures.get(marker.index).map(|o| (o.x, o.y, o.z, o.bearing)),
                                                        ObjectType::Door => area.doors.get(marker.index).map(|o| (o.x, o.y, o.z, o.bearing)),
                                                        ObjectType::Placeable => area.placeables.get(marker.index).map(|o| (o.x, o.y, o.z, o.bearing)),
                                                        ObjectType::Trigger => area.triggers.get(marker.index).map(|o| (o.x, o.y, o.z, o.bearing)),
                                                        ObjectType::Waypoint => area.waypoints.get(marker.index).map(|o| (o.x, o.y, o.z, o.bearing)),
                                                        ObjectType::Encounter => area.encounters.get(marker.index).map(|o| (o.x, o.y, o.z, o.bearing)),
                                                        ObjectType::SoundEmitter => area.sound_emitters.get(marker.index).map(|o| (o.x, o.y, o.z, o.bearing)),
                                                    };
                                                    if let Some((ox, oy, oz, ob)) = orig {
                                                        self.drag_state = Some(DragState {
                                                            obj_type: obj_type_str.to_string(),
                                                            obj_idx: marker.index,
                                                            orig_x: ox, orig_y: oy, orig_z: oz, orig_bearing: ob,
                                                            start_render_x: world[0],
                                                            start_render_z: world[2],
                                                        });
                                                    }
                                                }
                                            }

                                            let total = self.selection_list.len();
                                            if total > 1 {
                                                self.status = format!("Selected {} #{} (+{} more)", obj_type_str, marker.index, total - 1);
                                            } else {
                                                self.status = format!("Selected {} #{}", obj_type_str, marker.index);
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Tile selection via ground pick
                                if let Some(world) = self.viewport.pick_ground(vp_x, vp_y, vp_w, vp_h) {
                                    if let Some(ref area) = self.current_area {
                                        let tile_col = (world[0] / 10.0).floor() as usize;
                                        let tile_row = (-world[2] / 10.0).floor() as usize;
                                        if tile_col < area.width as usize && tile_row < area.height as usize {
                                            self.selected_tile = Some((tile_col, tile_row));
                                            self.status = format!("Selected tile ({}, {})", tile_col, tile_row);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ── 3D drag-to-move (live update while dragging, commit on release) ──
                let drag_clone = self.drag_state.clone();
                if let Some(ref drag) = drag_clone {
                    if primary_down {
                        if is_hovered {
                            let cur_hover = ctx.input(|i| i.pointer.hover_pos());
                            if let Some(pos) = cur_hover.filter(|p| rect.contains(*p)) {
                                let vp_cx = pos.x - rect.left();
                                let vp_cy = pos.y - rect.top();
                                if let Some(world) = self.viewport.pick_ground(vp_cx, vp_cy, vp_w, vp_h) {
                                    let dx = world[0] - drag.start_render_x;
                                    let dz = world[2] - drag.start_render_z;
                                    let new_x = drag.orig_x + dx;
                                    let new_y = drag.orig_y - dz;
                                    if let Some(area) = self.current_area.as_mut() {
                                        let area = Arc::make_mut(area);
                                        match drag.obj_type.as_str() {
                                            "creature" => { if let Some(c) = area.creatures.get_mut(drag.obj_idx) { c.x = new_x; c.y = new_y; } }
                                            "door" => { if let Some(d) = area.doors.get_mut(drag.obj_idx) { d.x = new_x; d.y = new_y; } }
                                            "placeable" => { if let Some(p) = area.placeables.get_mut(drag.obj_idx) { p.x = new_x; p.y = new_y; } }
                                            "trigger" => { if let Some(t) = area.triggers.get_mut(drag.obj_idx) { t.x = new_x; t.y = new_y; } }
                                            "waypoint" => { if let Some(w) = area.waypoints.get_mut(drag.obj_idx) { w.x = new_x; w.y = new_y; } }
                                            "encounter" => { if let Some(e) = area.encounters.get_mut(drag.obj_idx) { e.x = new_x; e.y = new_y; } }
                                            "sound_emitter" => { if let Some(s) = area.sound_emitters.get_mut(drag.obj_idx) { s.x = new_x; s.y = new_y; } }
                                            _ => {}
                                        }
                                        let area_clone = Arc::new(area.clone());
                                        self.area_version = self.area_version.wrapping_add(1);
                                        self.viewport.set_area(area_clone, self.area_version, target_format);
                                    }
                                }
                            }
                        }
                    } else {
                        // Primary released — commit drag
                        self.drag_state = None;
                        let area_exists = self.current_area.is_some();
                        if area_exists {
                            self.push_undo();
                        }
                        if let Some(area) = self.current_area.as_mut() {
                            let area = Arc::make_mut(area);
                            let resref = area.resref.clone();
                            if let Some(ref mut module) = self.module {
                                module.update_area_git(&resref, area);
                            }
                            self.area_version = self.area_version.wrapping_add(1);
                            self.viewport.set_area(Arc::new(area.clone()), self.area_version, target_format);
                            self.status = format!("Moved {} #{}", drag.obj_type, drag.obj_idx);
                        }
                    }
                }

                // Keyboard manipulation of selected object(s)
                if is_hovered && self.selected_obj_info.is_some() && !matches!(self.placement_state, PlacementState::Pending(..)) {
                    let mut nudge_targets: Vec<(String, usize)> = self.selection_list.clone();
                    if nudge_targets.is_empty() {
                        nudge_targets = vec![self.selected_obj_info.clone().unwrap()];
                    }
                    let mut needs_update = false;
                    let mut nudge_x = 0.0f32;
                    let mut nudge_y = 0.0f32;
                    let mut rotate_delta = 0.0f32;
                    let mut do_delete = false;

                    let keys = ctx.input(|i| {
                        let left = i.keys_down.contains(&egui::Key::ArrowLeft);
                        let right = i.keys_down.contains(&egui::Key::ArrowRight);
                        let up = i.keys_down.contains(&egui::Key::ArrowUp);
                        let down = i.keys_down.contains(&egui::Key::ArrowDown);
                        let q = i.keys_down.contains(&egui::Key::Q);
                        let e = i.keys_down.contains(&egui::Key::E);
                        let delete = i.key_pressed(egui::Key::Delete);
                        (left, right, up, down, q, e, delete)
                    });

                    if keys.0 { nudge_x -= 1.0; }
                    if keys.1 { nudge_x += 1.0; }
                    if keys.2 { nudge_y += 1.0; }
                    if keys.3 { nudge_y -= 1.0; }
                    if keys.4 { rotate_delta -= 0.1; }
                    if keys.5 { rotate_delta += 0.1; }
                    if keys.6 { do_delete = true; }

                    if nudge_x != 0.0 || nudge_y != 0.0 || rotate_delta != 0.0 || do_delete {
                        self.push_undo();
                        if let Some(area) = self.current_area.as_mut() {
                            let area = Arc::make_mut(area);
                            let speed = self.manip_nudge_speed;
                            if do_delete {
                                // Delete in reverse index order to preserve indices
                                let mut to_delete: Vec<(String, usize)> = nudge_targets.clone();
                                to_delete.sort_by(|a, b| b.1.cmp(&a.1));
                                for (ref t, i) in to_delete {
                                    match t.as_str() {
                                        "creature" => { if i < area.creatures.len() { area.creatures.remove(i); } }
                                        "door" => { if i < area.doors.len() { area.doors.remove(i); } }
                                        "placeable" => { if i < area.placeables.len() { area.placeables.remove(i); } }
                                        "trigger" => { if i < area.triggers.len() { area.triggers.remove(i); } }
                                        "waypoint" => { if i < area.waypoints.len() { area.waypoints.remove(i); } }
                                        "encounter" => { if i < area.encounters.len() { area.encounters.remove(i); } }
                                        "sound_emitter" => { if i < area.sound_emitters.len() { area.sound_emitters.remove(i); } }
                                        _ => {}
                                    }
                                }
                                self.selected_object = None;
                                self.selected_obj_info = None;
                                self.selection_list.clear();
                                self.status = format!("Deleted {} object(s)", nudge_targets.len());
                                needs_update = true;
                            } else {
                                for (t, i) in &nudge_targets {
                                    let do_snap = self.snap_to_grid_enabled;
                                    let snap_val = self.snap_to_grid_value;
                                    match t.as_str() {
                                        "creature" => {
                                            if let Some(c) = area.creatures.get_mut(*i) {
                                                c.x += nudge_x * speed; c.y += nudge_y * speed;
                                                if do_snap { let (sx, sy) = snap_to_grid_pos(c.x, c.y, true, snap_val); c.x = sx; c.y = sy; }
                                                c.bearing += rotate_delta; needs_update = true;
                                            }
                                        }
                                        "door" => {
                                            if let Some(d) = area.doors.get_mut(*i) {
                                                d.x += nudge_x * speed; d.y += nudge_y * speed;
                                                if do_snap { let (sx, sy) = snap_to_grid_pos(d.x, d.y, true, snap_val); d.x = sx; d.y = sy; }
                                                d.bearing += rotate_delta; needs_update = true;
                                            }
                                        }
                                        "placeable" => {
                                            if let Some(p) = area.placeables.get_mut(*i) {
                                                p.x += nudge_x * speed; p.y += nudge_y * speed;
                                                if do_snap { let (sx, sy) = snap_to_grid_pos(p.x, p.y, true, snap_val); p.x = sx; p.y = sy; }
                                                p.bearing += rotate_delta; needs_update = true;
                                            }
                                        }
                                        "trigger" => {
                                            if let Some(t) = area.triggers.get_mut(*i) {
                                                t.x += nudge_x * speed; t.y += nudge_y * speed;
                                                if do_snap { let (sx, sy) = snap_to_grid_pos(t.x, t.y, true, snap_val); t.x = sx; t.y = sy; }
                                                t.bearing += rotate_delta; needs_update = true;
                                            }
                                        }
                                        "waypoint" => {
                                            if let Some(w) = area.waypoints.get_mut(*i) {
                                                w.x += nudge_x * speed; w.y += nudge_y * speed;
                                                if do_snap { let (sx, sy) = snap_to_grid_pos(w.x, w.y, true, snap_val); w.x = sx; w.y = sy; }
                                                w.bearing += rotate_delta; needs_update = true;
                                            }
                                        }
                                        "encounter" => {
                                            if let Some(e) = area.encounters.get_mut(*i) {
                                                e.x += nudge_x * speed; e.y += nudge_y * speed;
                                                if do_snap { let (sx, sy) = snap_to_grid_pos(e.x, e.y, true, snap_val); e.x = sx; e.y = sy; }
                                                e.bearing += rotate_delta; needs_update = true;
                                            }
                                        }
                                        "sound_emitter" => {
                                            if let Some(s) = area.sound_emitters.get_mut(*i) {
                                                s.x += nudge_x * speed; s.y += nudge_y * speed;
                                                if self.snap_to_grid_enabled {
                                                    let (sx, sy) = snap_to_grid_pos(s.x, s.y, true, self.snap_to_grid_value);
                                                    s.x = sx; s.y = sy;
                                                }
                                                s.bearing += rotate_delta;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            if needs_update {
                                let resref = area.resref.clone();
                                if let Some(ref mut module) = self.module {
                                    module.update_area_git(&resref, area);
                                }
                                self.area_version = self.area_version.wrapping_add(1);
                            }
                        }
                    }
                }

                let cb = egui_wgpu::Callback::new_paint_callback(rect, self.viewport.clone_arc());
                ui.painter().add(cb);

                // Ghost placement preview overlay
                if let Some(ghost_pos) = self.ghost_screen_pos {
                    let painter = ui.painter();
                    painter.line_segment(
                        [ghost_pos - egui::vec2(10.0, 0.0), ghost_pos + egui::vec2(10.0, 0.0)],
                        egui::Stroke::new(2.0, egui::Color32::YELLOW),
                    );
                    painter.line_segment(
                        [ghost_pos - egui::vec2(0.0, 10.0), ghost_pos + egui::vec2(0.0, 10.0)],
                        egui::Stroke::new(2.0, egui::Color32::YELLOW),
                    );
                    if let PlacementState::Pending(ref typ, ref resref) = self.placement_state {
                        painter.text(
                            ghost_pos + egui::vec2(15.0, -15.0),
                            egui::Align2::LEFT_TOP,
                            format!("Place {}: {}", typ, resref),
                            egui::FontId::proportional(14.0),
                            egui::Color32::YELLOW,
                        );
                    }
                }
            } else if self.module.is_some() {
                ui.heading("Select an area from the navigator");
                ui.separator();
                ui.label("Click an area name in the left panel to view it.");
            } else {
                ui.heading("The Aurora BorealiuS Toolset");
                ui.separator();
                ui.label(&self.status);
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label("Open a Neverwinter Nights module to get started.");
                    ui.label("The toolset supports:");
                    ui.label("  - Module loading and area browsing");
                    ui.label("  - 3D area map viewport");
                    ui.label("  - Property editing");
                    ui.label("  - NWScript syntax highlighting and editing");
                    ui.label("  - Conversation tree editing");
                    ui.label("  - ERF/MOD/HAK packaging");
                });
            }
        });

        // Cancel placement mode on Escape
        if matches!(self.placement_state, PlacementState::Pending(..)) {
            let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
            if esc {
                self.placement_state = PlacementState::Inactive;
                self.status = "Placement cancelled.".to_string();
            }
        }

        // Global keyboard shortcuts
        let ctrl_c = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::C));
        let ctrl_v = ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::V));
        let ctrl_z = ctx.input(|i| i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::Z));
        let ctrl_shift_z = ctx.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::Z));
        if ctrl_c {
            self.copy_selected_object();
        }
        if ctrl_v {
            self.paste_object();
        }
        if ctrl_z {
            self.undo(target_format);
        }
        if ctrl_shift_z {
            self.redo(target_format);
        }

        // ===================== STATUS BAR =====================
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(ref module) = self.module {
                        ui.label(format!("{} areas", module.num_areas()));
                        ui.separator();
                        ui.label(&module.name);
                    }
                });
            });
        });

        // ===================== POST-RENDER HANDLERS =====================
        // Handle module properties save
        if module_props_changed {
            if let Some(ref mut module) = self.module {
                module.name = self.module_name_edit.clone();
                module.tag = self.module_tag_edit.clone();
                module.entry_area = self.module_entry_edit.clone();
                module.description = self.module_description_edit.clone();
                module.modified = true;
                self.status = "Module properties updated.".to_string();
            }
        }

        // Handle resource viewer callbacks
        if let Some((name, table)) = save_2da {
            if let Some(ref mut module) = self.module {
                let data = table.to_2da_string();
                module.update_2da(&name, &data);
                self.status = format!("2DA '{}' updated via resource viewer.", name);
            }
        }
        if let Some(name) = open_script {
            if let Some(ref module) = self.module {
                if let Some(source) = module.get_script_source(&name) {
                    self.script_editor_scripts.push((name.clone(), source.to_string()));
                    self.status = format!("Opened script '{}'.", name);
                }
            }
        }
        if let Some(name) = open_conversation {
            if let Some(ref module) = self.module {
                if let Some(convo) = module.get_conversation(&name) {
                    self.conversation_editors.push((name.clone(), ConversationEditor::new(name.clone(), convo)));
                    self.status = format!("Opened conversation '{}'.", name);
                }
            }
        }

        // ===================== CONTENTS PANEL =====================
        if self.show_contents_panel {
            egui::SidePanel::left("contents_panel")
                .resizable(true)
                .default_width(250.0)
                .show(ctx, |ui| {
                    ui.heading("Contents");
                    ui.separator();
                    if let Some(ref area) = self.current_area {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            fn list_category(ui: &mut egui::Ui, label: &str, items: &[impl AsRef<str>], selected: Option<usize>, on_select: &mut impl FnMut(usize)) {
                                egui::CollapsingHeader::new(format!("{} ({})", label, items.len()))
                                    .default_open(!items.is_empty())
                                    .show(ui, |ui| {
                                        for (i, item) in items.iter().enumerate() {
                                            let is_selected = selected == Some(i);
                                            let resp = ui.selectable_label(is_selected, item.as_ref());
                                            if resp.clicked() {
                                                on_select(i);
                                            }
                                        }
                                    });
                            }
                            // Cache lists to avoid borrowing issues
                            let creature_labels: Vec<String> = area.creatures.iter().map(|c| format!("{} ({})", c.resref, c.tag)).collect();
                            let door_labels: Vec<String> = area.doors.iter().map(|d| format!("{} ({})", d.resref, d.tag)).collect();
                            let placeable_labels: Vec<String> = area.placeables.iter().map(|p| format!("{} ({})", p.resref, p.tag)).collect();
                            let trigger_labels: Vec<String> = area.triggers.iter().map(|t| format!("Trigger ({})", t.tag)).collect();
                            let waypoint_labels: Vec<String> = area.waypoints.iter().map(|w| format!("Waypoint ({})", w.tag)).collect();
                            let encounter_labels: Vec<String> = area.encounters.iter().map(|e| format!("{} ({})", e.name, e.tag)).collect();
                            let sound_emitter_labels: Vec<String> = area.sound_emitters.iter().map(|s| format!("Sound: {}", s.tag)).collect();
                            let sel_info = self.selected_obj_info.clone();
                            let mut new_sel: Option<(String, usize)> = None;
                            list_category(ui, "Creatures", &creature_labels, if sel_info.as_ref().map_or(false, |(t, _)| t == "creature") { sel_info.as_ref().map(|(_, i)| *i) } else { None }, &mut |i| new_sel = Some(("creature".to_string(), i)));
                            list_category(ui, "Doors", &door_labels, if sel_info.as_ref().map_or(false, |(t, _)| t == "door") { sel_info.as_ref().map(|(_, i)| *i) } else { None }, &mut |i| new_sel = Some(("door".to_string(), i)));
                            list_category(ui, "Placeables", &placeable_labels, if sel_info.as_ref().map_or(false, |(t, _)| t == "placeable") { sel_info.as_ref().map(|(_, i)| *i) } else { None }, &mut |i| new_sel = Some(("placeable".to_string(), i)));
                            list_category(ui, "Triggers", &trigger_labels, if sel_info.as_ref().map_or(false, |(t, _)| t == "trigger") { sel_info.as_ref().map(|(_, i)| *i) } else { None }, &mut |i| new_sel = Some(("trigger".to_string(), i)));
                            list_category(ui, "Waypoints", &waypoint_labels, if sel_info.as_ref().map_or(false, |(t, _)| t == "waypoint") { sel_info.as_ref().map(|(_, i)| *i) } else { None }, &mut |i| new_sel = Some(("waypoint".to_string(), i)));
                            list_category(ui, "Encounters", &encounter_labels, if sel_info.as_ref().map_or(false, |(t, _)| t == "encounter") { sel_info.as_ref().map(|(_, i)| *i) } else { None }, &mut |i| new_sel = Some(("encounter".to_string(), i)));
                            list_category(ui, "Sound Emitters", &sound_emitter_labels, if sel_info.as_ref().map_or(false, |(t, _)| t == "sound_emitter") { sel_info.as_ref().map(|(_, i)| *i) } else { None }, &mut |i| new_sel = Some(("sound_emitter".to_string(), i)));
                            if let Some((obj_type, obj_idx)) = new_sel {
                                if let Some(ref area) = self.current_area {
                                    let bag = match obj_type.as_str() {
                                        "creature" => area.creatures.get(obj_idx).map(|c| c.to_property_bag()),
                                        "door" => area.doors.get(obj_idx).map(|d| d.to_property_bag()),
                                        "placeable" => area.placeables.get(obj_idx).map(|p| p.to_property_bag()),
                                        "trigger" => area.triggers.get(obj_idx).map(|t| t.to_property_bag()),
                                        "waypoint" => area.waypoints.get(obj_idx).map(|w| w.to_property_bag()),
                                        "encounter" => area.encounters.get(obj_idx).map(|e| e.to_property_bag()),
                                        "sound_emitter" => area.sound_emitters.get(obj_idx).map(|s| s.to_property_bag()),
                                        _ => None,
                                    };
                                    if let Some(bag) = bag {
                                        self.selected_object = Some(SelectedObject { title: format!("{} #{}", obj_type, obj_idx), bag });
                                        self.selected_obj_info = Some((obj_type, obj_idx));
                                    }
                                }
                            }
                        });
                    } else {
                        ui.label("No area loaded.");
                    }
                });
        }

        // ===================== NEW AREA DIALOG =====================
        if self.new_area_open {
            let mut new_area_open = true;
            egui::Window::new("New Area")
                .default_size([350.0, 250.0])
                .open(&mut new_area_open)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Resref:");
                        ui.text_edit_singleline(&mut self.new_area_resref);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Tileset:");
                        ui.text_edit_singleline(&mut self.new_area_tileset);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Width:");
                        ui.add(egui::DragValue::new(&mut self.new_area_width).range(4..=64));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Height:");
                        ui.add(egui::DragValue::new(&mut self.new_area_height).range(4..=64));
                    });
                    ui.separator();
                    if ui.button("Create Area").clicked() {
                        if !self.new_area_resref.is_empty() && !self.new_area_tileset.is_empty() {
                            if let Some(ref mut module) = self.module {
                                let resref = module.new_empty_area(&self.new_area_resref, &self.new_area_tileset, self.new_area_width, self.new_area_height);
                                self.new_area_resref.clear();
                                self.new_area_tileset.clear();
                                // Load area into viewport and add to open tabs
                                if let Some(area) = module.get_area(&resref) {
                                    self.area_version = self.area_version.wrapping_add(1);
                                    self.viewport.set_area(area.clone(), self.area_version, target_format);
                                    self.current_area = Some(area);
                                    self.selected_object = None;
                                    self.selected_obj_info = None;
                                }
                                if !self.open_areas.contains(&resref) {
                                    self.open_areas.push(resref.clone());
                                }
                                self.active_area_idx = self.open_areas.len() - 1;
                                self.status = format!("Created area '{}'.", resref);
                            }
                        } else {
                            self.status = "Enter a resref and tileset name.".to_string();
                        }
                    }
                });
            if !new_area_open {
                self.new_area_open = false;
            }
        }

        // ===================== TILE PALETTE =====================
        if self.tile_palette_open {
            let tileset_name = self.current_area.as_ref().map(|a| a.tileset.clone()).unwrap_or_default();
            // Pre-load tileset data outside closure to avoid borrow conflicts
            let tiles_from_2da: Vec<(i32, String)> = if let Some(ref module) = self.module {
                module.resource_manager.get_2da(&format!("tileset_apply_{}", tileset_name.to_lowercase()))
                    .map(|table| {
                        let rows = table.row_count().min(500);
                        (0..rows).filter_map(|r| {
                            let id = table.get_cell_by_label(r, "TileID").and_then(|s| s.parse::<i32>().ok()).unwrap_or(-1);
                            let name = table.get_cell_by_label(r, "ResRef").unwrap_or("").to_string();
                            if id < 0 || name.is_empty() { return None; }
                            Some((id, name))
                        }).collect::<Vec<_>>()
                    }).unwrap_or_default()
            } else { Vec::new() };
            let mut palette_open = true;
            egui::Window::new("Tile Palette")
                .default_size([300.0, 500.0])
                .open(&mut palette_open)
                .show(ctx, |ui| {
                    ui.label(format!("Tileset: {}", tileset_name));
                    ui.horizontal(|ui| {
                        ui.label("Filter:");
                        ui.text_edit_singleline(&mut self.tile_palette_filter);
                    });
                    ui.separator();
                    if tiles_from_2da.is_empty() {
                        ui.label(format!("No tileset data found for '{}'", tileset_name));
                    } else {
                        let mut filtered: Vec<(i32, String)> = tiles_from_2da.iter()
                            .filter(|(_, name)| {
                                self.tile_palette_filter.is_empty()
                                    || name.to_lowercase().contains(&self.tile_palette_filter.to_lowercase())
                            })
                            .cloned().collect();
                        filtered.sort_by_key(|(id, _)| *id);
                        ui.horizontal(|ui| {
                            ui.label("Auto:");
                            let fill_id = filtered.first().map(|(id, _)| *id).unwrap_or(0);
                            if ui.button("Fill All").clicked() {
                                self.fill_area_tiles(fill_id);
                            }
                            if ui.button("Orient").clicked() {
                                self.auto_orient_tiles();
                            }
                        });
                        ui.separator();
                        egui::Grid::new("tile_grid").striped(true).show(ui, |ui| {
                            for (i, (tile_id, tile_name)) in filtered.iter().enumerate() {
                                if i % 4 == 0 && i > 0 { ui.end_row(); }
                                let shift = ui.input(|i| i.modifiers.shift);
                                if ui.button(format!("{}:{}", tile_id, tile_name)).clicked() {
                                    let paint_id = *tile_id;
                                    if shift {
                                        if let Some((tile_col, tile_row)) = self.selected_tile {
                                            self.tile_flood_fill(tile_col, tile_row, paint_id);
                                        }
                                    } else if let Some((tile_col, tile_row)) = self.selected_tile {
                                        self.push_undo();
                                        if let Some(ref mut area) = self.current_area {
                                            let idx = tile_row * area.width as usize + tile_col;
                                            let area_mut = Arc::make_mut(area);
                                            if idx < area_mut.tiles.len() {
                                                area_mut.tiles[idx].id = paint_id;
                                                self.status = format!("Painted tile ({},{}) with id {}", tile_col, tile_row, paint_id);
                                                let resref = area_mut.resref.clone();
                                                if let Some(ref mut module) = self.module {
                                                    module.update_area_are(&resref, area_mut);
                                                }
                                                self.area_version = self.area_version.wrapping_add(1);
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    }
                });
            if !palette_open {
                self.tile_palette_open = false;
            }
        }

        // ===================== NEW OBJECT DIALOG =====================
        if self.new_object_open {
            let mut still_open = true;
            let mut created = false;
            egui::Window::new("New Object")
                .default_size([350.0, 300.0])
                .open(&mut still_open)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Type:");
                        egui::ComboBox::from_id_salt("new_obj_type")
                            .selected_text(&self.new_object_type)
                            .show_ui(ui, |ui| {
                                for t in &["creature", "door", "placeable", "trigger", "waypoint", "encounter", "sound_emitter"] {
                                    ui.selectable_value(&mut self.new_object_type, t.to_string(), *t);
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("ResRef:");
                        ui.text_edit_singleline(&mut self.new_object_resref);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Tag:");
                        ui.text_edit_singleline(&mut self.new_object_tag);
                    });
                    ui.separator();
                    ui.label("Position:");
                    ui.horizontal(|ui| {
                        ui.label("X:");
                        ui.add(egui::DragValue::new(&mut self.new_object_x).speed(0.5));
                        ui.label("Y:");
                        ui.add(egui::DragValue::new(&mut self.new_object_y).speed(0.5));
                        ui.label("Z:");
                        ui.add(egui::DragValue::new(&mut self.new_object_z).speed(0.5));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Bearing:");
                        ui.add(egui::DragValue::new(&mut self.new_object_bearing).speed(0.1));
                    });
                    ui.separator();
                    if ui.button("Create Object").clicked() {
                        let resref = self.new_object_resref.clone();
                        let typ = self.new_object_type.clone();
                        if resref.is_empty() {
                            self.status = "Enter a ResRef for the new object.".to_string();
                        } else {
                            let tag = if self.new_object_tag.is_empty() {
                                format!("{}_{}", typ, resref)
                            } else {
                                self.new_object_tag.clone()
                            };
                            self.push_undo();
                            let (sx, sy) = snap_to_grid_pos(self.new_object_x, self.new_object_y, self.snap_to_grid_enabled, self.snap_to_grid_value);
                            self.new_object_x = sx;
                            self.new_object_y = sy;
                            if let Some(area) = self.current_area.as_mut() {
                                let area = Arc::make_mut(area);
                                match typ.as_str() {
                                    "creature" => {
                                        area.creatures.push(nwn_gamemodel::CreatureInstance {
                                            resref: resref.clone(), tag,
                                            x: self.new_object_x, y: self.new_object_y,
                                            z: self.new_object_z, bearing: self.new_object_bearing,
                                            appearance: 0, model_name: None,
                                        });
                                    }
                                    "door" => {
                                        area.doors.push(nwn_gamemodel::DoorInstance {
                                            resref: resref.clone(), tag,
                                            x: self.new_object_x, y: self.new_object_y,
                                            z: self.new_object_z, bearing: self.new_object_bearing,
                                            appearance: 0, model_name: None,
                                        });
                                    }
                                    "placeable" => {
                                        area.placeables.push(nwn_gamemodel::PlaceableInstance {
                                            resref: resref.clone(), tag,
                                            x: self.new_object_x, y: self.new_object_y,
                                            z: self.new_object_z, bearing: self.new_object_bearing,
                                            appearance: 0, model_name: None,
                                        });
                                    }
                                    "trigger" => {
                                        area.triggers.push(nwn_gamemodel::TriggerInstance {
                                            resref: resref.clone(), tag,
                                            x: self.new_object_x, y: self.new_object_y,
                                            z: self.new_object_z, bearing: self.new_object_bearing,
                                            geometry: Vec::new(),
                                        });
                                    }
                                    "waypoint" => {
                                        area.waypoints.push(nwn_gamemodel::WayPointInstance {
                                            resref: resref.clone(), tag,
                                            x: self.new_object_x, y: self.new_object_y,
                                            z: self.new_object_z, bearing: self.new_object_bearing,
                                        });
                                    }
                                    "encounter" => {
                                        area.encounters.push(nwn_gamemodel::EncounterInstance {
                                            tag: tag.clone(), name: resref.clone(),
                                            x: self.new_object_x, y: self.new_object_y,
                                            z: self.new_object_z, bearing: self.new_object_bearing,
                                            geometry: Vec::new(), difficulty: 1,
                                            player_only: false, reset: 1,
                                            max_creatures: 6, spawn_option: 0,
                                            on_entered: String::new(), on_exhausted: String::new(),
                                            on_exit: String::new(), on_heartbeat: String::new(),
                                            on_user_defined: String::new(),
                                        });
                                    }
                                    "sound_emitter" => {
                                        area.sound_emitters.push(nwn_gamemodel::SoundEmitterInstance {
                                            tag: tag.clone(),
                                            template_resref: resref.clone(),
                                            x: self.new_object_x, y: self.new_object_y,
                                            z: self.new_object_z, bearing: self.new_object_bearing,
                                            positional: true,
                                            min_distance: 10.0,
                                            max_distance: 40.0,
                                            volume: 100,
                                            sounds: Vec::new(),
                                        });
                                    }
                                    _ => {}
                                }
                                let resref_name = area.resref.clone();
                                if let Some(ref mut module) = self.module {
                                    module.update_area_git(&resref_name, area);
                                }
                                self.area_version = self.area_version.wrapping_add(1);
                                self.status = format!("Created {} '{}'", typ, resref);
                            }
                            created = true;
                        }
                    }
                });
            if !still_open || created {
                self.new_object_open = false;
            }
        }

        // ===================== EDITOR WINDOWS =====================
        // Faction Editor
        if self.faction_editor_open {
            let mut faction_list: Option<FactionList> = Some(self.faction_list.clone());
            egui::Window::new("Faction Editor")
                .default_size([500.0, 400.0])
                .open(&mut self.faction_editor_open)
                .show(ctx, |ui| {
                    let erf = self.module.as_ref().and_then(|m| m.erf.as_ref());
                    crate::faction_editor::show_faction_editor(ui, erf, &mut faction_list);
                });
            if let Some(fl) = faction_list {
                self.faction_list = fl;
            }
        }
        // Journal Editor
        if self.journal_editor_open {
            let mut journal = self.module.as_ref().and_then(|m| {
                let data = m.erf.as_ref()?.get("journal", nwn_gamemodel::resource_types::JRL)?;
                let gff = nwn_gff::GffFile::from_bytes(&data).ok()?;
                nwn_gamemodel::Journal::from_gff(&gff).ok()
            });
            egui::Window::new("Journal Editor")
                .default_size([500.0, 400.0])
                .open(&mut self.journal_editor_open)
                .show(ctx, |ui| {
                    crate::journal_editor::show_journal_editor(ui, &mut journal);
                });
        }
        // Encounter Editor
        if self.encounter_editor_open {
            let mut do_save = false;
            egui::Window::new("Encounter Editor")
                .default_size([600.0, 450.0])
                .open(&mut self.encounter_editor_open)
                .show(ctx, |ui| {
                    if let Some(ref mut area) = self.current_area {
                        crate::encounter_editor::show_encounter_editor(ui, &mut Arc::make_mut(area).encounters, &mut do_save);
                    } else {
                        ui.label("No area loaded.");
                    }
                });
            if do_save {
                if let Some(ref area) = self.current_area {
                    let resref = area.resref.clone();
                    if let Some(ref mut module) = self.module {
                        module.update_area_git(&resref, area);
                    }
                    self.area_version = self.area_version.wrapping_add(1);
                }
            }
        }

        // ===================== PROPERTY INSPECTOR (RIGHT PANEL) =====================
        let mut inspector_modified = false;
        let mut delete_requested = false;
        let mut pos_update: Option<(String, usize, f32, f32, f32, f32)> = None;
        egui::SidePanel::right("inspector")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                        ui.heading("Properties");
                ui.separator();
                if let Some(ref mut obj) = self.selected_object {
                    // Position & bearing controls
                    if let Some((ref obj_type, obj_idx)) = self.selected_obj_info {
                        let obj_type = obj_type.clone();
                        let obj_idx = obj_idx;
                        let sel_count = self.selection_list.len();
                        if sel_count > 1 {
                            ui.label(format!("Type: {}  Index: {}  ({} selected)", obj_type, obj_idx, sel_count));
                        } else {
                            ui.label(format!("Type: {}  Index: {}", obj_type, obj_idx));
                        }
                        ui.separator();
                        let pos = self.current_area.as_ref().and_then(|area| {
                            match obj_type.as_str() {
                                "creature" => area.creatures.get(obj_idx).map(|c| (c.x, c.y, c.z, c.bearing)),
                                "door" => area.doors.get(obj_idx).map(|d| (d.x, d.y, d.z, d.bearing)),
                                "placeable" => area.placeables.get(obj_idx).map(|p| (p.x, p.y, p.z, p.bearing)),
                                "trigger" => area.triggers.get(obj_idx).map(|t| (t.x, t.y, t.z, t.bearing)),
                                "waypoint" => area.waypoints.get(obj_idx).map(|w| (w.x, w.y, w.z, w.bearing)),
                                "encounter" => area.encounters.get(obj_idx).map(|e| (e.x, e.y, e.z, e.bearing)),
                                "sound_emitter" => area.sound_emitters.get(obj_idx).map(|s| (s.x, s.y, s.z, s.bearing)),
                                _ => None,
                            }
                        });
                        if let Some((mut px, mut py, mut pz, mut pb)) = pos {
                            ui.horizontal(|ui| {
                                ui.label("X");
                                if ui.add(egui::DragValue::new(&mut px).speed(0.5).prefix(" ")).changed() {
                                    pos_update = Some((obj_type.clone(), obj_idx, px, py, pz, pb));
                                }
                                ui.label("Y");
                                if ui.add(egui::DragValue::new(&mut py).speed(0.5).prefix(" ")).changed() {
                                    pos_update = Some((obj_type.clone(), obj_idx, px, py, pz, pb));
                                }
                            });
                            ui.horizontal(|ui| {
                                ui.label("Z");
                                if ui.add(egui::DragValue::new(&mut pz).speed(0.2).prefix(" ")).changed() {
                                    pos_update = Some((obj_type.clone(), obj_idx, px, py, pz, pb));
                                }
                                ui.label("Rot");
                                if ui.add(egui::DragValue::new(&mut pb).speed(0.05).prefix(" ").range(-6.28..=6.28)).changed() {
                                    pos_update = Some((obj_type.clone(), obj_idx, px, py, pz, pb));
                                }
                            });
                            ui.separator();
                        }
                        if ui.button("🗑 Delete").clicked() {
                            delete_requested = true;
                        }
                        ui.separator();
                        if let Some((ref obj_type, obj_idx)) = self.selected_obj_info {
                            if ui.button("📋 Local Variables").clicked() {
                                if let Some(ref module) = self.module {
                                    if let Some(ref area) = self.current_area {
                                        let vars = load_local_vars(module, &area.resref, obj_type, obj_idx);
                                        self.local_var_editor = Some(LocalVarEditorState {
                                            obj_type: obj_type.clone(),
                                            obj_idx,
                                            vars,
                                        });
                                    }
                                }
                            }
                            if ui.button("🎨 Appearance Picker").clicked() {
                                let appearances = self.module.as_ref().and_then(|m| m.erf.as_ref()).and_then(|erf| {
                                    erf.get("appearance", nwn_gamemodel::resource_types::TWO_DA).and_then(|data| {
                                        let text = String::from_utf8(data.clone()).ok()?;
                                        let mut entries = Vec::new();
                                        for line in text.lines() {
                                            let trimmed = line.trim();
                                            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("2DA V2") { continue; }
                                            let parts: Vec<&str> = trimmed.split(|c| c == '\t' || c == ' ').filter(|s| !s.is_empty()).collect();
                                            if parts.len() >= 2 {
                                                if let Ok(id) = parts[0].parse::<i32>() {
                                                    if id > 0 {
                                                        let label = if parts.len() > 1 { parts[1].to_string() } else { String::new() };
                                                        let model = if parts.len() > 2 { parts[2].to_string() } else { String::new() };
                                                        entries.push(AppearanceEntry { id, label, model });
                                                    }
                                                }
                                            }
                                        }
                                        Some(entries)
                                    })
                                }).unwrap_or_default();
                                self.appearance_picker = Some(AppearancePickerState {
                                    obj_type: obj_type.clone(),
                                    obj_idx,
                                    filter: String::new(),
                                    appearances,
                                });
                            }
                            if ui.button("💬 Comment").clicked() {
                                let resref = self.current_area.as_ref().map(|a| a.resref.clone()).unwrap_or_default();
                                let comment = self.module.as_ref().map(|m| load_object_comment(m, &resref, obj_type, obj_idx)).unwrap_or_default();
                                self.object_comment_edit = Some((obj_type.clone(), obj_idx, comment));
                            }
                            if obj_type == "creature" || obj_type == "placeable" {
                                if ui.button("📦 Inventory").clicked() {
                                    let resref = self.current_area.as_ref().map(|a| a.resref.clone()).unwrap_or_default();
                                    let items = load_item_list(self.module.as_ref(), &resref, obj_type, obj_idx);
                                    self.inventory_editor = Some(InventoryEditorState {
                                        obj_type: obj_type.clone(),
                                        obj_idx,
                                        items,
                                        editing_item: None,
                                        add_blueprint_filter: String::new(),
                                        add_blueprint_results: Vec::new(),
                                    });
                                }
                            }
                        }
                        ui.separator();
                    }
                    inspector_modified = show_inspector(ui, obj);
                } else if matches!(self.placement_state, PlacementState::Pending(..)) {
                    ui.colored_label(egui::Color32::YELLOW, "Placement mode active");
                    ui.label("Click in the viewport to place the object.");
                    ui.label("Press Escape to cancel.");
                } else {
                    ui.colored_label(egui::Color32::GRAY, "Select an object to inspect");
                }
            });
        // Handle position update from inspector
        if let Some((obj_type, obj_idx, px, py, pz, pb)) = pos_update {
            self.update_obj_pos(&obj_type, obj_idx, px, py, pz, pb);
        }
        // Handle delete button
        if delete_requested {
            self.delete_selected_object();
        }
        if inspector_modified {
            let obj_info = self.selected_obj_info.clone();
            let area_resref = self.current_area.as_ref().map(|a| a.resref.clone());
            if let (Some(ref obj), Some((ref obj_type, obj_idx))) = (self.selected_object.as_ref(), obj_info) {
                if let Some(ref mut module) = self.module {
                    if let Some(ref resref) = area_resref {
                        if save_bag_to_git(module, resref, obj_type, obj_idx, &obj.bag) {
                            self.status = "Property saved to GIT.".to_string();
                        }
                    }
                }
            }
            // Reload area from updated GIT (separate borrow scope)
            let resref = area_resref.clone();
            if let (Some(ref mut module), Some(ref resref)) = (self.module.as_mut(), resref) {
                let git_key = resref.trim().to_lowercase();
                if let Some(git_data) = module.raw_git.get(&git_key) {
                    if let Ok(git_gff) = nwn_gff::GffFile::from_bytes(git_data) {
                        if let Some(area) = self.current_area.as_mut() {
                            let area = Arc::make_mut(area);
                            nwn_gamemodel::reload_from_git(area, &git_gff);
                        }
                    }
                }
            }
            self.push_undo();
            self.area_version = self.area_version.wrapping_add(1);
            if let Some(ref area) = self.current_area {
                self.viewport.set_area(area.clone(), self.area_version, target_format);
            }
        }
    }
}

impl AuroraBorealisApp {
    fn load_module_at_path(&mut self, _p: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = _p;
            match nwn_gamemodel::Module::open(path) {
                Ok(module) => {
                    let area_count = module.num_areas();
                    let module_name = module.name.clone();
                    self.module = Some(module);
                    self.current_area = None;
                    self.open_areas.clear();
                    self.active_area_idx = 0;
                    self.selected_object = None;
                    self.selected_obj_info = None;
                    self.selection_list.clear();
                    self.status = format!("Loaded module '{}' with {} areas.", module_name, area_count);
                }
                Err(nwn_gamemodel::ModelError::MissingHaks(haks)) => {
                    let list = haks.join(", ");
                    self.status = format!("Cannot load module — missing required hak files: {}", list);
                }
                Err(e) => {
                    self.status = format!("Failed to load module: {}", e);
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.status = "File system loading not available on web.".to_string();
        }
    }

    #[allow(dead_code)]
    fn load_module_from_bytes(&mut self, data: &[u8]) {
        match nwn_gamemodel::Module::from_bytes(data) {
            Ok(module) => {
                let area_count = module.num_areas();
                let module_name = module.name.clone();
                self.module = Some(module);
                self.current_area = None;
                self.open_areas.clear();
                self.active_area_idx = 0;
                self.selected_object = None;
                self.selected_obj_info = None;
                self.status = format!("Loaded module '{}' with {} areas.", module_name, area_count);
            }
            Err(nwn_gamemodel::ModelError::MissingHaks(haks)) => {
                let list = haks.join(", ");
                self.status = format!("Cannot load module — missing required hak files: {}", list);
            }
            Err(e) => {
                self.status = format!("Failed to load module: {}", e);
            }
        }
    }

    fn save_module(&mut self) {
        if let Some(ref mut module) = self.module {
            if module.path.to_string_lossy().is_empty() {
                if let Some(path) = pick_save_path(&module.name) {
                    self.save_module_as(&path);
                }
                return;
            }
            let path = module.path.to_string_lossy().to_string();
            self.save_module_as(&path);
        }
    }

    fn save_module_as(&mut self, path: &str) {
        if let Some(ref mut module) = self.module {
            let p = std::path::PathBuf::from(path);
            module.path = p.clone();
            match module.save_to(&p) {
                Ok(_) => {
                    self.status = format!("Module saved to '{}'.", path);
                }
                Err(e) => {
                    self.status = format!("Failed to save module: {}", e);
                }
            }
        }
    }

    fn launch_nwn(&mut self) {
        if let Some(ref module) = self.module {
            let mod_name = format!("{}.mod", module.name);
            let home = std::env::var("HOME").unwrap_or_default();
            let nwn_modules = std::path::PathBuf::from(&home)
                .join("Documents")
                .join("Neverwinter Nights")
                .join("modules")
                .join(&mod_name);

            if let Err(e) = std::fs::copy(&module.path, &nwn_modules) {
                self.status = format!("Failed to copy module to NWN dir: {}", e);
                return;
            }
            let nwn_binary = std::path::PathBuf::from(
                "/home/grimshackle/.local/share/Steam/steamapps/common/Neverwinter Nights/bin/linux-x86/nwmain-linux"
            );
            if nwn_binary.exists() {
                match std::process::Command::new(&nwn_binary)
                    .arg("+load")
                    .arg(&module.name)
                    .spawn()
                {
                    Ok(child) => {
                        self.nwn_process_handle = Some(child);
                        self.status = format!("Launched NWN:EE with module '{}'", module.name);
                    }
                    Err(e) => {
                        self.status = format!("Failed to launch NWN: {}", e);
                    }
                }
            } else {
                self.status = "NWN binary not found".to_string();
            }
        }
    }

    fn compile_script(&mut self, name: &str, source: &str) {
        let nwnsc_path = std::path::PathBuf::from(
            std::env::current_exe().unwrap_or_default().parent().unwrap_or(std::path::Path::new("."))
        ).join("nwnsc");
        if !nwnsc_path.exists() {
            self.status = "nwnsc binary not found alongside executable".to_string();
            return;
        }
        let tmp_dir = std::env::temp_dir().join("aurora_nwnsc");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let nss_path = tmp_dir.join(format!("{}.nss", name));
        let ncs_path = tmp_dir.join(format!("{}.ncs", name));
        if std::fs::write(&nss_path, source).is_err() {
            self.status = format!("Failed to write temp NSS for '{}'", name);
            return;
        }
        let nwn_install = std::path::PathBuf::from(
            "/home/grimshackle/.local/share/Steam/steamapps/common/Neverwinter Nights"
        );
        let output = std::process::Command::new(&nwnsc_path)
            .arg("-n").arg(&nwn_install)
            .arg("-q")
            .arg("-r").arg(&ncs_path)
            .arg(&nss_path)
            .output();
        match output {
            Ok(out) => {
                if !out.status.success() {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    self.status = format!("Compile failed for '{}': {}{}", name, stdout, stderr);
                    return;
                }
                match std::fs::read(&ncs_path) {
                    Ok(ncs_data) => {
                        if let Some(ref mut module) = self.module {
                            module.update_ncs(name, &ncs_data);
                            module.modified = true;
                            self.status = format!("Compiled '{}' successfully ({})", name, ncs_data.len());
                        }
                    }
                    Err(e) => {
                        self.status = format!("Failed to read NCS output for '{}': {}", name, e);
                    }
                }
            }
            Err(e) => {
                self.status = format!("Failed to run nwnsc for '{}': {}", name, e);
            }
        }
    }

    fn update_obj_pos(&mut self, obj_type: &str, obj_idx: usize, x: f32, y: f32, z: f32, bearing: f32) {
        self.push_undo();
        let (x, y) = snap_to_grid_pos(x, y, self.snap_to_grid_enabled, self.snap_to_grid_value);
        if let Some(area) = self.current_area.as_mut() {
            let area = Arc::make_mut(area);
            match obj_type {
                "creature" => {
                    if let Some(c) = area.creatures.get_mut(obj_idx) {
                        c.x = x; c.y = y; c.z = z; c.bearing = bearing;
                    }
                }
                "door" => {
                    if let Some(d) = area.doors.get_mut(obj_idx) {
                        d.x = x; d.y = y; d.z = z; d.bearing = bearing;
                    }
                }
                "placeable" => {
                    if let Some(p) = area.placeables.get_mut(obj_idx) {
                        p.x = x; p.y = y; p.z = z; p.bearing = bearing;
                    }
                }
                "trigger" => {
                    if let Some(t) = area.triggers.get_mut(obj_idx) {
                        t.x = x; t.y = y; t.z = z; t.bearing = bearing;
                    }
                }
                "waypoint" => {
                    if let Some(w) = area.waypoints.get_mut(obj_idx) {
                        w.x = x; w.y = y; w.z = z; w.bearing = bearing;
                    }
                }
                "encounter" => {
                    if let Some(e) = area.encounters.get_mut(obj_idx) {
                        e.x = x; e.y = y; e.z = z; e.bearing = bearing;
                    }
                }
                "sound_emitter" => {
                    if let Some(s) = area.sound_emitters.get_mut(obj_idx) {
                        s.x = x; s.y = y; s.z = z; s.bearing = bearing;
                    }
                }
                _ => {}
            }
            let resref = area.resref.clone();
            if let Some(ref mut module) = self.module {
                module.update_area_git(&resref, area);
            }
            self.area_version = self.area_version.wrapping_add(1);
            self.selected_object = None;
            self.selected_obj_info = None;
            self.status = format!("Updated {} position", obj_type);
        }
    }

    fn push_undo(&mut self) {
        if let Some(ref area) = self.current_area {
            self.undo_stack.push(Arc::new(area.as_ref().clone()));
            if self.undo_stack.len() > 50 {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
        }
    }

    fn tile_flood_fill(&mut self, col: usize, row: usize, new_id: i32) {
        let (w, h, old_id) = match self.current_area.as_ref() {
            Some(a) => {
                let w = a.width as usize;
                let h = a.height as usize;
                if col >= w || row >= h { return; }
                let idx = row * w + col;
                let old_id = a.tiles[idx].id;
                if old_id == new_id { return; }
                (w, h, old_id)
            }
            None => return,
        };
        self.push_undo();
        let area = Arc::make_mut(self.current_area.as_mut().unwrap());
        let mut visited = vec![false; area.tiles.len()];
        let mut stack = vec![(col, row)];
        while let Some((cx, cy)) = stack.pop() {
            if cx >= w || cy >= h { continue; }
            let ci = cy * w + cx;
            if visited[ci] { continue; }
            visited[ci] = true;
            if area.tiles[ci].id != old_id { continue; }
            area.tiles[ci].id = new_id;
            if cx > 0 { stack.push((cx - 1, cy)); }
            if cx + 1 < w { stack.push((cx + 1, cy)); }
            if cy > 0 { stack.push((cx, cy - 1)); }
            if cy + 1 < h { stack.push((cx, cy + 1)); }
        }
        let resref = area.resref.clone();
        if let Some(ref mut module) = self.module {
            module.update_area_are(&resref, area);
        }
        self.area_version = self.area_version.wrapping_add(1);
        self.status = format!("Flood filled tile {} at ({},{})", new_id, col, row);
    }

    fn auto_orient_tiles(&mut self) {
        if self.current_area.is_none() { return; }
        self.push_undo();
        let area = Arc::make_mut(self.current_area.as_mut().unwrap());
        let w = area.width as usize;
        let h = area.height as usize;
        for y in 0..h {
            for x in 0..w {
                let idx = y * w + x;
                let h_self = area.tiles[idx].height;
                let mut downhill: Option<u8> = None;
                let mut max_drop: i32 = 0;
                let dirs: [(i32, i32, u8); 4] = [
                    (0, -1, 0), (1, 0, 1), (0, 1, 2), (-1, 0, 3),
                ];
                for (dx, dy, dir) in dirs {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                        let ni = ny as usize * w + nx as usize;
                        let nh = area.tiles[ni].height;
                        let drop = h_self - nh;
                        if drop > max_drop {
                            max_drop = drop;
                            downhill = Some(dir);
                        }
                    }
                }
                if let Some(dir) = downhill {
                    area.tiles[idx].orientation = dir as i32;
                }
            }
        }
        let resref = area.resref.clone();
        if let Some(ref mut module) = self.module {
            module.update_area_are(&resref, area);
        }
        self.area_version = self.area_version.wrapping_add(1);
        self.status = "Auto-oriented tiles to face downhill.".to_string();
    }

    fn fill_area_tiles(&mut self, tile_id: i32) {
        if self.current_area.is_none() { return; }
        self.push_undo();
        let area = Arc::make_mut(self.current_area.as_mut().unwrap());
        for tile in area.tiles.iter_mut() {
            tile.id = tile_id;
        }
        let resref = area.resref.clone();
        if let Some(ref mut module) = self.module {
            module.update_area_are(&resref, area);
        }
        self.area_version = self.area_version.wrapping_add(1);
        self.status = format!("Filled area with tile {}", tile_id);
    }

    fn update_visibility_filter(&mut self, target_format: wgpu::TextureFormat) {
        let filter = VisibilityFilter {
            show_tiles: true,
            show_creatures: self.show_creatures,
            show_doors: self.show_doors,
            show_placeables: self.show_placeables,
            show_triggers: self.show_triggers,
            show_waypoints: self.show_waypoints,
            show_encounters: self.show_encounters,
            show_sound_emitters: self.show_sound_emitters,
        };
        self.viewport.set_visibility_filter(filter);
        if let Some(ref area) = self.current_area {
            self.area_version = self.area_version.wrapping_add(1);
            self.viewport.set_area(area.clone(), self.area_version, target_format);
        }
    }

    fn switch_to_area(&mut self, name: &str, target_format: wgpu::TextureFormat) {
        // No-op if already current
        if self.current_area.as_ref().map_or(false, |a| a.resref == name) {
            return;
        }
        if let Some(area) = self.module.as_ref().and_then(|m| m.get_area(name)) {
            if let Some(idx) = self.open_areas.iter().position(|r| r == name) {
                self.active_area_idx = idx;
            } else {
                self.open_areas.push(name.to_string());
                self.active_area_idx = self.open_areas.len() - 1;
            }
            self.area_version = self.area_version.wrapping_add(1);
            self.viewport.set_area(area.clone(), self.area_version, target_format);
            self.current_area = Some(area);
            self.selected_object = None;
            self.selected_obj_info = None;
            self.selection_list.clear();
            self.selected_tile = None;
            self.status = format!("Loaded area '{}'.", name);
        }
    }

    fn close_area_tab(&mut self, idx: usize) {
        if idx >= self.open_areas.len() {
            return;
        }
        let was_active = idx == self.active_area_idx;
        self.open_areas.remove(idx);
        if self.open_areas.is_empty() {
            self.current_area = None;
            self.selected_object = None;
            self.selected_obj_info = None;
            self.selected_tile = None;
            self.active_area_idx = 0;
            self.area_version = self.area_version.wrapping_add(1);
        } else {
            if was_active {
                self.current_area = None;
                self.selected_object = None;
                self.selected_obj_info = None;
                self.selected_tile = None;
                self.active_area_idx = self.active_area_idx.min(self.open_areas.len() - 1);
            } else if self.active_area_idx >= self.open_areas.len() {
                self.active_area_idx = self.open_areas.len() - 1;
            }
        }
    }

    fn undo(&mut self, target_format: wgpu::TextureFormat) {
        if let Some(area) = self.undo_stack.pop() {
            if let Some(ref current) = self.current_area {
                self.redo_stack.push(Arc::new(current.as_ref().clone()));
            }
            self.current_area = Some(area.clone());
            self.viewport.set_area(area, self.area_version, target_format);
            self.selected_object = None;
            self.selected_obj_info = None;
            self.selection_list.clear();
            self.status = "Undo".to_string();
            self.area_version = self.area_version.wrapping_add(1);
        } else {
            self.status = "Nothing to undo.".to_string();
        }
    }

    fn redo(&mut self, target_format: wgpu::TextureFormat) {
        if let Some(area) = self.redo_stack.pop() {
            if let Some(ref current) = self.current_area {
                self.undo_stack.push(Arc::new(current.as_ref().clone()));
            }
            self.current_area = Some(area.clone());
            self.viewport.set_area(area, self.area_version, target_format);
            self.selected_object = None;
            self.selected_obj_info = None;
            self.selection_list.clear();
            self.status = "Redo".to_string();
            self.area_version = self.area_version.wrapping_add(1);
        } else {
            self.status = "Nothing to redo.".to_string();
        }
    }

    fn copy_selected_object(&mut self) {
        if let Some((ref obj_type, obj_idx)) = self.selected_obj_info.clone() {
            if let Some(ref area) = self.current_area {
                let copy_data = match obj_type.as_str() {
                    "creature" => area.creatures.get(obj_idx).map(|c| (c.resref.clone(), c.tag.clone(), c.bearing, c.to_property_bag())),
                    "door" => area.doors.get(obj_idx).map(|d| (d.resref.clone(), d.tag.clone(), d.bearing, d.to_property_bag())),
                    "placeable" => area.placeables.get(obj_idx).map(|p| (p.resref.clone(), p.tag.clone(), p.bearing, p.to_property_bag())),
                    "trigger" => area.triggers.get(obj_idx).map(|t| (t.resref.clone(), t.tag.clone(), t.bearing, t.to_property_bag())),
                    "waypoint" => area.waypoints.get(obj_idx).map(|w| (w.resref.clone(), w.tag.clone(), w.bearing, w.to_property_bag())),
                    "encounter" => area.encounters.get(obj_idx).map(|e| (e.tag.clone(), e.tag.clone(), e.bearing, e.to_property_bag())),
                    "sound_emitter" => area.sound_emitters.get(obj_idx).map(|s| (s.tag.clone(), s.tag.clone(), s.bearing, s.to_property_bag())),
                    _ => None,
                };
                if let Some((resref, tag, bearing, bag)) = copy_data {
                    self.status = format!("Copied {} '{}'", obj_type, resref);
                    self.clipboard = Some(ObjectClipboard {
                        obj_type: obj_type.clone(),
                        resref,
                        tag,
                        bearing,
                        bag,
                    });
                } else {
                    self.status = format!("Cannot copy {} objects yet", obj_type);
                }
            }
        } else {
            self.status = "No object selected to copy.".to_string();
        }
    }

    fn paste_object(&mut self) {
        self.push_undo();
        let snap_enabled = self.snap_to_grid_enabled;
        let snap_value = self.snap_to_grid_value;
        if let Some(ref clip) = self.clipboard.clone() {
            let offset_x = 2.0;
            let offset_y = 2.0;
            if let Some(area) = self.current_area.as_mut() {
                let area = Arc::make_mut(area);
                let tag = format!("{}_{}", clip.obj_type, clip.resref);
                match clip.obj_type.as_str() {
                    "creature" => {
                        let mut inst = nwn_gamemodel::CreatureInstance::from_property_bag(&clip.bag);
                        inst.tag = tag;
                        let (sx, sy) = snap_to_grid_pos(inst.x + offset_x, inst.y + offset_y, snap_enabled, snap_value);
                        inst.x = sx; inst.y = sy;
                        area.creatures.push(inst);
                    }
                    "door" => {
                        let mut inst = nwn_gamemodel::DoorInstance::from_property_bag(&clip.bag);
                        inst.tag = tag;
                        let (sx, sy) = snap_to_grid_pos(inst.x + offset_x, inst.y + offset_y, snap_enabled, snap_value);
                        inst.x = sx; inst.y = sy;
                        area.doors.push(inst);
                    }
                    "placeable" => {
                        let mut inst = nwn_gamemodel::PlaceableInstance::from_property_bag(&clip.bag);
                        inst.tag = tag;
                        let (sx, sy) = snap_to_grid_pos(inst.x + offset_x, inst.y + offset_y, snap_enabled, snap_value);
                        inst.x = sx; inst.y = sy;
                        area.placeables.push(inst);
                    }
                    "trigger" => {
                        let mut inst = nwn_gamemodel::TriggerInstance::from_property_bag(&clip.bag);
                        inst.tag = tag;
                        let (sx, sy) = snap_to_grid_pos(inst.x + offset_x, inst.y + offset_y, snap_enabled, snap_value);
                        inst.x = sx; inst.y = sy;
                        area.triggers.push(inst);
                    }
                    "waypoint" => {
                        let mut inst = nwn_gamemodel::WayPointInstance::from_property_bag(&clip.bag);
                        inst.tag = tag;
                        let (sx, sy) = snap_to_grid_pos(inst.x + offset_x, inst.y + offset_y, snap_enabled, snap_value);
                        inst.x = sx; inst.y = sy;
                        area.waypoints.push(inst);
                    }
                    "encounter" => {
                        let mut inst = nwn_gamemodel::EncounterInstance::from_property_bag(&clip.bag);
                        inst.tag = tag;
                        let (sx, sy) = snap_to_grid_pos(inst.x + offset_x, inst.y + offset_y, snap_enabled, snap_value);
                        inst.x = sx; inst.y = sy;
                        area.encounters.push(inst);
                    }
                    "sound_emitter" => {
                        let mut inst = nwn_gamemodel::SoundEmitterInstance::from_property_bag(&clip.bag);
                        inst.tag = tag;
                        if self.snap_to_grid_enabled {
                            let (sx, sy) = snap_to_grid_pos(inst.x, inst.y, true, self.snap_to_grid_value);
                            inst.x = sx; inst.y = sy;
                        }
                        area.sound_emitters.push(inst);
                    }
                    _ => {
                        self.status = format!("Cannot paste {} objects yet", clip.obj_type);
                        return;
                    }
                }
                let resref = area.resref.clone();
                if let Some(ref mut module) = self.module {
                    module.update_area_git(&resref, area);
                }
                self.area_version = self.area_version.wrapping_add(1);
                self.selected_object = None;
                self.selected_obj_info = None;
                self.selection_list.clear();
                self.status = format!("Pasted {} '{}'", clip.obj_type, clip.resref);
            }
        } else {
            self.status = "Nothing to paste. Copy an object first.".to_string();
        }
    }

    fn delete_selected_object(&mut self) {
        self.push_undo();
        let targets: Vec<(String, usize)> = if self.selection_list.is_empty() {
            self.selected_obj_info.clone().into_iter().collect()
        } else {
            self.selection_list.clone()
        };
        if targets.is_empty() { return; }
        if let Some(area) = self.current_area.as_mut() {
            let area = Arc::make_mut(area);
            let mut to_delete: Vec<(String, usize)> = targets.clone();
            to_delete.sort_by(|a, b| b.1.cmp(&a.1));
            for (ref obj_type, obj_idx) in to_delete {
                match obj_type.as_str() {
                    "creature" => { if obj_idx < area.creatures.len() { area.creatures.remove(obj_idx); } }
                    "door" => { if obj_idx < area.doors.len() { area.doors.remove(obj_idx); } }
                    "placeable" => { if obj_idx < area.placeables.len() { area.placeables.remove(obj_idx); } }
                    "trigger" => { if obj_idx < area.triggers.len() { area.triggers.remove(obj_idx); } }
                    "waypoint" => { if obj_idx < area.waypoints.len() { area.waypoints.remove(obj_idx); } }
                    "encounter" => { if obj_idx < area.encounters.len() { area.encounters.remove(obj_idx); } }
                    "sound_emitter" => { if obj_idx < area.sound_emitters.len() { area.sound_emitters.remove(obj_idx); } }
                    _ => {}
                }
            }
            let resref = area.resref.clone();
            if let Some(ref mut module) = self.module {
                module.update_area_git(&resref, area);
            }
            self.area_version = self.area_version.wrapping_add(1);
        }
        self.selected_object = None;
        self.selected_obj_info = None;
        self.selection_list.clear();
        self.status = format!("Deleted {} object(s)", targets.len());
    }
}

#[allow(dead_code)]
fn toggle_walkmesh_wall(area: &mut nwn_gamemodel::Area, wall: (usize, u8), status: &mut String) {
    if let Some(pos) = area.walkmesh_walls.iter().position(|&w| w == wall) {
        area.walkmesh_walls.remove(pos);
        *status = "Wall removed.".to_string();
    } else {
        area.walkmesh_walls.push(wall);
        *status = "Wall added.".to_string();
    }
}

fn generate_walkmesh_walls(area: &mut nwn_gamemodel::Area, threshold: i32) -> usize {
    let w = area.width;
    let h = area.height;
    let mut new_walls: Vec<(usize, u8)> = Vec::new();

    for row in 0..h {
        for col in 0..w {
            let idx = (row * w + col) as usize;
            let ht = area.tiles[idx].height;

            // Right edge: perimeter or height diff with neighbor
            if col + 1 < w {
                let ri = idx + 1;
                if (area.tiles[ri].height - ht).abs() >= threshold {
                    new_walls.push((idx, 0));
                }
            }
            // Bottom edge: perimeter or height diff with neighbor
            if row + 1 < h {
                let bi = ((row + 1) * w + col) as usize;
                if (area.tiles[bi].height - ht).abs() >= threshold {
                    new_walls.push((idx, 1));
                }
            }
        }
    }

    let count = new_walls.len();
    area.walkmesh_walls = new_walls;
    count
}

fn recent_files_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".nwn_toolset_recent.json")
}

fn load_recent_files() -> Vec<String> {
    let path = recent_files_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn save_recent_files(files: &[String]) {
    if let Ok(data) = serde_json::to_string(files) {
        let _ = std::fs::write(recent_files_path(), &data);
    }
}

fn add_recent_file(files: &mut Vec<String>, path: &str) {
    files.retain(|f| f != path);
    files.insert(0, path.to_string());
    if files.len() > 10 {
        files.truncate(10);
    }
    save_recent_files(files);
}

fn height_to_color(height: i32) -> (u8, u8, u8) {
    let h = height.clamp(0, 255) as f32 / 255.0;
    // Blue (0,0,128) -> Cyan (0,128,255) -> Green (0,255,0) -> Yellow (255,255,0) -> Red (255,0,0)
    let (r, g, b) = if h < 0.25 {
        let t = h / 0.25;
        (0, (128.0 + t * 127.0) as u8, (128.0 + t * 127.0) as u8)
    } else if h < 0.5 {
        let t = (h - 0.25) / 0.25;
        (0, 255, (255.0 * (1.0 - t)) as u8)
    } else if h < 0.75 {
        let t = (h - 0.5) / 0.25;
        ((255.0 * t) as u8, 255, 0)
    } else {
        let t = (h - 0.75) / 0.25;
        (255, (255.0 * (1.0 - t)) as u8, 0)
    };
    (r, g, b)
}

fn export_minimap_png(area: &nwn_gamemodel::Area, path: &str) -> Result<(), String> {
    let w = area.width.max(1) as usize;
    let h = area.height.max(1) as usize;
    let scale = if w.max(h) > 100 { 1 } else if w.max(h) > 50 { 2 } else { 4 };
    let pw = w * scale;
    let ph = h * scale;
    let mut pixels = vec![0u8; pw * ph * 4];
    for row in 0..h {
        for col in 0..w {
            let ti = row * w + col;
            let tile = area.tiles.get(ti);
            let tint = area.tile_tints.iter().find(|&&(t, _)| t == ti).map(|&(_, c)| c);
            let color = if let Some(c) = tint {
                [c[0], c[1], c[2], 255]
            } else if let Some(t) = tile {
                let id = t.id as u32;
                let r = ((id * 137 + 53) % 200 + 55) as u8;
                let g = ((id * 251 + 97) % 200 + 55) as u8;
                let b = ((id * 199 + 173) % 200 + 55) as u8;
                [r, g, b, 255]
            } else {
                [40, 40, 40, 255]
            };
            for dy in 0..scale {
                for dx in 0..scale {
                    let pi = ((row * scale + dy) * pw + (col * scale + dx)) * 4;
                    pixels[pi..pi + 4].copy_from_slice(&color);
                }
            }
        }
    }
    for obj in &area.creatures {
        let cx = (obj.x / 10.0) as isize * scale as isize + scale as isize / 2;
        let cy = (obj.y / 10.0) as isize * scale as isize + scale as isize / 2;
        if cx >= 0 && cx < pw as isize && cy >= 0 && cy < ph as isize {
            let pi = (cy as usize * pw + cx as usize) * 4;
            pixels[pi..pi + 4].copy_from_slice(&[255, 60, 60, 255]);
        }
    }
    for obj in &area.doors {
        let cx = (obj.x / 10.0) as isize * scale as isize + scale as isize / 2;
        let cy = (obj.y / 10.0) as isize * scale as isize + scale as isize / 2;
        if cx >= 0 && cx < pw as isize && cy >= 0 && cy < ph as isize {
            let pi = (cy as usize * pw + cx as usize) * 4;
            pixels[pi..pi + 4].copy_from_slice(&[255, 200, 50, 255]);
        }
    }
    for obj in &area.placeables {
        let cx = (obj.x / 10.0) as isize * scale as isize + scale as isize / 2;
        let cy = (obj.y / 10.0) as isize * scale as isize + scale as isize / 2;
        if cx >= 0 && cx < pw as isize && cy >= 0 && cy < ph as isize {
            let pi = (cy as usize * pw + cx as usize) * 4;
            pixels[pi..pi + 4].copy_from_slice(&[80, 120, 255, 255]);
        }
    }
    let img = image::RgbaImage::from_raw(pw as u32, ph as u32, pixels)
        .ok_or("failed to create image")?;
    img.save(path).map_err(|e| format!("save error: {}", e))
}

#[allow(dead_code)]
fn move_object_to(area: &mut nwn_gamemodel::Area, obj_type: &str, index: usize, nx: f32, ny: f32) -> bool {
    match obj_type {
        "creature" => {
            if let Some(c) = area.creatures.get_mut(index) { c.x = nx; c.y = ny; true }
            else { false }
        }
        "door" => {
            if let Some(d) = area.doors.get_mut(index) { d.x = nx; d.y = ny; true }
            else { false }
        }
        "placeable" => {
            if let Some(p) = area.placeables.get_mut(index) { p.x = nx; p.y = ny; true }
            else { false }
        }
        "trigger" => {
            if let Some(t) = area.triggers.get_mut(index) { t.x = nx; t.y = ny; true }
            else { false }
        }
        "waypoint" => {
            if let Some(w) = area.waypoints.get_mut(index) { w.x = nx; w.y = ny; true }
            else { false }
        }
        "encounter" => {
            if let Some(e) = area.encounters.get_mut(index) { e.x = nx; e.y = ny; true }
            else { false }
        }
        "sound_emitter" => { if let Some(s) = area.sound_emitters.get_mut(index) { s.x = nx; s.y = ny; true } else { false } }
        _ => false,
    }
}

#[allow(dead_code)]
fn snap_to_grid(val: f32, grid_size: f32) -> f32 {
    if grid_size <= 0.0 { val }
    else { (val / grid_size).round() * grid_size }
}

fn snap_to_grid_pos(x: f32, y: f32, enabled: bool, grid_size: f32) -> (f32, f32) {
    if enabled && grid_size > 0.0 {
        (snap_to_grid(x, grid_size), snap_to_grid(y, grid_size))
    } else {
        (x, y)
    }
}

fn git_struct_id(obj_type: &str) -> u32 {
    match obj_type {
        "creature" => 4,
        "door" => 5,
        "placeable" => 6,
        "trigger" => 7,
        "waypoint" => 8,
        "encounter" => 9,
        "sound_emitter" => 10,
        _ => 0,
    }
}

fn git_list_name(obj_type: &str) -> &'static str {
    match obj_type {
        "creature" => "Creature_List",
        "door" => "Door_List",
        "placeable" => "Placeable_List",
        "trigger" => "Trigger_List",
        "waypoint" => "WaypointList",
        "encounter" => "Encounter_List",
        "sound_emitter" => "SoundList",
        _ => "",
    }
}

fn select_single_object(
    area: &nwn_gamemodel::Area,
    module: Option<&nwn_gamemodel::Module>,
    selected_object: &mut Option<SelectedObject>,
    selected_obj_info: &mut Option<(String, usize)>,
    obj_type: &str,
    idx: usize,
) {
    let bag = match obj_type {
        "creature" => area.creatures.get(idx).map(|c| (format!("Creature: {}", c.resref), c.to_property_bag())),
        "door" => area.doors.get(idx).map(|d| (format!("Door: {}", d.resref), d.to_property_bag())),
        "placeable" => area.placeables.get(idx).map(|p| (format!("Placeable: {}", p.resref), p.to_property_bag())),
        "trigger" => area.triggers.get(idx).map(|t| (format!("Trigger: {}", t.tag), t.to_property_bag())),
        "waypoint" => area.waypoints.get(idx).map(|w| (format!("Waypoint: {}", w.tag), w.to_property_bag())),
        "encounter" => area.encounters.get(idx).map(|e| (format!("Encounter: {}", e.name), e.to_property_bag())),
        "sound_emitter" => area.sound_emitters.get(idx).map(|s| (format!("Sound: {}", s.tag), s.to_property_bag())),
        _ => None,
    };
    if let Some((title, bag)) = bag {
        let full_bag = module.and_then(|module| {
            let area_key = area.resref.trim().to_lowercase();
            let git_bytes = module.raw_git.get(&area_key)?;
            let gff = nwn_gff::GffFile::from_bytes(git_bytes).ok()?;
            let list_name = git_list_name(obj_type);
            let s = gff.root().find(list_name)?.unwrap_list().get(idx)?;
            Some(PropertyBag::from_gff_struct(s))
        });
        *selected_object = Some(SelectedObject { title, bag: full_bag.unwrap_or(bag) });
        *selected_obj_info = Some((obj_type.to_string(), idx));
    }
}

fn save_bag_to_git(module: &mut nwn_gamemodel::Module, area_resref: &str, obj_type: &str, idx: usize, bag: &PropertyBag) -> bool {
    let key = area_resref.trim().to_lowercase();
    let bytes = match module.raw_git.get(&key) {
        Some(b) => b.clone(),
        None => return false,
    };
    let mut gff = match nwn_gff::GffFile::from_bytes(&bytes) {
        Ok(g) => g,
        Err(_) => return false,
    };
    let list_name = git_list_name(obj_type);
    let struct_id = git_struct_id(obj_type);
    let new_struct = bag.to_gff_struct(struct_id);
    match gff.root_mut().find_mut(list_name) {
        Some(nwn_gff::FieldValue::List(list)) => {
            if idx < list.len() {
                list[idx] = new_struct;
            } else {
                return false;
            }
        }
        _ => return false,
    }
    let data = gff.to_bytes();
    module.raw_git.insert(key, data);
    module.modified = true;
    true
}

#[allow(dead_code)]
fn do_find_replace(
    module: &mut nwn_gamemodel::Module,
    find: &str,
    replace: &str,
    case_sensitive: bool,
) -> (usize, usize) {
    let script_names: Vec<String> = module.script_names().into_iter().map(String::from).collect();
    let mut total_found = 0usize;
    let mut total_replaced = 0usize;
    for sname in &script_names {
        if let Some(source) = module.get_script_source(sname) {
            let mut modified = source.to_string();
            let find_lower = find.to_lowercase();
            let src_lower = modified.to_lowercase();
            if case_sensitive {
                let count = modified.matches(find).count();
                total_found += count;
                if !replace.is_empty() && count > 0 {
                    modified = modified.replace(find, replace);
                    total_replaced += count;
                }
            } else {
                let count = src_lower.matches(&find_lower).count();
                total_found += count;
                if !replace.is_empty() && count > 0 {
                    let mut result = String::new();
                    let mut last = 0;
                    for (idx, _) in src_lower.match_indices(&find_lower) {
                        result.push_str(&modified[last..idx]);
                        result.push_str(replace);
                        last = idx + find.len();
                    }
                    result.push_str(&modified[last..]);
                    modified = result;
                    total_replaced += count;
                }
            }
            if modified != source {
                module.update_script(sname, &modified);
            }
        }
    }
    (total_found, total_replaced)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run_native() {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 900.0]),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "The Aurora BorealiuS Toolset",
        native_options,
        Box::new(|cc| Ok(Box::new(AuroraBorealisApp::new(cc)))),
    ) {
        eprintln!("FATAL: Failed to start: {}", e);
        std::process::exit(1);
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct WebHandle {
    runner: eframe::WebRunner,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl WebHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        eframe::WebLogger::init(log::LevelFilter::Debug).ok();
        Self {
            runner: eframe::WebRunner::new(),
        }
    }

    pub async fn start(&self, canvas: web_sys::HtmlCanvasElement) -> Result<(), wasm_bindgen::JsValue> {
        self.runner
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(AuroraBorealisApp::new(cc)))),
            )
            .await
    }

    pub fn destroy(&self) {
        self.runner.destroy();
    }

    pub fn has_panicked(&self) -> bool {
        self.runner.has_panicked()
    }

    pub fn panic_message(&self) -> Option<String> {
        self.runner.panic_summary().map(|s| s.message())
    }

    pub fn load_module(&self, _name: String, data: Vec<u8>) {
        if let Some(mut app) = self.runner.app_mut::<AuroraBorealisApp>() {
            app.load_module_from_bytes(&data);
        }
    }
}
