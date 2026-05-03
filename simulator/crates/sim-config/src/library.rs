//! IC library scanner. Walks an `ic-library/` directory, loads every
//! `manifest.yaml`, validates each, and indexes them by `vendor/part@version`.

use crate::manifest::IcManifest;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct IcLibrary {
    /// Index by `vendor/part@version` (the form used in board YAML
    /// `type:` references).
    pub by_ref: HashMap<String, IcEntry>,
}

pub struct IcEntry {
    pub manifest: IcManifest,
    /// Path to the manifest file (useful for editor icon lookup, etc.).
    pub manifest_path: PathBuf,
    /// Path to icon.svg if present.
    pub icon_path: Option<PathBuf>,
}

impl IcLibrary {
    /// Scan a directory tree, loading every `manifest.yaml` found at depth 2
    /// (i.e. `<library_root>/<vendor>_<part>/manifest.yaml`).
    pub fn load(_root: &Path) -> Result<Self, crate::validate::ValidationError> {
        // TODO: walk root, load+validate each manifest, build the index.
        unimplemented!()
    }

    pub fn lookup(&self, _type_ref: &str) -> Option<&IcEntry> {
        // TODO
        unimplemented!()
    }
}
