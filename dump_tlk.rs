fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: dump_tlk <tlk_file> <strref_start> [strref_end]");
        return;
    }
    let tlk = nwn_tlk::TlkFile::open(&args[1]).unwrap();
    let start: u32 = args[2].parse().unwrap();
    let end: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(start + 20);
    for i in start..=end {
        if let Some(s) = tlk.strref(i) {
            println!("STRREF {}: {}", i, s);
        } else {
            println!("STRREF {}: (none)", i);
        }
    }
    println!("\nTotal entries: {}", tlk.count());
}
