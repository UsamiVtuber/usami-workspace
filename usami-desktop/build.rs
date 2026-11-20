use std::env;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon_path = manifest_dir
        .join("assets")
        .join("icons")
        .join("app-icon.ico");

    println!("cargo:rerun-if-changed={}", icon_path.display());
    println!("cargo:rerun-if-changed=build.rs");

    if icon_path.exists() {
        let mut res = winres::WindowsResource::new();
        let icon_str = icon_path.to_str().unwrap().replace('\\', "/");
        res.set_icon(&icon_str);
        res.set("ProductName", "Usami Desktop");
        res.set("FileDescription", "Usami Desktop Companion");
        if let Err(e) = res.compile() {
            eprintln!(
                "cargo:warning=Não foi possível compilar recursos do Windows com winres: {e}"
            );
        }
    } else {
        eprintln!(
            "cargo:warning=Ícone não encontrado para embutir no binário: {}",
            icon_path.display()
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {}
