use std::path::Path;
fn main() {
    let path = Path::new("/home/grimshackle/HOPE_Shargast_CH1_fixed_save.mod");
    let erf = nwn_erf::ErfFile::open(path).unwrap();
    let mut total = 0;
    let mut with_name = 0;
    for r in &erf.inner.resources {
        if r.resource_type != 2012 { continue; }
        total += 1;
        let resref = r.file_name.to_string().trim().to_string();
        if let Some(data) = erf.get(&resref, 2012) {
            if let Ok(gff) = nwn_gff::GffFile::from_bytes(&data) {
                let display = gff.get_string("Area_Name").unwrap_or_default();
                if !display.is_empty() && display != resref {
                    with_name += 1;
                    if with_name <= 5 {
                        println!("  '{}' -> '{}'", resref, display);
                    }
                }
            }
        }
    }
    println!("Total: {}, with different name: {}", total, with_name);
}
