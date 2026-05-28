#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use nwnrs_types::resman::ResContainer;

pub type Result<T> = std::result::Result<T, TilesetError>;

#[derive(Debug, thiserror::Error)]
pub enum TilesetError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("ERF error: {0}")]
    Erf(#[from] nwn_erf::ErfError),
    #[error("Tileset not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone)]
pub struct Tileset {
    pub name: String,
    pub tiles: HashMap<u32, TileDef>,
}

#[derive(Debug, Clone)]
pub struct TileDef {
    pub id: u32,
    pub model_name: String,
}

impl Tileset {
    pub fn from_bytes(name: &str, data: &[u8]) -> Result<Self> {
        let text = std::str::from_utf8(data)
            .map_err(|e| TilesetError::Parse(format!("invalid UTF-8 in .set: {e}")))?;
        let set_file = nwnrs_types::set::parse_set(text)
            .map_err(|e| TilesetError::Parse(e.to_string()))?;

        let mut tiles = HashMap::new();
        for tile in set_file.tiles.values() {
            if let Some(ref model) = tile.model {
                tiles.insert(tile.id, TileDef {
                    id: tile.id,
                    model_name: model.to_lowercase(),
                });
            }
        }

        Ok(Self {
            name: name.to_lowercase(),
            tiles,
        })
    }

    pub fn tile_model(&self, tile_id: u32) -> Option<&str> {
        self.tiles.get(&tile_id).map(|t| t.model_name.as_str())
    }
}

fn game_data_dir() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let data_dir = Path::new(&home)
        .join(".local/share/Steam/steamapps/common/Neverwinter Nights/data");
    if data_dir.is_dir() { Some(data_dir) } else { None }
}

fn load_resource_from_key(resref: &nwnrs_types::resman::ResRef, data_dir: &Path) -> Result<Option<Vec<u8>>> {
    for key_name in &["nwn_base.key", "nwn_retail.key", "nwn_expn.key"] {
        let key_path = data_dir.join(key_name);
        if !key_path.exists() { continue; }
        if let Ok(key_table) = nwnrs_types::key::read_key_table_from_file(&key_path) {
            if key_table.contains(resref) {
                if let Ok(res) = key_table.demand(resref) {
                    if let Ok(data) = res.read_all(nwnrs_types::resman::CachePolicy::Bypass) {
                        return Ok(Some(data));
                    }
                }
            }
        }
    }
    Ok(None)
}

pub fn find_tileset_in_game_data(name: &str) -> Result<Option<Tileset>> {
    let data_dir = match game_data_dir() {
        Some(d) => d,
        None => return Ok(None),
    };
    let set_lower = name.to_lowercase();
    let set_resref = match nwnrs_types::resman::ResRef::new(set_lower, nwnrs_types::resman::ResType(2013)) {
        Ok(rr) => rr,
        Err(_) => return Ok(None),
    };
    match load_resource_from_key(&set_resref, &data_dir)? {
        Some(data) => Tileset::from_bytes(name, &data).map(Some),
        None => Ok(None),
    }
}

#[derive(Clone)]
pub struct ModelMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub uvs: Vec<[f32; 2]>,
    pub texture_name: Option<String>,
    pub name: String,
}

pub fn load_model_mesh(model_name: &str) -> Result<Option<ModelMesh>> {
    let meshes = load_model_meshes(model_name)?;
    match meshes {
        Some(mut ms) if !ms.is_empty() => {
            // Merge all texture groups into a single mesh (keep first texture name)
            let first = ms.swap_remove(0);
            if ms.is_empty() {
                Ok(Some(first))
            } else {
                let mut pos = first.positions;
                let mut norms = first.normals;
                let mut u = first.uvs;
                let mut idx = first.indices;
                let tex_name = first.texture_name;
                for m in ms {
                    let base = pos.len() as u32;
                    pos.extend(m.positions);
                    norms.extend(m.normals);
                    u.extend(m.uvs);
                    for i in m.indices {
                        idx.push(base + i);
                    }
                }
                Ok(Some(ModelMesh { positions: pos, normals: norms, indices: idx, uvs: u, texture_name: tex_name, name: model_name.to_string() }))
            }
        }
        _ => Ok(None),
    }
}

/// Load a model and return one `ModelMesh` per unique texture name.
/// This correctly handles NWN tile models that have different textures
/// for floor, walls, furniture, etc.
pub fn load_model_meshes(model_name: &str) -> Result<Option<Vec<ModelMesh>>> {
    let data_dir = match game_data_dir() {
        Some(d) => d,
        None => return Ok(None),
    };

    let name_lower = model_name.to_lowercase();
    let mdl_resref = match nwnrs_types::resman::ResRef::new(
        name_lower.clone(),
        nwnrs_types::resman::ResType(2002),
    ) {
        Ok(rr) => rr,
        Err(_) => return Ok(None),
    };

    let data = match load_resource_from_key(&mdl_resref, &data_dir)? {
        Some(d) => d,
        None => return Ok(None),
    };

    let scene = match nwnrs_types::mdl::parse_scene_model_auto(&data) {
        Ok(s) => s,
        Err(e) => return Err(TilesetError::Parse(format!("Scene parse error for '{model_name}': {e}"))),
    };
    let mut baked = match nwnrs_types::mdl::bake_scene_pose(&scene) {
        Ok(s) => s,
        Err(e) => return Err(TilesetError::Parse(format!("Bake error for '{model_name}': {e}"))),
    };

    // Apply node world transforms
    {
        let mut world = vec![[[0.0f32; 4]; 4]; baked.nodes.len()];
        for (i, node) in baked.nodes.iter().enumerate() {
            let local = nwn_transform_to_mat4(&node.local_transform);
            world[i] = match node.parent {
                Some(p) => mat4_mul(&world[p], &local),
                None => local,
            };
        }
        for mesh in &mut baked.meshes {
            let m = world[mesh.source_node];
            if mat4_is_identity(&m) { continue; }
            for prim in &mut mesh.primitives {
                for pos in &mut prim.positions {
                    *pos = mat4_transform(&m, pos);
                }
                for n in &mut prim.normals {
                    *n = mat4_transform_vector(&m, n);
                }
            }
        }
    }

    // Group geometry by texture name
    struct TexGroup {
        positions: Vec<[f32; 3]>,
        normals: Vec<[f32; 3]>,
        uvs: Vec<[f32; 2]>,
        indices: Vec<u32>,
        texture_name: Option<String>,
    }

    let mut groups: Vec<TexGroup> = Vec::new();

    for mesh in &baked.meshes {
        // Skip AABB walkmesh/collision geometry
        if let Some(node) = baked.nodes.get(mesh.source_node) {
            use nwnrs_types::mdl::NodeKind;
            if let NodeKind::Aabb = node.kind {
                continue;
            }
        }
        for prim in &mesh.primitives {
            // Determine texture name for this primitive
            let tex_name = prim.texture_names.first().cloned().or_else(|| {
                prim.material.and_then(|mi| {
                    baked.materials.get(mi)
                        .and_then(|mat| mat.textures.first().map(|t| t.name.clone()))
                })
            });

            // Find or create group for this texture
            let gidx = groups.iter().position(|g| g.texture_name == tex_name);
            let gidx = gidx.unwrap_or_else(|| {
                groups.push(TexGroup {
                    positions: Vec::new(),
                    normals: Vec::new(),
                    uvs: Vec::new(),
                    indices: Vec::new(),
                    texture_name: tex_name,
                });
                groups.len() - 1
            });
            let group = &mut groups[gidx];

            // Load UV set (flip V: OpenGL bottom-left -> wgpu top-left)
            let prim_uvs: Vec<[f32; 2]> = if let Some(uv_set) = prim.uv_sets.first() {
                uv_set.coordinates.iter()
                    .map(|uv| [uv[0], 1.0 - uv[1]])
                    .collect()
            } else {
                vec![[0.0, 0.0]; prim.positions.len()]
            };
            let use_planar = prim_uvs.len() == prim.positions.len() && uvs_are_unreasonable(&prim_uvs);
            let planar_uvs = if use_planar && !prim.positions.is_empty() {
                let min_x = prim.positions.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
                let max_x = prim.positions.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
                let min_z = prim.positions.iter().map(|p| p[2]).fold(f32::MAX, f32::min);
                let max_z = prim.positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
                let range_x = (max_x - min_x).max(0.001);
                let range_z = (max_z - min_z).max(0.001);
                prim.positions.iter().map(|p| {
                    [1.0 - (p[0] - min_x) / range_x, 1.0 - (p[2] - min_z) / range_z]
                }).collect::<Vec<_>>()
            } else {
                Vec::new()
            };

            for face in &prim.faces {
                for j in 0..3 {
                    let vi = face.vertex_indices[j] as usize;
                    let uvi = face.uv_indices[j] as usize;

                    let pos = if vi < prim.positions.len() { &prim.positions[vi] } else { &prim.positions[0] };
                    let p = [pos[0], pos[2], -pos[1]];
                    group.positions.push(p);

                    if vi < prim.normals.len() {
                        let n = &prim.normals[vi];
                        group.normals.push([n[0], n[2], -n[1]]);
                    } else {
                        group.normals.push([0.0, 1.0, 0.0]);
                    }

                    let uv = if use_planar {
                        if uvi < planar_uvs.len() { planar_uvs[uvi] } else { [0.0, 0.0] }
                    } else {
                        if uvi < prim_uvs.len() { prim_uvs[uvi] } else { [0.0, 0.0] }
                    };
                    group.uvs.push(uv);

                    group.indices.push(group.positions.len() as u32 - 1);
                }
            }
        }
    }

    // Convert groups to ModelMesh with auto-generated normals
    let result: Vec<ModelMesh> = groups.into_iter().filter(|g| !g.positions.is_empty()).map(|g| {
        let normals = compute_normals(&g.positions, &g.indices);
        ModelMesh {
            positions: g.positions,
            normals,
            indices: g.indices,
            uvs: g.uvs,
            texture_name: g.texture_name,
            name: model_name.to_string(),
        }
    }).collect();

    if result.is_empty() {
        Ok(None)
    } else {
        Ok(Some(result))
    }
}

fn uvs_are_unreasonable(uvs: &[[f32; 2]]) -> bool {
    if uvs.is_empty() { return true; }
    let u_min = uvs.iter().map(|u| u[0]).fold(f32::MAX, f32::min);
    let u_max = uvs.iter().map(|u| u[0]).fold(f32::MIN, f32::max);
    let v_min = uvs.iter().map(|u| u[1]).fold(f32::MAX, f32::min);
    let v_max = uvs.iter().map(|u| u[1]).fold(f32::MIN, f32::max);
    let u_range = (u_max - u_min).abs();
    let v_range = (v_max - v_min).abs();
    u_range > 10.0 || v_range > 10.0 || u_max.abs() > 100.0 || v_max.abs() > 100.0
}

#[allow(dead_code)]
fn compute_planar_uvs(positions: &[[f32; 3]]) -> Vec<[f32; 2]> {
    let min_x = positions.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
    let max_x = positions.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
    let min_z = positions.iter().map(|p| p[2]).fold(f32::MAX, f32::min);
    let max_z = positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
    let range_x = (max_x - min_x).max(0.001);
    let range_z = (max_z - min_z).max(0.001);
    positions.iter().map(|p| {
        [(p[0] - min_x) / range_x, (p[2] - min_z) / range_z]
    }).collect()
}

fn compute_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0f32; 3]; positions.len()];
    for tri in indices.chunks(3) {
        if tri.len() < 3 { continue; }
        let a = positions[tri[0] as usize];
        let b = positions[tri[1] as usize];
        let c = positions[tri[2] as usize];
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let nx = u[1] * v[2] - u[2] * v[1];
        let ny = u[2] * v[0] - u[0] * v[2];
        let nz = u[0] * v[1] - u[1] * v[0];
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        if len > 0.0 {
            for vi in tri.iter().take(3) {
                normals[*vi as usize][0] += nx / len;
                normals[*vi as usize][1] += ny / len;
                normals[*vi as usize][2] += nz / len;
            }
        }
    }
    for n in &mut normals {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 0.0 {
            n[0] /= len; n[1] /= len; n[2] /= len;
        }
    }
    normals
}

pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn load_texture_data(texture_name: &str) -> Result<Option<TextureData>> {
    let data_dir = match game_data_dir() {
        Some(d) => d,
        None => return Ok(None),
    };
    let name_lower = texture_name.to_lowercase()
        .replace(".dds", "").replace(".tga", "")
        .replace(" ", "_").replace("-", "_");

    // Try DDS (type 8), then TGA (type 3)
    for res_type in &[8u16, 3u16] {
        let resref = match nwnrs_types::resman::ResRef::new(
            name_lower.clone(),
            nwnrs_types::resman::ResType(*res_type),
        ) {
            Ok(rr) => rr,
            Err(_) => continue,
        };
        if let Some(data) = load_resource_from_key(&resref, &data_dir)? {
            if *res_type == 8 {
                if let Ok(dds) = nwnrs_types::dds::DdsTexture::read_from_texture_bytes(&data) {
                    if let Ok(rgba) = dds.decode_mip_rgba8(0) {
                        return Ok(Some(TextureData {
                            width: dds.width,
                            height: dds.height,
                            rgba,
                        }));
                    }
                }
            } else if *res_type == 3 {
                if let Ok(tga) = nwnrs_types::tga::read_tga(&mut Cursor::new(&data)) {
                    if let Ok(rgba) = tga.decode_rgba8() {
                        return Ok(Some(TextureData {
                            width: tga.width as u32,
                            height: tga.height as u32,
                            rgba,
                        }));
                    }
                }
            }
        }
    }

    Ok(None)
}

pub fn find_and_load_tileset(name: &str, module_path: &Path, haks: &[String]) -> Result<Option<Tileset>> {
    let mod_dir = module_path.parent().unwrap_or(Path::new("."));
    let set_lower = name.to_lowercase();

    fn try_erf(erf: &nwn_erf::ErfFile, set_lower: &str, name: &str) -> Result<Option<Tileset>> {
        let set_type = nwn_lib_rs::erf::ResourceType::Set as u16;
        if let Some(data) = erf.get(set_lower, set_type) {
            return Tileset::from_bytes(name, &data).map(Some);
        }
        Ok(None)
    }

    // First try the module itself
    if let Ok(erf) = nwn_erf::ErfFile::open(module_path) {
        if let Some(ts) = try_erf(&erf, &set_lower, name)? {
            return Ok(Some(ts));
        }
    }

    // Try haks in module dir
    for hak_name in haks {
        let paths = [
            mod_dir.join(format!("{}.hak", hak_name)),
            mod_dir.join("hak").join(format!("{}.hak", hak_name)),
        ];
        for p in &paths {
            if let Ok(erf) = nwn_erf::ErfFile::open(p) {
                if let Some(ts) = try_erf(&erf, &set_lower, name)? {
                    return Ok(Some(ts));
                }
            }
        }
    }

    // Also check ~/Documents/Neverwinter Nights/hak/
    if let Ok(home) = std::env::var("HOME") {
        let hak_dir = Path::new(&home).join("Documents/Neverwinter Nights/hak");
        if hak_dir.is_dir() {
            for hak_name in haks {
                let hak_path = hak_dir.join(format!("{}.hak", hak_name));
                if let Ok(erf) = nwn_erf::ErfFile::open(&hak_path) {
                    if let Some(ts) = try_erf(&erf, &set_lower, name)? {
                        return Ok(Some(ts));
                    }
                }
            }
        }
    }

    // Finally, try NWN game data (KEY/BIF files)
    find_tileset_in_game_data(name)
}

/// Build a 4x4 column-major matrix from an NWN node transform (Z-up, axis-angle rotation).
fn nwn_transform_to_mat4(t: &nwnrs_types::mdl::NwnTransform) -> [[f32; 4]; 4] {
    let t_mat: [[f32; 4]; 4] = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [t.translation[0], t.translation[1], t.translation[2], 1.0],
    ];
    let r_mat = if t.rotation_axis_angle[3].abs() <= f32::EPSILON {
        IDENTITY_MAT4
    } else {
        let [ax, ay, az] = [t.rotation_axis_angle[0], t.rotation_axis_angle[1], t.rotation_axis_angle[2]];
        let len = (ax*ax + ay*ay + az*az).sqrt();
        if len <= f32::EPSILON {
            IDENTITY_MAT4
        } else {
            let (x, y, z) = (ax / len, ay / len, az / len);
            let (c, s) = (t.rotation_axis_angle[3].cos(), t.rotation_axis_angle[3].sin());
            let omc = 1.0 - c;
            [
                [c + x*x*omc, y*x*omc + z*s, z*x*omc - y*s, 0.0],
                [x*y*omc - z*s, c + y*y*omc, z*y*omc + x*s, 0.0],
                [x*z*omc + y*s, y*z*omc - x*s, c + z*z*omc, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ]
        }
    };
    let s_mat: [[f32; 4]; 4] = [
        [t.scale[0], 0.0, 0.0, 0.0],
        [0.0, t.scale[1], 0.0, 0.0],
        [0.0, 0.0, t.scale[2], 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    mat4_mul(&mat4_mul(&t_mat, &r_mat), &s_mat)
}

const IDENTITY_MAT4: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// Column-major 4x4 matrix multiply: a * b.
/// Each matrix is stored as [col0, col1, col2, col3] where col[i][j] = row j of column i.
fn mat4_mul(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut r = [[0.0f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            for k in 0..4 {
                r[col][row] += a[k][row] * b[col][k];
            }
        }
    }
    r
}

/// Transform a 3D point by a 4x4 column-major matrix (treats w=1).
fn mat4_transform(m: &[[f32; 4]; 4], p: &[f32; 3]) -> [f32; 3] {
    [
        m[0][0]*p[0] + m[1][0]*p[1] + m[2][0]*p[2] + m[3][0],
        m[0][1]*p[0] + m[1][1]*p[1] + m[2][1]*p[2] + m[3][1],
        m[0][2]*p[0] + m[1][2]*p[1] + m[2][2]*p[2] + m[3][2],
    ]
}

/// Transform a 3D vector by a 4x4 column-major matrix (treats w=0, no translation).
fn mat4_transform_vector(m: &[[f32; 4]; 4], v: &[f32; 3]) -> [f32; 3] {
    [
        m[0][0]*v[0] + m[1][0]*v[1] + m[2][0]*v[2],
        m[0][1]*v[0] + m[1][1]*v[1] + m[2][1]*v[2],
        m[0][2]*v[0] + m[1][2]*v[1] + m[2][2]*v[2],
    ]
}

fn mat4_is_identity(m: &[[f32; 4]; 4]) -> bool {
    m[0][0] == 1.0 && m[0][1] == 0.0 && m[0][2] == 0.0 && m[0][3] == 0.0 &&
    m[1][0] == 0.0 && m[1][1] == 1.0 && m[1][2] == 0.0 && m[1][3] == 0.0 &&
    m[2][0] == 0.0 && m[2][1] == 0.0 && m[2][2] == 1.0 && m[2][3] == 0.0 &&
    m[3][0] == 0.0 && m[3][1] == 0.0 && m[3][2] == 0.0 && m[3][3] == 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Vec<String> {
        let ts = find_tileset_in_game_data("tin01").unwrap().unwrap();
        let mut names: Vec<String> = ts.tiles.values().map(|t| t.model_name.clone()).collect();
        names.sort();
        names.dedup();
        names
    }

    #[test]
    fn test_uvs_match_positions() {
        for name in setup().iter().take(15) {
            if let Ok(Some(mesh)) = load_model_mesh(name) {
                assert_eq!(mesh.uvs.len(), mesh.positions.len(),
                    "UV count mismatch for {name}");
                let u_min = mesh.uvs.iter().map(|u| u[0]).fold(f32::MAX, f32::min);
                let u_max = mesh.uvs.iter().map(|u| u[0]).fold(f32::MIN, f32::max);
                let v_min = mesh.uvs.iter().map(|u| u[1]).fold(f32::MAX, f32::min);
                let v_max = mesh.uvs.iter().map(|u| u[1]).fold(f32::MIN, f32::max);
                let sample: Vec<[f32;2]> = mesh.uvs.iter().take(5).copied().collect();
                println!("{name}: UV ranges U[{u_min:.3}..{u_max:.3}] V[{v_min:.3}..{v_max:.3}] samples={sample:?}");
            }
        }
    }

    #[test]
    fn test_texture_name_present() {
        let found = setup().iter().take(15).any(|name| {
            load_model_mesh(name).ok().flatten().and_then(|m| m.texture_name).is_some()
        });
        assert!(found, "Expected at least one model to have a texture name");
    }

    #[test]
    fn test_tga_texture_loads() {
        for name in setup().iter().take(10) {
            if let Ok(Some(mesh)) = load_model_mesh(name) {
                if let Some(ref tn) = mesh.texture_name {
                    if let Ok(Some(data)) = load_texture_data(tn) {
                        assert!(data.width > 0 && data.height > 0, "Zero-size texture: {tn}");
                        assert!(!data.rgba.is_empty(), "Empty texture data: {tn}");
                        return;
                    }
                }
            }
        }
        panic!("No loadable texture found in first 10 models");
    }
}
