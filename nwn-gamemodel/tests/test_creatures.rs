#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

/// Fast test that reads creature appearances directly from raw GIT data
/// without loading tilesets or preparing full area structures.
#[test]
fn inspect_raw_creature_appearances() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/grimshackle".to_string());
    let path = format!("{}/Documents/Neverwinter Nights/modules/HOPE_Shargast_CH1.mod", home);
    let module = nwn_gamemodel::Module::open(&path).expect("Failed to open module");
    println!("Module: {} ({} areas)", module.name, module.num_areas());

    let mut appearances: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();
    let area_names = module.area_names();

    for name in area_names {
        if let Some(raw_git) = module.raw_git.get(name) {
            if let Ok(gff) = nwn_gff::GffFile::from_bytes(raw_git) {
                if let Some(v) = gff.root().find("Creature List") {
                    if let nwn_gff::FieldValue::List(creature_list) = v {
                        use nwn_gff::FieldValue;
                        for creature in creature_list.iter() {
                            let app = creature.find("Appearance")
                                .and_then(|v| if let FieldValue::Int(x) = v { Some(*x) } else { None })
                                .unwrap_or(-1);
                            *appearances.entry(app).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
    }

    println!("Total areas: {}", module.num_areas());
    let mut sorted: Vec<_> = appearances.into_iter().collect();
    sorted.sort_by_key(|k| k.0);
    eprintln!("Creature appearance distribution:");
    for (app, count) in &sorted {
        eprintln!("  appearance {}: {} creatures", app, count);
    }
}
