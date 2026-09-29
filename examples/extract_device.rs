//! Pulls one device block out of a gunzipped Live set, for refreshing the
//! device assets in `templates/devices/`.
//!
//! Run: `cargo run --example extract_device -- <set.xml> <DeviceTag> <out.xml>`
//! e.g. `... -- scratch/edited.als StringStudio templates/devices/tension.xml`.
//!
//! Takes the first `<DeviceTag …>…</DeviceTag>` in the file, as Live wrote it.
//! Refuses to write a block that names a `/Users/` path.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [set, tag, out] = args.as_slice() else {
        eprintln!("usage: extract_device <set.xml> <DeviceTag> <out.xml>");
        std::process::exit(2);
    };
    let xml = std::fs::read_to_string(set).expect("read set");
    let open = format!("<{tag} ");
    let close = format!("</{tag}>");
    let start = xml.find(&open).expect("device not found");
    let end = start + xml[start..].find(&close).expect("device not closed") + close.len();
    let block = &xml[start..end];
    if block.contains("/Users/") {
        eprintln!("block names a /Users/ path; clean the set first");
        std::process::exit(1);
    }
    std::fs::write(out, format!("{block}\n")).expect("write block");
    println!("{out}: {} lines", block.lines().count());
}
