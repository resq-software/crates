// This file is intentionally missing a copyright header.
// Run `resq copyright` from this directory to add one.

fn main() {
    println!("Hello from the sample project!");
    let config = load_config();
    println!("Loaded config: {}", config);
}

fn load_config() -> String {
    String::from("default")
}
