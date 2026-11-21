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

pub(crate) fn first_existing(candidates: impl IntoIterator<Item = Option<PathBuf>>) -> Option<PathBuf> {
    candidates.into_iter().flatten().find(|path| path.exists())
}

/// Verifica se o arquivo tem extensão suportada pelo motor WeebUI (`.vrm` ou `.glb`).
pub(crate) fn is_supported_vrm_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| {
            let lower = s.to_ascii_lowercase();
            lower == "vrm" || lower == "glb"
        })
        .unwrap_or(false)
}

/// Nome amigável derivado do arquivo `.vrm` para exibição nos cards da galeria.
pub(crate) fn vrm_display_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Avatar");
    let words: Vec<String> = stem
        .split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect();
    if words.is_empty() {
        stem.to_string()
    } else {
        words.join(" ")
    }
}

/// Descobre modelos `.vrm` disponíveis nas pastas conhecidas do workspace e do
/// diretório local de dados (`%APPDATA%\Usami\avatars`).
pub(crate) fn discover_vrm_models_in(dirs: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let is_vrm = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.eq_ignore_ascii_case("vrm"))
                .unwrap_or(false);
            if !is_vrm {
                continue;
            }
            let canonical_key = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if seen.insert(canonical_key) {
                found.push(path);
            }
        }
    }

    found.sort_by(|a, b| {
        let a_name = a
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let b_name = b
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        // Mantém shiro.vrm primeiro como modelo padrão da Usami.
        let a_default = a_name == "shiro.vrm";
        let b_default = b_name == "shiro.vrm";
        b_default.cmp(&a_default).then_with(|| a_name.cmp(&b_name))
    });

    found
}

pub(crate) fn discover_vrm_models() -> Vec<PathBuf> {
    discover_vrm_models_in([
        asset_path("../usami/public/models"),
        asset_path("assets/models"),
        asset_path("assets"),
        crate::storage::data_dir().join("avatars"),
    ])
}

/// Nome amigável derivado de um arquivo `.vrma` (remove prefixo técnico `aidelle_`).
pub(crate) fn vrma_display_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Animação");
    let stripped = stem
        .strip_prefix("aidelle_")
        .or_else(|| stem.strip_prefix("Aidelle_"))
        .unwrap_or(stem);
    let words: Vec<String> = stripped
        .split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect();
    if words.is_empty() {
        stripped.to_string()
    } else {
        words.join(" ")
    }
}

/// Descobre todos os clipes `.vrma` disponíveis na pasta configurada, ordenados
/// alfabeticamente pelo nome amigável.
pub(crate) fn discover_vrma_clips(dir: Option<&Path>) -> Vec<PathBuf> {
    let Some(dir) = dir else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut clips: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("vrma"))
        })
        .collect();
    clips.sort_by(|a, b| {
        vrma_display_name(a)
            .to_ascii_lowercase()
            .cmp(&vrma_display_name(b).to_ascii_lowercase())
    });
    clips
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

    #[test]
    fn vrm_display_name_formats_stems_cleanly() {
        assert_eq!(vrm_display_name(Path::new("shiro.vrm")), "Shiro");
        assert_eq!(
            vrm_display_name(Path::new("models/companion-a.vrm")),
            "Companion A"
        );
        assert_eq!(
            vrm_display_name(Path::new("custom_avatar_v2.vrm")),
            "Custom Avatar V2"
        );
    }

    #[test]
    fn discover_vrm_models_prioritizes_shiro_and_ignores_backups() {
        let dir = std::env::temp_dir().join(format!(
            "usami-desktop-vrm-discover-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("cria diretório temporário");
        std::fs::write(dir.join("companion-b.vrm"), b"b").unwrap();
        std::fs::write(dir.join("shiro.vrm"), b"s").unwrap();
        std::fs::write(dir.join("companion-a.vrm"), b"a").unwrap();
        std::fs::write(dir.join("shiro.vrm.backup"), b"bak").unwrap();
        std::fs::write(dir.join("notes.txt"), b"txt").unwrap();

        let models = discover_vrm_models_in([dir.clone()]);
        let names: Vec<String> = models
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(String::from))
            .collect();
        assert_eq!(names, vec!["shiro.vrm", "companion-a.vrm", "companion-b.vrm"]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn vrma_display_name_strips_aidelle_prefix_and_formats() {
        assert_eq!(
            vrma_display_name(Path::new("aidelle_Head_Nod_Yes.vrma")),
            "Head Nod Yes"
        );
        assert_eq!(vrma_display_name(Path::new("aidelle_Idle.vrma")), "Idle");
        assert_eq!(vrma_display_name(Path::new("Clapping.vrma")), "Clapping");
    }

    #[test]
    fn discover_vrma_clips_lists_sorted_vrma_files_only() {
        let dir = std::env::temp_dir().join(format!(
            "usami-desktop-vrma-discover-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("cria diretório temporário");
        std::fs::write(dir.join("Jump.vrma"), b"j").unwrap();
        std::fs::write(dir.join("aidelle_Idle.vrma"), b"i").unwrap();
        std::fs::write(dir.join("Clapping.vrma"), b"c").unwrap();
        std::fs::write(dir.join("readme.md"), b"r").unwrap();

        let clips = discover_vrma_clips(Some(&dir));
        let labels: Vec<String> = clips.iter().map(|p| vrma_display_name(p)).collect();
        assert_eq!(labels, vec!["Clapping", "Idle", "Jump"]);

        std::fs::remove_dir_all(&dir).ok();
    }
}
