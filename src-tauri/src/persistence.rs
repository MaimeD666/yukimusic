use serde::Serialize;
use std::{io::Write, path::Path};

pub fn atomic_write<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path.parent().ok_or("Invalid storage path")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = path.with_extension("tmp");
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let mut file = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(temp, path).map_err(|e| e.to_string())
}

pub fn prune_artwork(directory: &Path) {
    // Only our content-addressed cache files; never follow subdirectories or links.
    let Ok(items) = std::fs::read_dir(directory) else {
        return;
    };
    let mut files: Vec<_> = items
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.len() != 68
                || !name.ends_with(".jpg")
                || !name[..64].bytes().all(|b| b.is_ascii_hexdigit())
            {
                return None;
            }
            if !entry.file_type().ok()?.is_file() {
                return None;
            }
            let meta = entry.metadata().ok()?;
            Some((entry.path(), meta.len(), meta.modified().ok()?))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort_by_key(|f| f.2);
    for (path, bytes, _) in files {
        if total <= 96 * 1024 * 1024 {
            break;
        }
        if std::fs::remove_file(path).is_ok() {
            total = total.saturating_sub(bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replaces_existing_snapshot_on_windows() {
        let path =
            std::env::temp_dir().join(format!("yukicloud-test-{}.json", uuid::Uuid::new_v4()));
        atomic_write(&path, &vec![1, 2]).unwrap();
        atomic_write(&path, &vec![3, 4]).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[3,4]");
        std::fs::remove_file(path).unwrap();
    }
}
