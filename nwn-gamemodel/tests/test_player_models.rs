#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

#[test]
fn test_all_player_models_resolve() {
    for name in &["pmd0", "pma0", "pme0", "pmg0", "pmh0", "pmo0",
                  "pfd0", "pfa0", "pfe0", "pfg0", "pfh0", "pfo0"] {
        let result = nwn_tileset::load_model_meshes(name).unwrap();
        assert!(result.is_some(), "{name} should resolve to meshes via supermodel");
        assert!(!result.unwrap().is_empty(), "{name} should have non-empty meshes");
    }
}

#[test]
fn test_model_cache() {
    let r1 = nwn_tileset::load_model_meshes("pmd0").unwrap();
    let r2 = nwn_tileset::load_model_meshes("pmd0").unwrap();
    assert!(r1.is_some() && r2.is_some());
}

#[test]
fn test_player_model_textures_load() {
    for &name in &[
        "pmh0_chest001", "pmh0_belt003", "pmo0_head005", "pme0_chest001",
        "pma0_gloves001", "pmd0_boot001", "pmg0_belt003", "pmh0_gloves001",
        "pmo0_chest001", "pmd0_chest001", "pme0_belt003", "pme0_boot001",
        "pma0_chest001", "FB1_g", "TF3_g",
    ] {
        let tex = nwn_tileset::load_texture_data(name);
        assert!(tex.unwrap().is_some(), "texture '{name}' should be loadable");
    }
}

#[test]
fn test_module_end_to_end() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/grimshackle".to_string());
    let mod_path = std::path::Path::new(&home)
        .join("Documents/Neverwinter Nights/modules/HOPE_Shargast_CH1_stripped.mod");
    if !mod_path.exists() {
        eprintln!("  Module not found, skipping end-to-end test");
        return;
    }

    let module = nwn_gamemodel::module::Module::open(&mod_path).unwrap();
    module.preload_areas();
    let area_names = module.area_names().to_vec();
    let max_areas = area_names.len().min(3);

    for area_name in &area_names[..max_areas] {
        if let Some(area) = module.get_area(area_name) {
            for c in &area.creatures {
                let resolved = module.resolve_creature_model(c.appearance, &c.resref);
                if let Some(ref model) = resolved {
                    if model.len() > 2 && model != "Character_model" {
                        let meshes = nwn_tileset::load_model_meshes(model).unwrap();
                        assert!(meshes.is_some(),
                            "creature '{}' model '{}' should load", c.resref, model);
                    }
                }
            }
        }
    }
}
