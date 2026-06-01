use nwnrs_types::resman::{CachePolicy, ResContainer};
use nwnrs_types::resman::ResRef;
use nwnrs_types::resman::ResType;

fn main() {
    let home = std::env::var("HOME").unwrap();
    let data_dir = std::path::Path::new(&home)
        .join(".steam/steam/steamapps/common/Neverwinter Nights/data");
    let key_file = data_dir.join("nwn_base.key");
    let key_table = nwnrs_types::key::read_key_table_from_file(key_file.to_string_lossy().as_ref()).unwrap();

    let mut body_parts: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let all_resources = key_table.list();

    // Search for body part models: p_<letter><number> or similar
    for res in &all_resources {
        let name = res.0.as_str();
        let rtype = res.1.0;
        if rtype == 2002 { // MDL
            // NWN body parts pattern: p_d0, p_d1, etc. or p_d_torso, etc.
            if name.len() >= 3 && name.as_bytes()[0] == b'p' && name.as_bytes()[1] == b'_' {
                body_parts.insert(name.to_string());
            }
            // Also check for p<letter><digit> pattern
            if name.len() >= 3 && name.as_bytes()[0] == b'p' && name.as_bytes()[1].is_ascii_lowercase() && name.as_bytes()[2].is_ascii_digit() {
                body_parts.insert(name.to_string());
            }
        }
    }

    println!("Found {} potential body part models:", body_parts.len());
    for name in &body_parts {
        let rr = ResRef::new(name.clone(), ResType(2002)).unwrap();
        if let Ok(d) = key_table.demand(&rr) {
            if let Ok(data) = d.read_all(CachePolicy::Bypass) {
                if let Ok(scene) = nwnrs_types::mdl::parse_scene_model_auto(&data) {
                    let has_geom = scene.mesh_placements.iter().any(|m| !m.positions.is_empty());
                    let mesh_count = scene.mesh_placements.iter().filter(|m| !m.positions.is_empty()).count();
                    println!("  {}: {} nodes, {} with geom ({} bytes)", name, scene.nodes.len(), mesh_count, data.len());
                } else {
                    println!("  {}: parse error", name);
                }
            }
        }
    }
}
