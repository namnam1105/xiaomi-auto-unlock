use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=ui");
    let config = slint_build::CompilerConfiguration::new()
    .with_style("material-dark".to_string())
    .with_include_paths(vec!["ui/".into()]);

    let main_ui_file = Path::new("ui/main.slint");

    // .with_include_paths(vec!["ui/main.slint", "ui/about.slint"]);
    slint_build::compile_with_config(main_ui_file, config)
        .unwrap_or_else(|err| {
            panic!("Error compiling main window Slint file: {}", err);
        });

}