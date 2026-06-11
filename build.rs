use std::{env, fs, path::Path};

const VERSIONED_ASSETS: &[&str] = &[
    "static/css/app.css",
    "static/favicon.svg",
    "static/js/i18n.js",
    "static/js/app.js",
];

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
    let mut hash = 0xcbf29ce484222325_u64;

    for relative_path in VERSIONED_ASSETS {
        println!("cargo:rerun-if-changed={relative_path}");
        let bytes = fs::read(Path::new(&manifest_dir).join(relative_path))
            .unwrap_or_else(|error| panic!("failed to read {relative_path}: {error}"));
        for byte in relative_path.bytes().chain(bytes) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    println!("cargo:rustc-env=DOGN_ASSET_VERSION={hash:016x}");
}
