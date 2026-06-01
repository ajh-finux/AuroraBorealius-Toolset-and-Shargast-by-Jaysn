/// CLI tool: decode a PLT texture from game data and dump raw RGBA.
/// Usage: cargo run --example decode_plt -- <resref> [out.rgba]
use std::path::{Path, PathBuf};

fn find_nwn_root() -> PathBuf {
    if let Ok(nwn) = std::env::var("NWN") {
        return PathBuf::from(nwn);
    }
    let home = std::env::var("HOME").unwrap();
    for c in &[
        format!("{}/.steam/steam/steamapps/common/Neverwinter Nights", home),
        format!("{}/.local/share/Steam/steamapps/common/Neverwinter Nights", home),
    ] {
        let p = Path::new(c);
        if p.join("data").join("nwn_base.key").exists() { return p.to_path_buf(); }
    }
    panic!("NWN not found. Set NWN_ROOT env var or check install.");
}

fn main() {
    let resref = std::env::args().nth(1).unwrap_or_else(|| "pmh0_chest001".into());
    let out_path = std::env::args().nth(2).unwrap_or_else(|| format!("/tmp/plt_{}.rgba", resref));

    let root = find_nwn_root();
    eprintln!("NWN root: {:?}", root);

    // Initialize GameData singleton
    let gd = nwn_gamemodel::game_data::GameData::new().expect("GameData::new");

    // Load PLT resource (type 6)
    let data = gd.get_resource(&resref, 6u16).expect("get_resource failed");
    eprintln!("Loaded {} bytes for '{}' type=6", data.len(), resref);

    // Decode with DEFAULT spec (no layer colors)
    let tex_default = nwn_tileset::decode_texture_data(6, &data, None).expect("decode PLT (default)");
    eprintln!("PLT (default spec): {}x{} pixels={}",
        tex_default.width, tex_default.height, tex_default.rgba.len() / 4);
    dump_stats(&tex_default.rgba);

    // Decode with custom layer colors (same as toolset)
    let layers: [u8; 10] = [2, 1, 5, 5, 40, 40, 20, 20, 0, 0];
    let spec = nwn_tileset::build_plt_spec_from_layers(&layers);
    let tex_spec = nwn_tileset::decode_texture_data(6, &data, Some(&spec)).expect("decode PLT (spec)");
    eprintln!("PLT (custom spec): {}x{} pixels={}",
        tex_spec.width, tex_spec.height, tex_spec.rgba.len() / 4);
    dump_stats(&tex_spec.rgba);

    // Write output
    std::fs::write(&out_path, &tex_spec.rgba).expect("write RGBA");
    eprintln!("Wrote {} to {}", tex_spec.rgba.len(), out_path);
}

fn dump_stats(rgba: &[u8]) {
    if rgba.len() < 4 { return; }
    let n = rgba.len() / 4;
    let r_sum: u64 = rgba.chunks_exact(4).map(|p| p[0] as u64).sum();
    let g_sum: u64 = rgba.chunks_exact(4).map(|p| p[1] as u64).sum();
    let b_sum: u64 = rgba.chunks_exact(4).map(|p| p[2] as u64).sum();
    let first = &rgba[..4];
    eprintln!("  avg_rgb=({},{},{}) first=({},{},{},{})",
        r_sum / n as u64, g_sum / n as u64, b_sum / n as u64,
        first[0], first[1], first[2], first[3]);
}
