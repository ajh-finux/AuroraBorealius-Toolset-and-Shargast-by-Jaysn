/// Decode a PLT texture from game data using the same pipeline as the toolset,
/// write the raw RGBA to a file for inspection.
use std::path::PathBuf;

fn find_nwn_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap();
    let candidates = [
        PathBuf::from(&home).join(".local/share/Steam/steamapps/common/Neverwinter Nights"),
        PathBuf::from(&home).join(".steam/steam/steamapps/common/Neverwinter Nights"),
    ];
    for c in &candidates {
        if c.join("data").join("nwn_base.key").exists() {
            return c;
        }
    }
    panic!("NWN not found — set NWN_ROOT or check install path");
}

fn main() {
    let resref = std::env::args().nth(1).unwrap_or_else(|| "pmh0_chest001".into());
    let out_path = std::env::args().nth(2).unwrap_or_else(|| "/tmp/plt_decoded.rgba".into());

    let root = find_nwn_root();
    eprintln!("NWN root: {:?}", root);

    let data_dir = root.join("data");

    // Initialize GameData (needed by load_texture_data)
    let _game_data = nwn_gamemodel::GameData::new(root, data_dir.clone())
        .expect("GameData::new");

    // Layer color indices: [skin, hair, metal, metal2, cloth, cloth2, leather, leather2, tattoo, tattoo2]
    // Using same default as toolset appearance picker
    let layers: [u8; 10] = [2, 1, 5, 5, 40, 40, 20, 20, 0, 0];

    // Build PLT spec from the layer indices (same as toolset)
    let spec = nwn_tileset::build_plt_spec_from_layers(&layers);
    let spec_ref = Some(&spec);

    // Load the PLT resource from game data
    let resref_lower = resref.to_lowercase();
    let resource = nwn_gamemodel::get_resource_data(&resref_lower, 6u16)
        .expect("get_resource_data");

    let tex_data = match resource {
        Some(data) => {
            nwn_tileset::decode_texture_data(6, &data, spec_ref)
        }
        None => {
            eprintln!("Resource '{}' type 6 not found", resref);
            return;
        }
    };

    let tex = match tex_data {
        Some(t) => t,
        None => {
            eprintln!("Failed to decode PLT texture '{}'", resref);
            return;
        }
    };

    eprintln!("PLT DECODE: {} {}x{} pixels={}", resref, tex.width, tex.height, tex.rgba.len() / 4);

    // Stats
    if !tex.rgba.is_empty() {
        let chunks: Vec<&[u8]> = tex.rgba.chunks_exact(4).collect();
        let n = chunks.len() as u64;
        let r_sum: u64 = chunks.iter().map(|p| p[0] as u64).sum();
        let g_sum: u64 = chunks.iter().map(|p| p[1] as u64).sum();
        let b_sum: u64 = chunks.iter().map(|p| p[2] as u64).sum();
        let first = chunks[0];
        eprintln!("  avg_rgb=({},{},{}) first_pixel=({},{},{},{})",
            r_sum / n, g_sum / n, b_sum / n,
            first[0], first[1], first[2], first[3]);
    }

    std::fs::write(out_path, &tex.rgba).expect("write RGBA");
    eprintln!("Wrote {} bytes to {}", tex.rgba.len(), out_path);

    // Print palette colors used
    eprintln!("  Layer colors (from build_plt_spec_from_layers):");
    for (i, color) in spec.layer_colors.iter().enumerate() {
        if i < 10 {
            eprintln!("    layer[{}] = ({},{},{},{})", i, color[0], color[1], color[2], color[3]);
        }
    }
}
