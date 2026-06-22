use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ImageFile {
    pub path: PathBuf,
    pub size_bytes: u64,
}

impl ImageFile {
    pub fn file_name_label(&self) -> String {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown image")
            .to_string()
    }

    pub fn fits_in_bytes(&self, target_size_bytes: u64) -> bool {
        self.size_bytes <= target_size_bytes
    }
}

pub fn inspect_image(path: impl AsRef<Path>) -> Result<ImageFile, String> {
    let path = path.as_ref();

    let metadata =
        fs::metadata(path).map_err(|error| format!("failed to read image metadata: {error}"))?;

    if !metadata.is_file() {
        return Err("selected path is not a regular file".to_string());
    }

    if metadata.len() == 0 {
        return Err("selected image file is empty".to_string());
    }

    Ok(ImageFile {
        path: path.to_path_buf(),
        size_bytes: metadata.len(),
    })
}
