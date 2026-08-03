use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source_dir = manifest.join("vendor/cadical/src");
    let wrapper = manifest.join("src/mrs_cadical.cpp");
    let mut sources = Vec::new();
    for entry in fs::read_dir(&source_dir).expect("read CaDiCaL source directory") {
        let path = entry.expect("read CaDiCaL source entry").path();
        if path.extension().is_some_and(|extension| extension == "cpp") {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !matches!(
                name,
                "cadical.cpp" | "mobical.cpp" | "ccadical.cpp" | "ipasir.cpp"
            ) {
                sources.push(path);
            }
        }
    }
    sources.push(wrapper.clone());
    sources.sort();

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .warnings(false)
        .define("NBUILD", None)
        .define("NUNLOCKED", None)
        .define("QUIET", None)
        .define("VERSION", "\"3.0.1\"")
        .include(&source_dir);
    for source in &sources {
        build.file(source);
        println!("cargo:rerun-if-changed={}", source.display());
    }
    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("vendor/cadical/VERSION").display()
    );
    build.compile("mrs_cadical");

    let kitten = source_dir.join("kitten.c");
    let mut c_build = cc::Build::new();
    c_build.warnings(false).include(&source_dir).file(&kitten);
    println!("cargo:rerun-if-changed={}", kitten.display());
    c_build.compile("mrs_cadical_kitten");
}
