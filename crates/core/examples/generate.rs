//! Print the engine script for a configuration file:
//! `cargo run -p keyforge-core --example generate -- config.json`
use keyforge_core::{ahk, store, validate};

fn main() {
    let path = std::env::args().nth(1).expect("usage: generate <config.json>");
    let config = store::read_file(std::path::Path::new(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    let (report, plan) = validate::validate(&config);
    for w in &report.warnings {
        eprintln!("warning: {}", w.message);
    }
    print!("{}", ahk::generate(&plan.unwrap(), &ahk::GenOptions { scripts_dir: "C:\\KeyForge\\scripts" }));
}
