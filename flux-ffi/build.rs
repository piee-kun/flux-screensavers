use cbindgen::{Config, Language};
use std::path::PathBuf;
use std::{env, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let package_name = env::var("CARGO_PKG_NAME").unwrap();
    let output_file = target_dir()
        .join(format!("{}.h", package_name))
        .display()
        .to_string();

    let config = Config {
        language: Language::C,
        // namespace: Some(String::from("lib")),
        // function: cbindgen::FunctionConfig {
        //     // swift_name_macro: Some("CF_SWIFT_NAME".to_string()), // TODO: control the Swift name
        //     ..Default::default()
        // },
        ..Default::default()
    };

    cbindgen::generate_with_config(&crate_dir, config)
        .unwrap()
        .write_to_file(&output_file);

    Ok(())
}

fn target_dir() -> PathBuf {
    // PathBuf::from(env::var("CARGO_TARGET_DIR").unwrap())

    if let Ok(target) = env::var("CARGO_TARGET_DIR") {
        PathBuf::from(target)
    } else {
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("target")
    }
}
