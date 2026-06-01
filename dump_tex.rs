fn main() {
    if let Some(meshes) = nwn_tileset::load_model_meshes("pmo0").unwrap() {
        eprintln!("pmo0: {} meshes", meshes.len());
        for (i, m) in meshes.iter().enumerate() {
            eprintln!("  mesh[{}]: tex={:?} verts={} idx={}",
                i, m.texture_name, m.positions.len(), m.indices.len());
        }
    }
}
