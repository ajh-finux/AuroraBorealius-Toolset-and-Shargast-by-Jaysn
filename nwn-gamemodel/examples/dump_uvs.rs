/// Dump model info: UVs, texture names, vertex colors for creature body parts
use std::path::PathBuf;

fn main() {
    let model_name = std::env::args().nth(1).unwrap_or_else(|| "pmh0_chest001".into());

    // Initialize game data
    let _gd = nwn_gamemodel::game_data::GameData::new().expect("GameData::new");

    let meshes = nwn_tileset::load_model_meshes(&model_name)
        .expect("load_model_meshes")
        .unwrap_or_else(|| panic!("model '{model_name}' not found"));

    eprintln!("Model: {} — {} mesh groups", model_name, meshes.len());

    for (i, m) in meshes.iter().enumerate() {
        eprintln!("  mesh[{}]: tex={:?} verts={} indices={} uvs={} colors={}",
            i, m.texture_name, m.positions.len(), m.indices.len(),
            m.uvs.len(), m.colors.len());

        // Check UVs
        if !m.uvs.is_empty() {
            let unique_uvs: std::collections::BTreeSet<[u32; 2]> = m.uvs.iter().map(|uv| {
                [(uv[0] * 1000.0) as u32, (uv[1] * 1000.0) as u32]
            }).collect();
            eprintln!("    unique UVs (quantized): {}", unique_uvs.len());
            let min_u = m.uvs.iter().map(|uv| uv[0]).fold(f32::MAX, f32::min);
            let max_u = m.uvs.iter().map(|uv| uv[0]).fold(f32::MIN, f32::max);
            let min_v = m.uvs.iter().map(|uv| uv[1]).fold(f32::MAX, f32::min);
            let max_v = m.uvs.iter().map(|uv| uv[1]).fold(f32::MIN, f32::max);
            eprintln!("    UV range: u=[{:.3},{:.3}] v=[{:.3},{:.3}]", min_u, max_u, min_v, max_v);
            if unique_uvs.len() <= 5 {
                for uv in &m.uvs {
                    eprintln!("      uv=({:.3},{:.3})", uv[0], uv[1]);
                }
            }
        }

        // Check colors
        if !m.colors.is_empty() {
            let unique_colors: std::collections::BTreeSet<[u32; 4]> = m.colors.iter().map(|c| {
                [(c[0] * 255.0) as u32, (c[1] * 255.0) as u32, (c[2] * 255.0) as u32, (c[3] * 255.0) as u32]
            }).collect();
            eprintln!("    unique vertex colors: {}", unique_colors.len());
            if unique_colors.len() <= 8 {
                for c in unique_colors {
                    eprintln!("      color=({},{},{},{})", c[0], c[1], c[2], c[3]);
                }
            }
        }
    }
}
