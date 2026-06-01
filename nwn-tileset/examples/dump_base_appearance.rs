#![feature(generic_const_exprs)]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --example dump_base_appearance -- <data_dir>");
        return;
    }
    let data_dir = &args[1];
    
    // Try to find appearance.2da in game data
    use std::sync::Mutex;
    use std::collections::HashMap;
    use std::path::Path;
    use nwnrs_types::resman::ResRef;
    
    // Use the same approach as load_resource_from_key
    let data_path = Path::new(data_dir);
    let key_files = ["nwn_base.key", "nwn_retail.key"];
    
    for key_name in &key_files {
        let key_path = data_path.join(key_name);
        if !key_path.exists() { continue; }
        let data = std::fs::read(&key_path).unwrap();
        
        if data.len() < 56 { continue; }
        let bif_count = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let key_count = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
        let off_file_tbl = u32::from_le_bytes([data[16], data[17], data[18], data[19]]) as usize;
        let off_key_tbl = u32::from_le_bytes([data[20], data[21], data[22], data[23]]) as usize;
        
        // Find appearance.2da in key entries
        for i in 0..key_count {
            let base = off_key_tbl + i * 22;
            if base + 22 > data.len() { break; }
            let resref_bytes = &data[base..base+16];
            let end = resref_bytes.iter().position(|&b| b == 0).unwrap_or(16);
            let resref = String::from_utf8_lossy(&resref_bytes[..end]).trim().to_lowercase();
            if resref.is_empty() { continue; }
            let restype = u16::from_le_bytes([data[base+16], data[base+17]]);
            if resref == "appearance" && restype == 2017 {
                let resource_id = u32::from_le_bytes([data[base+18], data[base+19], data[base+20], data[base+21]]);
                let bif_idx_raw = (resource_id >> 20) as usize;
                let resource_idx = (resource_id & 0xFFFFF) as usize;
                println!("Found appearance.2da in {}: bif_idx={} resource_idx={}", key_name, bif_idx_raw, resource_idx);
                
                // Now find the BIF file name
                let off_file_tbl_entries = &data[off_file_tbl..off_file_tbl + bif_count * 12];
                if bif_idx_raw < bif_count {
                    let bf_base = off_file_tbl + bif_idx_raw * 12;
                    let fn_off = u32::from_le_bytes([data[bf_base+4], data[bf_base+5], data[bf_base+6], data[bf_base+7]]) as usize;
                    let fn_len = u16::from_le_bytes([data[bf_base+8], data[bf_base+9]]) as usize;
                    if fn_off + fn_len <= data.len() {
                        let name = String::from_utf8_lossy(&data[fn_off..fn_off + fn_len]).trim_end_matches('\0').to_string();
                        println!("BIF file: {}", name);
                        
                        // Load the BIF
                        let clean_name = name.replace('\\', "/");
                        let fname = clean_name.rsplit('/').next().unwrap_or(&clean_name);
                        let bif_path = data_path.join(fname);
                        if bif_path.exists() {
                            let bif_data = std::fs::read(&bif_path).unwrap();
                            // BIF header: 4+4+4 (variable resource count at offset 8)
                            // Actually parse BIF resource entries
                            // Each BIF resource entry is 16 bytes (res_id + offset + size)
                            let var_res_count = u32::from_le_bytes([bif_data[8], bif_data[9], bif_data[10], bif_data[11]]) as usize;
                            
                            if resource_idx < var_res_count {
                                // Resource entry at offset 20 + resource_idx * 16
                                let entry_off = 20 + resource_idx * 16;
                                let res_id = u32::from_le_bytes([bif_data[entry_off], bif_data[entry_off+1], bif_data[entry_off+2], bif_data[entry_off+3]]);
                                let res_off = u32::from_le_bytes([bif_data[entry_off+4], bif_data[entry_off+5], bif_data[entry_off+6], bif_data[entry_off+7]]) as usize;
                                let res_size = u32::from_le_bytes([bif_data[entry_off+8], bif_data[entry_off+9], bif_data[entry_off+10], bif_data[entry_off+11]]) as usize;
                                
                                println!("Resource: id={} offset={} size={}", res_id, res_off, res_size);
                                
                                if res_off + res_size <= bif_data.len() {
                                    let two_da_text = String::from_utf8_lossy(&bif_data[res_off..res_off+res_size]);
                                    println!("\nRow 0 (first few lines):");
                                    for line in two_da_text.lines().skip(2).take(3) {
                                        println!("  {}", line);
                                    }
                                    
                                    // Parse columns and row 0
                                    let lines: Vec<&str> = two_da_text.lines().collect();
                                    if lines.len() > 3 {
                                        let header = lines[2].trim();
                                        println!("\nColumns: {}", header);
                                        
                                        let row0 = lines[3].trim();
                                        println!("Row 0: {}", row0);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
