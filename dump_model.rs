fn main() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/grimshackle".to_string());
    let data_dir = std::path::Path::new(&home)
        .join(".steam/steam/steamapps/common/Neverwinter Nights/data");
    let key_file = data_dir.join("nwn_base.key");

    let (_, key_table) = nwnrs_types::key::read_key_table_from_file(&key_file.to_string_lossy()).unwrap();
    for name in &["pmd0", "pma0", "pme0", "pmg0", "pmh0", "pmo0",
                  "pfd0", "pfa0", "pfe0", "pfg0", "pfh0", "pfo0"] {
        let rr = nwnrs_types::resman::ResRef::new(name.to_string(), nwnrs_types::resman::ResType(2002)).unwrap();
        if let Some(data) = key_table.demand(&rr).ok().and_then(|d| d.read_all().ok()) {
            println!("{}: {} bytes found", name, data.len());
            match nwnrs_types::mdl::parse_scene_model_auto(&data) {
                Ok(scene) => {
                    println!("  {} nodes:", scene.nodes.len());
                    for (i, node) in scene.nodes.iter().enumerate() {
                        let part = node.part_number.map(|p| p.to_string()).unwrap_or_default();
                        println!("  [{}] name={} type={} part={} parent={:?}", i, node.name, node.node_type, part, node.parent);
                    }
                }
                Err(e) => println!("  parse error: {}", e),
            }
        } else {
            println!("{}: NOT FOUND", name);
        }
    }
}
