#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

#[test]
fn load_mdl_test() {
    // First load the tileset to find model names
    match nwn_tileset::find_tileset_in_game_data("tin01") {
        Ok(Some(ts)) => {
            println!("tin01 tileset loaded: {} tile defs", ts.tiles.len());
            // Print first 15 model names
            let mut names: Vec<&str> = ts.tiles.values().map(|t| t.model_name.as_str()).collect();
            names.sort();
            names.dedup();
            println!("Unique models: {}", names.len());
            for n in names.iter().take(15) {
                println!("  model: {}", n);
            }
            // Try loading the first model
            if let Some(first_name) = names.first() {
                match nwn_tileset::load_model_mesh(first_name) {
                    Ok(Some(mesh)) => {
                        println!("  -> {}: {} verts, {} tris", first_name, mesh.positions.len(), mesh.indices.len() / 3);
                        if !mesh.positions.is_empty() {
                            let min_x = mesh.positions.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
                            let max_x = mesh.positions.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
                            let min_y = mesh.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
                            let max_y = mesh.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
                            let min_z = mesh.positions.iter().map(|p| p[2]).fold(f32::MAX, f32::min);
                            let max_z = mesh.positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
                            println!("  -> pos X[{:.3}..{:.3}] Y[{:.3}..{:.3}] Z[{:.3}..{:.3}]", min_x, max_x, min_y, max_y, min_z, max_z);
                        }
                        println!("  -> tex: {:?}", mesh.texture_name);
                    }
                    Ok(None) => {
                        println!("  -> {}: not found in KEY/BIF", first_name);
                    }
                    Err(e) => {
                        println!("  -> {}: error: {}", first_name, e);
                    }
                }
            }
        }
        Ok(None) => println!("tin01 tileset not found"),
        Err(e) => println!("Error loading tin01 tileset: {}", e),
    }
}
