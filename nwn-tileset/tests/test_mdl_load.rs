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
