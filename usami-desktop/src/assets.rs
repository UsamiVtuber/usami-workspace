//! Resolução de caminhos de assets (ícones, fundo, VRM, VRMA).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// Caminhos relativos à crate: o app roda de qualquer lugar do disco e continua
// achando ícones, fundo, VRM e VRMA. Variáveis abaixo permitem apontar outra
// instalação sem recompilar.
pub(crate) fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Pasta empacotada (release): `assets/` ao lado do executável. Em
/// desenvolvimento não existe, então cai para a pasta da crate.
pub(crate) fn asset_base_dir() -> &'static Path {
    static BASE: OnceLock<PathBuf> = OnceLock::new();
    BASE.get_or_init(|| {
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
            && dir.join("assets").is_dir()
        {
            return dir.to_path_buf();
        }
        manifest_dir()
    })
}

pub(crate) fn asset_path(relative: &str) -> PathBuf {
    asset_base_dir().join(relative)
}

pub(crate) fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.exists())
}

pub(crate) fn first_existing(
    candidates: impl IntoIterator<Item = Option<PathBuf>>,
) -> Option<PathBuf> {
    candidates.into_iter().flatten().find(|path| path.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_paths_resolve_from_crate_dir() {
        let path = asset_path("assets/icons/chat.svg");
        assert!(path.is_absolute());
        assert!(path.starts_with(manifest_dir()));
        assert!(path.exists(), "ícone esperado em {}", path.display());
    }

    #[test]
    fn first_existing_skips_missing_candidates() {
        let dir =
            std::env::temp_dir().join(format!("usami-desktop-path-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("cria diretório temporário");
        let missing = dir.join("missing.png");
        let present = dir.join("present.png");
        std::fs::write(&present, b"x").expect("cria arquivo de teste");

        assert_eq!(
            first_existing([Some(missing.clone()), Some(present.clone())]),
            Some(present)
        );
        assert_eq!(first_existing([Some(missing), None]), None);

        std::fs::remove_dir_all(&dir).ok();
    }
}
