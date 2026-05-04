//! IC library scanner. Walks an `ic-library/` directory, loads every
//! `manifest.yaml`, validates each, and indexes them by `vendor/part@version`.

use crate::manifest::IcManifest;
use crate::validate::ValidationError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct IcLibrary {
    /// Index by `vendor/part@version` (the form used in board YAML `type:` fields).
    pub by_ref: HashMap<String, IcEntry>,
}

pub struct IcEntry {
    pub manifest: IcManifest,
    /// Absolute path to the manifest file.
    pub manifest_path: PathBuf,
    /// Absolute path to `icon.svg` if present.
    pub icon_path: Option<PathBuf>,
}

impl IcLibrary {
    /// Scan a library root directory. Every immediate subdirectory that contains
    /// a `manifest.yaml` is loaded and validated. The resulting entry is indexed
    /// by `manifest.id + "@" + manifest.version` (e.g. `bosch/bme280@0.1`).
    pub fn load(root: &Path) -> Result<Self, ValidationError> {
        let mut by_ref: HashMap<String, IcEntry> = HashMap::new();

        let read_dir = std::fs::read_dir(root).map_err(|e| ValidationError::Io {
            path: root.display().to_string(),
            source: e,
        })?;

        for dir_result in read_dir {
            let dir_entry = dir_result.map_err(|e| ValidationError::Io {
                path: root.display().to_string(),
                source: e,
            })?;

            let dir_path = dir_entry.path();
            if !dir_path.is_dir() {
                continue;
            }

            let manifest_path = dir_path.join("manifest.yaml");
            if !manifest_path.exists() {
                tracing::warn!(
                    path = %dir_path.display(),
                    "directory in IC library has no manifest.yaml, skipping"
                );
                continue;
            }

            let manifest = IcManifest::load_yaml_file(&manifest_path)?;
            let type_ref = format!("{}@{}", manifest.id, manifest.version);

            if by_ref.contains_key(&type_ref) {
                return Err(ValidationError::CrossValidation(format!(
                    "duplicate IC type-ref '{type_ref}' found in library"
                )));
            }

            let icon_path = {
                let p = dir_path.join("icon.svg");
                if p.exists() { Some(p) } else { None }
            };

            tracing::debug!(type_ref = %type_ref, "loaded IC manifest");
            by_ref.insert(type_ref, IcEntry { manifest, manifest_path, icon_path });
        }

        Ok(IcLibrary { by_ref })
    }

    /// Look up an IC entry by its type-ref string (e.g. `bosch/bme280@0.1`).
    pub fn lookup(&self, type_ref: &str) -> Option<&IcEntry> {
        self.by_ref.get(type_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_example_library() {
        // Four directories: st_stm32f4, bosch_bme280, microchip_mcp23017, generic_gpio_led
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../ic-library");
        let lib = IcLibrary::load(&root).unwrap();
        assert!(lib.lookup("bosch/bme280@0.1").is_some());
        assert!(lib.lookup("st/stm32f4@0.1").is_some());
        assert!(lib.lookup("microchip/mcp23017@0.1").is_some());
        assert!(lib.lookup("generic/gpio_led@0.1").is_some());
        assert!(lib.lookup("nonexistent/part@0.0").is_none());
    }
}
