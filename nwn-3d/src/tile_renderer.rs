use std::collections::HashMap;
use nwn_gamemodel::Area;

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct ModelGroup {
    pub texture_name: Option<String>,
    pub data: MeshData,
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct InstanceData {
    pub row0: [f32; 4],
    pub row1: [f32; 4],
    pub row2: [f32; 4],
    pub row3: [f32; 4],
}

pub struct ModelCache {
    pub cache: HashMap<String, Vec<nwn_tileset::ModelMesh>>,
}

impl ModelCache {
    pub fn new() -> Self {
        Self { cache: HashMap::new() }
    }

    pub fn get_or_load(&mut self, name: &str) -> Option<&Vec<nwn_tileset::ModelMesh>> {
        if self.cache.contains_key(name) {
            return self.cache.get(name);
        }
        match nwn_tileset::load_model_meshes(name) {
            Ok(Some(meshes)) => {
                self.cache.insert(name.to_string(), meshes);
                self.cache.get(name)
            }
            _ => None,
        }
    }
}

fn tile_instance_transform(col: usize, row: usize, height: f32, orientation: u8) -> InstanceData {
    let tile_w = 10.0;
    let half = tile_w / 2.0;
    let angle_deg = match orientation { 1 => 90, 2 => 180, 3 => 270, _ => 0 };
    let rad = (angle_deg as f32).to_radians();
    let (s, c) = rad.sin_cos();
    let tx = col as f32 * tile_w;
    let tz = -(row as f32) * tile_w;
    InstanceData {
        row0: [c, 0.0, -s, 0.0],
        row1: [0.0, 1.0, 0.0, 0.0],
        row2: [s, 0.0, c, 0.0],
        row3: [tx + half - half*c + half*s, height, tz + half*s + half*c - half, 1.0],
    }
}

fn fallback_instance_transform(col: usize, row: usize, height: f32) -> InstanceData {
    let tile_w = 10.0;
    let tx = col as f32 * tile_w;
    let tz = -(row as f32) * tile_w;
    InstanceData {
        row0: [1.0, 0.0, 0.0, 0.0],
        row1: [0.0, 1.0, 0.0, 0.0],
        row2: [0.0, 0.0, 1.0, 0.0],
        row3: [tx, height, tz, 1.0],
    }
}

pub struct InstancedTiles {
    pub models: Vec<ModelGroup>,
    pub instance_data: Vec<Vec<InstanceData>>,
}

pub fn build_instanced_tiles(area: &Area, model_cache: &mut ModelCache) -> InstancedTiles {
    let cols = area.width.max(1) as usize;
    let rows = area.height.max(1) as usize;
    if cols == 0 || rows == 0 {
        return InstancedTiles { models: Vec::new(), instance_data: Vec::new() };
    }

    // Phase 1: collect unique model names
    let mut unique_models: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (i, _) in area.tiles.iter().enumerate() {
        if i >= cols * rows { break; }
        if let Some(Some(name)) = area.tile_models.get(i) {
            if !name.is_empty() && seen.insert(name.clone()) {
                unique_models.push(name.clone());
            }
        }
    }

    // Phase 2: load all models using multi-texture aware loader
    for name in &unique_models {
        model_cache.get_or_load(name);
    }

    // Maps (model_name, texture_group_index) -> group index
    let mut group_map: HashMap<(String, usize), usize> = HashMap::new();
    let mut models: Vec<ModelGroup> = Vec::new();
    let mut instance_data: Vec<Vec<InstanceData>> = Vec::new();
    let mut fallback_instances: Vec<InstanceData> = Vec::new();

    // Phase 3: build instances
    for (i, tile) in area.tiles.iter().enumerate() {
        if i >= cols * rows { break; }
        let col = i % cols;
        let row = i / cols;
        let model_name = area.tile_models.get(i).and_then(|m| m.as_deref()).unwrap_or("");
        let height = tile.height as f32 * 0.25;

        if let Some(meshes) = model_cache.get_or_load(model_name) {
            if meshes.is_empty() { continue; }
            let all_empty = meshes.iter().all(|m| m.indices.is_empty());
            if all_empty { continue; }

            for (mi, mesh) in meshes.iter().enumerate() {
                if mesh.indices.is_empty() { continue; }
                let gidx = if let Some(&idx) = group_map.get(&(model_name.to_string(), mi)) {
                    idx
                } else {
                    let idx = models.len();
                    group_map.insert((model_name.to_string(), mi), idx);
                    let vertices: Vec<Vertex> = mesh.positions.iter().zip(mesh.normals.iter().zip(mesh.uvs.iter())).map(|(p, (n, u))| {
                        Vertex { position: *p, normal: *n, uv: *u, color: [1.0, 1.0, 1.0, 1.0] }
                    }).collect();
                    let mut indices = mesh.indices.clone();
                    let vcount = vertices.len() as u32;
                    for idx in &mut indices {
                        if *idx >= vcount {
                            eprintln!("WARN: index {} >= vertex count {}", idx, vcount);
                            *idx = (*idx).min(vcount - 1);
                        }
                    }
                    models.push(ModelGroup {
                        texture_name: mesh.texture_name.clone(),
                        data: MeshData { vertices, indices },
                    });
                    instance_data.push(Vec::new());
                    idx
                };
                instance_data[gidx].push(tile_instance_transform(col, row, height, tile.orientation as u8));
            }
        } else {
            fallback_instances.push(fallback_instance_transform(col, row, height));
        }
    }

    if !fallback_instances.is_empty() {
        let verts = [
            [0.0, 0.0, 0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0, 1.0, 0.0],
            [10.0, 0.0, -10.0, 1.0, 1.0],
            [0.0, 0.0, -10.0, 0.0, 1.0],
        ];
        let vertices: Vec<Vertex> = verts.iter().map(|v| Vertex {
            position: [v[0], v[1], v[2]],
            normal: [0.0, 1.0, 0.0],
            uv: [v[3], v[4]],
            color: [1.0, 1.0, 1.0, 1.0],
        }).collect();
        let indices = vec![0u32, 1, 2, 0, 2, 3];

        let thick = 0.3;
        let deep = [
            [0.0, -thick, 0.0, 0.0, 0.0],
            [10.0, -thick, 0.0, 1.0, 0.0],
            [10.0, -thick, -10.0, 1.0, 1.0],
            [0.0, -thick, -10.0, 0.0, 1.0],
        ];
        let side_normals = [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [-1.0, 0.0, 0.0]];
        let side_quads = [(0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)];
        let all_verts = [verts, deep].concat();
        let mut box_vertices = vertices;
        let mut box_indices = indices;
        for (qi, &(a, b, c, d)) in side_quads.iter().enumerate() {
            let bv = box_vertices.len() as u32;
            for &idx in &[a, b, c, d] {
                box_vertices.push(Vertex {
                    position: [all_verts[idx][0], all_verts[idx][1], all_verts[idx][2]],
                    normal: side_normals[qi],
                    uv: [all_verts[idx][3], all_verts[idx][4]],
                    color: [0.7, 0.7, 0.7, 1.0],
                });
            }
            box_indices.extend_from_slice(&[bv, bv + 1, bv + 2, bv, bv + 2, bv + 3]);
        }

        models.push(ModelGroup {
            texture_name: None,
            data: MeshData { vertices: box_vertices, indices: box_indices },
        });
        instance_data.push(fallback_instances);
    }
    InstancedTiles { models, instance_data }
}

pub fn build_tile_mesh(area: &Area) -> MeshData {
    if area.width <= 0 || area.height <= 0 || area.tiles.is_empty() {
        return MeshData { vertices: Vec::new(), indices: Vec::new() };
    }
    let tile_w = 10.0;
    let tile_h = 10.0;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let cols = area.width as usize;
    let rows = area.height as usize;

    for (i, tile) in area.tiles.iter().enumerate() {
        let row = i / cols;
        let col = i % cols;
        if row >= rows { break; }
        let tx = col as f32 * tile_w;
        let ty = -(row as f32) * tile_h;
        let height = tile.height as f32 * 0.25;

        let base_index = vertices.len() as u32;
        let verts: [[f32; 5]; 4] = [
            [tx,       height, ty, 0.0, 0.0],
            [tx+tile_w, height, ty, 1.0, 0.0],
            [tx+tile_w, height, ty-tile_h, 1.0, 1.0],
            [tx,       height, ty-tile_h, 0.0, 1.0],
        ];
        for &v in &verts {
            vertices.push(Vertex {
                position: [v[0], v[1], v[2]],
                normal: [0.0, 1.0, 0.0],
                uv: [v[3], v[4]],
                color: [1.0, 1.0, 1.0, 1.0],
            });
        }
        indices.extend_from_slice(&[
            base_index, base_index + 1, base_index + 2,
            base_index, base_index + 2, base_index + 3,
        ]);

        let thick = 0.3;
        let low = height - thick;
        let deep: [[f32; 5]; 4] = [
            [tx,       low, ty, 0.0, 0.0],
            [tx+tile_w, low, ty, 1.0, 0.0],
            [tx+tile_w, low, ty-tile_h, 1.0, 1.0],
            [tx,       low, ty-tile_h, 0.0, 1.0],
        ];
        let side_quads = [(0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)];
        let side_normals = [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [-1.0, 0.0, 0.0]];
        let all_verts = [verts, deep].concat();
        for (qi, &(a, b, c, d)) in side_quads.iter().enumerate() {
            let base = vertices.len() as u32;
            for &idx in &[a, b, c, d] {
                vertices.push(Vertex {
                    position: [all_verts[idx][0], all_verts[idx][1], all_verts[idx][2]],
                    normal: side_normals[qi],
                    uv: [all_verts[idx][3], all_verts[idx][4]],
                    color: [0.7, 0.7, 0.7, 1.0],
                });
            }
            indices.extend_from_slice(&[
                base, base + 1, base + 2,
                base, base + 2, base + 3,
            ]);
        }
    }

    MeshData { vertices, indices }
}

#[derive(Debug, Clone, Copy)]
pub enum ObjectType {
    Creature,
    Door,
    Placeable,
    Trigger,
    Waypoint,
    Encounter,
    SoundEmitter,
}

#[derive(Debug, Clone, Copy)]
pub struct MarkerInfo {
    pub obj_type: ObjectType,
    pub index: usize,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisibilityFilter {
    pub show_tiles: bool,
    pub show_creatures: bool,
    pub show_doors: bool,
    pub show_placeables: bool,
    pub show_triggers: bool,
    pub show_waypoints: bool,
    pub show_encounters: bool,
    pub show_sound_emitters: bool,
}

impl Default for VisibilityFilter {
    fn default() -> Self {
        Self {
            show_tiles: true,
            show_creatures: true,
            show_doors: true,
            show_placeables: true,
            show_triggers: true,
            show_waypoints: true,
            show_encounters: true,
            show_sound_emitters: true,
        }
    }
}

pub struct MarkersResult {
    pub mesh: MeshData,
    pub markers: Vec<MarkerInfo>,
}

pub fn build_object_markers(area: &Area, filter: VisibilityFilter) -> MarkersResult {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut markers = Vec::new();

    eprintln!("DEBUG: Building markers for area '{}' — {} creatures, {} doors, {} placeables, {} triggers, {} waypoints, {} encounters",
        area.resref, area.creatures.len(), area.doors.len(), area.placeables.len(), area.triggers.len(), area.waypoints.len(), area.encounters.len());

    if filter.show_creatures {
        for (i, c) in area.creatures.iter().enumerate() {
            let oz = c.z + 0.3;
            markers.push(MarkerInfo { obj_type: ObjectType::Creature, index: i, x: c.x, y: c.y, z: oz });
            add_creature_marker(&mut vertices, &mut indices, c.x, c.y, oz, c.bearing, [1.0, 0.3, 0.3, 1.0]);
        }
    }
    if filter.show_doors {
        for (i, d) in area.doors.iter().enumerate() {
            markers.push(MarkerInfo { obj_type: ObjectType::Door, index: i, x: d.x, y: d.y, z: d.z });
            add_door_marker(&mut vertices, &mut indices, d.x, d.y, d.z, d.bearing, [1.0, 0.8, 0.0, 1.0]);
        }
    }
    if filter.show_placeables {
        for (i, p) in area.placeables.iter().enumerate() {
            let oz = p.z + 0.3;
            markers.push(MarkerInfo { obj_type: ObjectType::Placeable, index: i, x: p.x, y: p.y, z: oz });
            add_placeable_marker(&mut vertices, &mut indices, p.x, p.y, oz, [0.2, 0.4, 1.0, 1.0]);
        }
    }
    if filter.show_triggers {
        for (i, t) in area.triggers.iter().enumerate() {
            markers.push(MarkerInfo { obj_type: ObjectType::Trigger, index: i, x: t.x, y: t.y, z: t.z });
            add_trigger_marker(&mut vertices, &mut indices, t.x, t.y, t.z, [0.8, 0.2, 0.8, 0.3]);
        }
    }
    if filter.show_waypoints {
        for (i, w) in area.waypoints.iter().enumerate() {
            markers.push(MarkerInfo { obj_type: ObjectType::Waypoint, index: i, x: w.x, y: w.y, z: w.z });
            add_waypoint_marker(&mut vertices, &mut indices, w.x, w.y, w.z, [0.2, 1.0, 0.2, 1.0]);
        }
    }
    if filter.show_encounters {
        for (i, e) in area.encounters.iter().enumerate() {
            markers.push(MarkerInfo { obj_type: ObjectType::Encounter, index: i, x: e.x, y: e.y, z: e.z });
            add_encounter_marker(&mut vertices, &mut indices, e.x, e.y, e.z, [1.0, 0.5, 0.0, 0.8]);
        }
    }
    if filter.show_sound_emitters {
        for (i, s) in area.sound_emitters.iter().enumerate() {
            markers.push(MarkerInfo { obj_type: ObjectType::SoundEmitter, index: i, x: s.x, y: s.y, z: s.z });
            add_sound_emitter_marker(&mut vertices, &mut indices, s.x, s.y, s.z, [0.2, 0.8, 1.0, 0.8]);
        }
    }

    eprintln!("DEBUG: Built {} marker meshes with {} vertices, {} indices", markers.len(), vertices.len(), indices.len());

    MarkersResult { mesh: MeshData { vertices, indices }, markers }
}

pub fn build_area_floor(area: &Area) -> MeshData {
    let tile_w = 10.0;
    let cols = area.width.max(1) as usize;
    let rows = area.height.max(1) as usize;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Overlay checkerboard at tile height with transparency
    for row in 0..rows {
        for col in 0..cols {
            let tx = col as f32 * tile_w;
            let tz = -(row as f32) * tile_w;
            let base = vertices.len() as u32;
            let shade = if (row + col) % 2 == 0 { 0.25 } else { 0.20 };
            for &(dx, dz, u, v) in &[
                (0.0, 0.0, 0.0, 0.0),
                (tile_w, 0.0, 1.0, 0.0),
                (tile_w, -tile_w, 1.0, 1.0),
                (0.0, -tile_w, 0.0, 1.0),
            ] {
                vertices.push(Vertex {
                    position: [tx + dx, 0.0, tz + dz],
                    normal: [0.0, 1.0, 0.0],
                    uv: [u, v],
                    color: [shade, shade * 1.2, shade * 1.5, 0.3],
                });
            }
            indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }

    // Grid lines at tile boundaries
    for row in 0..=rows {
        let tz = -(row as f32) * tile_w;
        let base = vertices.len() as u32;
        for col in 0..cols {
            let tx = col as f32 * tile_w;
            for &(dx, dz) in &[(0.0, 0.0), (tile_w, 0.0), (tile_w, -0.05), (0.0, -0.05)] {
                vertices.push(Vertex {
                    position: [tx + dx, 0.0, tz + dz],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                    color: [0.3, 0.4, 0.6, 0.5],
                });
            }
            indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }

    for col in 0..=cols {
        let tx = col as f32 * tile_w;
        let base = vertices.len() as u32;
        for row in 0..rows {
            let tz = -(row as f32) * tile_w;
            for &(dx, dz) in &[(0.0, 0.0), (0.0, -tile_w), (-0.05, -tile_w), (-0.05, 0.0)] {
                vertices.push(Vertex {
                    position: [tx + dx, 0.0, tz + dz],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                    color: [0.3, 0.4, 0.6, 0.5],
                });
            }
            indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
    }

    MeshData { vertices, indices }
}

pub fn build_paint_overlay(area: &Area) -> MeshData {
    build_paint_overlay_inner(area, 0, false, [40, 80, 200, 160])
}

pub fn build_paint_overlay_ex(area: &Area, water_level: i32, show_water: bool, water_color: [u8; 4]) -> MeshData {
    build_paint_overlay_inner(area, water_level, show_water, water_color)
}

fn build_paint_overlay_inner(area: &Area, water_level: i32, show_water: bool, water_color: [u8; 4]) -> MeshData {
    let cols = area.width.max(1) as usize;
    let rows = area.height.max(1) as usize;
    let tile_w = 10.0;
    let tint_map: std::collections::HashMap<usize, [f32; 4]> = area.tile_tints.iter().map(|&(ti, c)| {
        (ti, [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, c[3] as f32 / 255.0])
    }).collect();
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut base_idx: u32 = 0;
    for ti in 0..(cols * rows) {
        if let Some(&tint) = tint_map.get(&ti) {
            if tint[3] <= 0.0 { continue; }
            let col = ti % cols;
            let row = ti / cols;
            let h = area.tiles.get(ti).map(|t| t.height).unwrap_or(0);
            let height = h as f32 * 0.25 + 0.05; // slight offset above tile
            let x0 = col as f32 * tile_w;
            let z0 = -(row as f32) * tile_w;
            let x1 = x0 + tile_w;
            let z1 = z0 - tile_w;
            vertices.extend_from_slice(&[
                Vertex { position: [x0, height, z0], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], color: tint },
                Vertex { position: [x1, height, z0], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0], color: tint },
                Vertex { position: [x1, height, z1], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0], color: tint },
                Vertex { position: [x0, height, z1], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0], color: tint },
            ]);
            indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2, base_idx, base_idx + 2, base_idx + 3]);
            base_idx += 4;
        }
    }
    // Water level overlay
    if show_water && water_level > 0 {
        let water_color = [water_color[0] as f32 / 255.0, water_color[1] as f32 / 255.0, water_color[2] as f32 / 255.0, water_color[3] as f32 / 255.0];
        let water_height = water_level as f32 * 0.25;
        for ti in 0..(cols * rows) {
            let col = ti % cols;
            let row = ti / cols;
            let x0 = col as f32 * tile_w;
            let z0 = -(row as f32) * tile_w;
            let x1 = x0 + tile_w;
            let z1 = z0 - tile_w;
            vertices.extend_from_slice(&[
                Vertex { position: [x0, water_height, z0], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], color: water_color },
                Vertex { position: [x1, water_height, z0], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0], color: water_color },
                Vertex { position: [x1, water_height, z1], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0], color: water_color },
                Vertex { position: [x0, water_height, z1], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0], color: water_color },
            ]);
            indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2, base_idx, base_idx + 2, base_idx + 3]);
            base_idx += 4;
        }
    }
    MeshData { vertices, indices }
}

pub fn build_walkmesh(area: &Area) -> MeshData {
    let cols = area.width.max(1) as usize;
    let rows = area.height.max(1) as usize;
    let tile_w = 10.0;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Build a lookup for walkmesh walls: tile_index -> [right, bottom] bools
    let mut wall_set = std::collections::HashSet::new();
    for &(ti, edge) in &area.walkmesh_walls {
        wall_set.insert((ti, edge));
    }

    for (i, tile) in area.tiles.iter().enumerate() {
        if i >= cols * rows { break; }
        let row = i / cols;
        let col = i % cols;
        let tx = col as f32 * tile_w;
        let tz = -(row as f32) * tile_w;
        let height = tile.height as f32 * 0.25;

        let walk_color = if tile.walkable {
            [0.0, 0.6, 0.0, 0.25]
        } else {
            [0.6, 0.0, 0.0, 0.35]
        };

        let base = vertices.len() as u32;
        for &(dx, dz) in &[(0.0, 0.0), (tile_w, 0.0), (tile_w, -tile_w), (0.0, -tile_w)] {
            vertices.push(Vertex {
                position: [tx + dx, height + 0.05, tz + dz],
                normal: [0.0, 1.0, 0.0],
                uv: [0.0, 0.0],
                color: walk_color,
            });
        }
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    // Edge walls between tiles at different heights
    for row in 0..rows {
        for col in 0..cols {
            let i = row * cols + col;
            if i >= area.tiles.len() { break; }
            let h = area.tiles[i].height as f32 * 0.25;
            let tx = col as f32 * tile_w;
            let tz = -(row as f32) * tile_w;

            // Check right neighbor
            if col + 1 < cols {
                let ri = row * cols + col + 1;
                if ri < area.tiles.len() {
                    let rh = area.tiles[ri].height as f32 * 0.25;
                    let has_wall = wall_set.contains(&(i, 0)) || wall_set.contains(&(ri, 2));
                    if (rh - h).abs() > 0.01 || has_wall {
                        let wall_color = if has_wall { [0.8, 0.3, 0.0, 0.6] } else { [0.0, 0.4, 0.0, 0.3] };
                        let base = vertices.len() as u32;
                        vertices.push(Vertex { position: [tx + tile_w, h, tz], normal: [1.0, 0.0, 0.0], uv: [0.0, 0.0], color: wall_color });
                        vertices.push(Vertex { position: [tx + tile_w, rh, tz], normal: [1.0, 0.0, 0.0], uv: [0.0, 0.0], color: wall_color });
                        vertices.push(Vertex { position: [tx + tile_w, rh, tz - tile_w], normal: [1.0, 0.0, 0.0], uv: [0.0, 0.0], color: wall_color });
                        vertices.push(Vertex { position: [tx + tile_w, h, tz - tile_w], normal: [1.0, 0.0, 0.0], uv: [0.0, 0.0], color: wall_color });
                        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                    }
                }
            }
            // Check bottom neighbor
            if row + 1 < rows {
                let bi = (row + 1) * cols + col;
                if bi < area.tiles.len() {
                    let bh = area.tiles[bi].height as f32 * 0.25;
                    let has_wall = wall_set.contains(&(i, 1)) || wall_set.contains(&(bi, 3));
                    if (bh - h).abs() > 0.01 || has_wall {
                        let wall_color = if has_wall { [0.8, 0.3, 0.0, 0.6] } else { [0.0, 0.4, 0.0, 0.3] };
                        let base = vertices.len() as u32;
                        vertices.push(Vertex { position: [tx, h, tz - tile_w], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0], color: wall_color });
                        vertices.push(Vertex { position: [tx, bh, tz - tile_w], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0], color: wall_color });
                        vertices.push(Vertex { position: [tx + tile_w, bh, tz - tile_w], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0], color: wall_color });
                        vertices.push(Vertex { position: [tx + tile_w, h, tz - tile_w], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0], color: wall_color });
                        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                    }
                }
            }
        }
    }

    MeshData { vertices, indices }
}

pub fn build_walkmesh_edges(area: &Area) -> (Vec<f32>, Vec<u32>) {
    let cols = area.width.max(1) as usize;
    let rows = area.height.max(1) as usize;
    let tile_w = 10.0;
    let mut verts: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut wall_set = std::collections::HashSet::new();
    for &(ti, edge) in &area.walkmesh_walls {
        wall_set.insert((ti, edge));
    }

    for (i, tile) in area.tiles.iter().enumerate() {
        if i >= cols * rows { break; }
        let row = i / cols;
        let col = i % cols;
        let tx = col as f32 * tile_w;
        let tz = -(row as f32) * tile_w;
        let h = tile.height as f32 * 0.25;

        let color = if tile.walkable { [0.0, 1.0, 0.0, 0.6] } else { [1.0, 0.0, 0.0, 0.7] };
        // Tile quad perimeter: 4 corners -> 4 edges
        let corners: [[f32; 3]; 4] = [
            [tx, h + 0.05, tz],
            [tx + tile_w, h + 0.05, tz],
            [tx + tile_w, h + 0.05, tz - tile_w],
            [tx, h + 0.05, tz - tile_w],
        ];
        let line_edges: [(usize, usize); 4] = [(0,1), (1,2), (2,3), (3,0)];
        let base = verts.len() as u32 / 7;
        for &(a, b) in &line_edges {
            for &ci in &[a, b] {
                verts.push(corners[ci][0]);
                verts.push(corners[ci][1]);
                verts.push(corners[ci][2]);
                verts.push(color[0]);
                verts.push(color[1]);
                verts.push(color[2]);
                verts.push(color[3]);
            }
            indices.push(base + (a as u32 * 2));
            indices.push(base + (a as u32 * 2 + 1));
        }
    }

    // Edge walls between tiles at different heights
    for row in 0..rows {
        for col in 0..cols {
            let i = row * cols + col;
            if i >= area.tiles.len() { break; }
            let h = area.tiles[i].height as f32 * 0.25;
            let tx = col as f32 * tile_w;
            let tz = -(row as f32) * tile_w;

            // Right neighbor
            if col + 1 < cols {
                let ri = row * cols + col + 1;
                if ri < area.tiles.len() {
                    let rh = area.tiles[ri].height as f32 * 0.25;
                    let has_wall = wall_set.contains(&(i, 0)) || wall_set.contains(&(ri, 2));
                    if (rh - h).abs() > 0.01 || has_wall {
                        let wall_color = if has_wall { [0.8, 0.3, 0.0, 0.8] } else { [0.0, 0.6, 0.0, 0.5] };
                        let base = verts.len() as u32 / 7;
                        let pts: [[f32; 3]; 4] = [
                            [tx + tile_w, h, tz],
                            [tx + tile_w, rh, tz],
                            [tx + tile_w, rh, tz - tile_w],
                            [tx + tile_w, h, tz - tile_w],
                        ];
                        let wall_edges: [(usize, usize); 4] = [(0,1), (1,2), (2,3), (3,0)];
                        for &(a, b) in &wall_edges {
                            for &ci in &[a, b] {
                                verts.push(pts[ci][0]); verts.push(pts[ci][1]); verts.push(pts[ci][2]);
                                verts.push(wall_color[0]); verts.push(wall_color[1]); verts.push(wall_color[2]); verts.push(wall_color[3]);
                            }
                            indices.push(base + (a as u32 * 2));
                            indices.push(base + (a as u32 * 2 + 1));
                        }
                    }
                }
            }
            // Bottom neighbor
            if row + 1 < rows {
                let bi = (row + 1) * cols + col;
                if bi < area.tiles.len() {
                    let bh = area.tiles[bi].height as f32 * 0.25;
                    let has_wall = wall_set.contains(&(i, 1)) || wall_set.contains(&(bi, 3));
                    if (bh - h).abs() > 0.01 || has_wall {
                        let wall_color = if has_wall { [0.8, 0.3, 0.0, 0.8] } else { [0.0, 0.6, 0.0, 0.5] };
                        let base = verts.len() as u32 / 7;
                        let pts: [[f32; 3]; 4] = [
                            [tx, h, tz - tile_w],
                            [tx, bh, tz - tile_w],
                            [tx + tile_w, bh, tz - tile_w],
                            [tx + tile_w, h, tz - tile_w],
                        ];
                        let wall_edges: [(usize, usize); 4] = [(0,1), (1,2), (2,3), (3,0)];
                        for &(a, b) in &wall_edges {
                            for &ci in &[a, b] {
                                verts.push(pts[ci][0]); verts.push(pts[ci][1]); verts.push(pts[ci][2]);
                                verts.push(wall_color[0]); verts.push(wall_color[1]); verts.push(wall_color[2]); verts.push(wall_color[3]);
                            }
                            indices.push(base + (a as u32 * 2));
                            indices.push(base + (a as u32 * 2 + 1));
                        }
                    }
                }
            }
        }
    }

    (verts, indices)
}

/// Add a box with 6 faces (24 verts, 36 indices)
fn add_box_marker(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>,
    cx: f32, cy: f32, cz: f32, sx: f32, sy: f32, sz: f32, color: [f32; 4],
    transform: impl Fn(f32, f32, f32) -> (f32, f32, f32))
{
    if !cx.is_finite() || !cy.is_finite() || !cz.is_finite() { return; }
    let (rx, ry, rz) = transform(cx, cy, cz);
    // 6 faces, each 4 verts, each face has its own normal
    let faces: [([f32; 3], [f32; 3], [f32; 3], [f32; 3], [f32; 3]); 6] = [
        // front (+Z)
        ([-sx, -sy,  sz], [ sx, -sy,  sz], [ sx,  sy,  sz], [-sx,  sy,  sz], [ 0.0,  0.0,  1.0]),
        // back (-Z)
        ([ sx, -sy, -sz], [-sx, -sy, -sz], [-sx,  sy, -sz], [ sx,  sy, -sz], [ 0.0,  0.0, -1.0]),
        // right (+X)
        ([ sx, -sy,  sz], [ sx, -sy, -sz], [ sx,  sy, -sz], [ sx,  sy,  sz], [ 1.0,  0.0,  0.0]),
        // left (-X)
        ([-sx, -sy, -sz], [-sx, -sy,  sz], [-sx,  sy,  sz], [-sx,  sy, -sz], [-1.0,  0.0,  0.0]),
        // top (+Y)
        ([-sx,  sy,  sz], [ sx,  sy,  sz], [ sx,  sy, -sz], [-sx,  sy, -sz], [ 0.0,  1.0,  0.0]),
        // bottom (-Y)
        ([-sx, -sy, -sz], [ sx, -sy, -sz], [ sx, -sy,  sz], [-sx, -sy,  sz], [ 0.0, -1.0,  0.0]),
    ];
    for (a, b, c, d, n) in &faces {
        let vi = vertices.len() as u32;
        for pt in [a, b, c, d] {
            vertices.push(Vertex {
                position: [rx + pt[0], ry + pt[1], rz + pt[2]],
                normal: *n,
                uv: [0.0, 0.0],
                color,
            });
        }
        indices.extend_from_slice(&[vi, vi + 1, vi + 2, vi, vi + 2, vi + 3]);
    }
}

fn nwn_to_renderer(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    (x, z, -y)
}

fn add_creature_marker(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>,
    x: f32, y: f32, z: f32, _bearing: f32, color: [f32; 4])
{
    add_box_marker(vertices, indices, x, y, z, 0.35, 0.6, 0.3, color, nwn_to_renderer);
    add_box_marker(vertices, indices, x, y, z + 0.45, 0.2, 0.2, 0.2, color, nwn_to_renderer);
    add_box_marker(vertices, indices, x, y, z, 0.45, 0.06, 0.45, color, nwn_to_renderer);
}

fn add_door_marker(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>,
    x: f32, y: f32, z: f32, _bearing: f32, color: [f32; 4])
{
    add_box_marker(vertices, indices, x, y, z, 0.4, 1.0, 0.08, color, nwn_to_renderer);
    add_box_marker(vertices, indices, x - 0.45, y, z, 0.06, 1.1, 0.10, color, nwn_to_renderer);
    add_box_marker(vertices, indices, x + 0.45, y, z, 0.06, 1.1, 0.10, color, nwn_to_renderer);
}

fn add_placeable_marker(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>,
    x: f32, y: f32, z: f32, color: [f32; 4])
{
    add_box_marker(vertices, indices, x, y, z, 0.35, 0.35, 0.35, color, nwn_to_renderer);
}

fn add_trigger_marker(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>,
    x: f32, y: f32, z: f32, color: [f32; 4])
{
    add_box_marker(vertices, indices, x, y, z, 1.0, 0.6, 1.0, color, nwn_to_renderer);
}

fn add_waypoint_marker(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>,
    x: f32, y: f32, z: f32, color: [f32; 4])
{
    // Diamond: octahedron. Center at NWN pos, Z-offset puts it above ground.
    let (rx, ry, rz) = nwn_to_renderer(x, y, z + 0.5);
    let h = 0.5;
    let tip_top = [rx, ry + h, rz];
    let tip_bot = [rx, ry - h, rz];
    let corners = [
        [rx - 0.2, ry, rz - 0.2],
        [rx + 0.2, ry, rz - 0.2],
        [rx + 0.2, ry, rz + 0.2],
        [rx - 0.2, ry, rz + 0.2],
    ];
    let bn = vertices.len() as u32;
    for &c in &corners {
        vertices.push(Vertex { position: c, normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0], color });
    }
    vertices.push(Vertex { position: tip_top, normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], color });
    let tt = bn + 4;
    vertices.push(Vertex { position: tip_bot, normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0], color });
    let tb = bn + 5;
    for i in 0..4 {
        let next = (i + 1) % 4;
        indices.extend_from_slice(&[bn + i, bn + next, tt]);
    }
    for i in 0..4 {
        let next = (i + 1) % 4;
        indices.extend_from_slice(&[bn + next, bn + i, tb]);
    }
}

fn add_encounter_marker(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>,
    x: f32, y: f32, z: f32, color: [f32; 4])
{
    // Hexagon disc with upward arrow
    let (rx, ry, rz) = nwn_to_renderer(x, y, z + 0.3);
    let r = 0.5;
    let h = 0.15;
    let n = 6;
    let base = vertices.len() as u32;
    // Bottom ring
    for i in 0..n {
        let angle = (i as f32 / n as f32) * std::f32::consts::TAU;
        let cx = rx + angle.cos() * r;
        let cz = rz + angle.sin() * r;
        vertices.push(Vertex { position: [cx, ry - h, cz], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], color });
    }
    // Top ring
    for i in 0..n {
        let angle = (i as f32 / n as f32) * std::f32::consts::TAU;
        let cx = rx + angle.cos() * r;
        let cz = rz + angle.sin() * r;
        vertices.push(Vertex { position: [cx, ry + h, cz], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], color });
    }
    // Center top
    vertices.push(Vertex { position: [rx, ry + h + 0.3, rz], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], color });
    let ct = base + n as u32 * 2;
    // Top disk triangles
    for i in 0..n {
        let next = (i + 1) % n;
        let ti = base + n as u32 + i as u32;
        let t_next = base + n as u32 + next as u32;
        indices.extend_from_slice(&[ti, t_next, ct]);
    }
    // Side quads
    for i in 0..n {
        let next = (i + 1) % n;
        let bi = base + i as u32;
        let b_next = base + next as u32;
        let ti = base + n as u32 + i as u32;
        let t_next = base + n as u32 + next as u32;
        indices.extend_from_slice(&[bi, b_next, t_next]);
        indices.extend_from_slice(&[bi, t_next, ti]);
    }
}

fn add_sound_emitter_marker(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>,
    x: f32, y: f32, z: f32, color: [f32; 4])
{
    // Circle with sound ring arcs
    let (rx, ry, rz) = nwn_to_renderer(x, y, z + 0.3);
    let r = 0.4;
    let base = vertices.len() as u32;
    let n = 12;
    // Center
    vertices.push(Vertex { position: [rx, ry, rz], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], color });
    // Outer ring
    for i in 0..n {
        let angle = (i as f32 / n as f32) * std::f32::consts::TAU;
        let cx = rx + angle.cos() * r;
        let cz = rz + angle.sin() * r;
        vertices.push(Vertex { position: [cx, ry, cz], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], color });
    }
    let c = base;
    for i in 0..n {
        let next = (i + 1) % n;
        let vi = base + 1 + i as u32;
        let v_next = base + 1 + next as u32;
        indices.extend_from_slice(&[c, v_next, vi]);
    }
    // Sound wave arcs
    for wave in 0..3 {
        let arc_r = r + 0.15 + wave as f32 * 0.15;
        let arc_n = 8;
        let start_angle = -0.8 + wave as f32 * 0.3;
        let end_angle = 0.8 + wave as f32 * 0.3;
        let abase = vertices.len() as u32;
        for i in 0..=arc_n {
            let t = i as f32 / arc_n as f32;
            let angle = start_angle + (end_angle - start_angle) * t;
            let cx = rx + angle.cos() * arc_r;
            let cz = rz + angle.sin() * arc_r;
            vertices.push(Vertex { position: [cx, ry, cz], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0], color });
        }
        for i in 0..arc_n {
            indices.extend_from_slice(&[abase + i as u32, abase + i as u32 + 1]);
        }
    }
}
