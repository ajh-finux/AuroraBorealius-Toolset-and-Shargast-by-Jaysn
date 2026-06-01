use nwnrs_types::resman::ResContainer;

#[test]
fn test_pmd0_loading() {
    let home = std::env::var("HOME").unwrap();
    let data_dir = std::path::Path::new(&home)
        .join(".local/share/Steam/steamapps/common/Neverwinter Nights/data");
    
    let rr = nwnrs_types::resman::ResRef::new(
        "pmd0", 
        nwnrs_types::resman::ResType(2002),
    ).unwrap();
    
    let key_path = data_dir.join("nwn_base.key");
    let key_table = nwnrs_types::key::read_key_table_from_file(&key_path).unwrap();
    
    assert!(key_table.contains(&rr), "pmd0 should be in nwn_base.key");
    
    let res = key_table.demand(&rr).unwrap();
    let data = res.read_all(nwnrs_types::resman::CachePolicy::Bypass).unwrap();
    assert!(!data.is_empty(), "pmd0 data should not be empty");
    println!("pmd0.mdl loaded: {} bytes", data.len());
}
