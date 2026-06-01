fn main() {
    let pool = [
        "c_gnoll", "c_goblinx", "c_basilisk", "c_cat_mpanth",
        "c_direrat", "c_rat", "c_spidgiant", "c_wagn2",
        "c_wagn3", "c_behold", "c_jelly", "c_wingbat",
        "c_wingbut", "c_wyrmlblk", "c_drider", "c_manticore",
    ];

    let data_dir = nwn_tileset::game_data_dir();
    match &data_dir {
        Some(d) => eprintln!("Game data dir: {:?}", d),
        None => {
            eprintln!("Game data dir: NOT FOUND");
            eprintln!("Expected Steam path: ~/.local/share/Steam/steamapps/common/Neverwinter Nights/data");
        }
    }

    for &name in &pool {
        eprint!("  {} ... ", name);

        // Check supermodel from raw MDL data
        let data_dir = match nwn_tileset::game_data_dir() {
            Some(d) => d,
            None => {
                eprintln!("NO_DATA_DIR");
                continue;
            }
        };
        let supermodel = (|| -> Option<String> {
            for restype in &[2002u16, 2001u16] {
                let rr = nwnrs_types::resman::ResRef::new(
                    name.to_string(),
                    nwnrs_types::resman::ResType(*restype),
                ).ok()?;
                let data = nwn_tileset::load_resource_from_key(&rr, &data_dir).ok()??;
                let scene = nwnrs_types::mdl::parse_scene_model_auto(&data).ok()?;
                let sup = scene.supermodel?;
                if sup != "NULL" && sup.to_lowercase() != name.to_lowercase() {
                    return Some(sup);
                }
            }
            None
        })();

        // Load geometry
        match nwn_tileset::load_model_meshes(name) {
            Ok(Some(meshes)) => {
                let total_verts: usize = meshes.iter().map(|m| m.positions.len()).sum();
                let total_tris: usize = meshes.iter().map(|m| m.indices.len() / 3).sum();
                if let Some(ref sup) = supermodel {
                    eprintln!("OK  meshes={} verts={} tris={}  supermodel='{}'", meshes.len(), total_verts, total_tris, sup);
                } else {
                    eprintln!("OK  meshes={} verts={} tris={}", meshes.len(), total_verts, total_tris);
                }
                for (i, m) in meshes.iter().enumerate() {
                    let tex = m.texture_name.as_deref().unwrap_or("(none)");
                    eprintln!("         mesh[{}]: '{}' {} verts {} tris", i, tex, m.positions.len(), m.indices.len() / 3);
                }
            }
            Ok(None) => {
                if let Some(ref sup) = supermodel {
                    eprintln!("no_geom  supermodel='{}' (no meshes)", sup);
                } else {
                    eprintln!("MISS (no data in game)");
                }
            }
            Err(e) => {
                if let Some(ref sup) = supermodel {
                    eprintln!("ERR={}  supermodel='{}'", e, sup);
                } else {
                    eprintln!("ERR={}", e);
                }
            }
        }
    }
}
