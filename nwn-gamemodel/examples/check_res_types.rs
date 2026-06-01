/// Check if a given resref exists at different resource types
fn main() {
    let gd = nwn_gamemodel::game_data::GameData::new().expect("GameData::new");

    for resref in &["pmh0_chest001", "pmh0_bicepl001", "pmh0_forel001", "c_drgblack_bod", "FB1_g", "TF3_g", "tcn01_c04_04"] {
        eprintln!("--- {} ---", resref);
        for restype in &[8u16, 3u16, 1u16, 6u16] {
            let typename = match restype {
                8 => "DDS",
                3 => "TGA",
                1 => "BMP",
                6 => "PLT",
                _ => "?",
            };
            match gd.get_resource(resref, *restype) {
                Some(data) => {
                    eprintln!("  type {} ({}): {} bytes", restype, typename, data.len());
                }
                None => {
                    eprintln!("  type {} ({}): NOT FOUND", restype, typename);
                }
            }
        }
    }
}
