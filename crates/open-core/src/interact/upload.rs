use std::path::{Path, PathBuf};

use super::element::ElementHandle;
use crate::page::Page;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub file_name: String,
    pub mime_type: String,
    pub content: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum UploadError {
    #[error("file not found: {0}")]
    FileNotFound(PathBuf),
    #[error("file size ({size}) exceeds maximum allowed ({max})")]
    FileTooLarge { size: usize, max: usize },
    #[error("file type '{mime}' not accepted by accept filter: {accept}")]
    AcceptMismatch { mime: String, accept: String },
    #[error("element is not a file input")]
    NotFileInput,
    #[error("element is disabled")]
    Disabled,
    #[error("multiple attribute not set, only 1 file allowed (got {count})")]
    MultipleNotAllowed { count: usize },
    #[error("sandbox mode active: file uploads are blocked")]
    SandboxBlocked,
    #[error("path must be absolute: {0}")]
    NotAbsolutePath(PathBuf),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl FileEntry {
    pub fn new(path: &Path, max_size: usize) -> Result<Self, UploadError> {
        if !path.is_absolute() {
            return Err(UploadError::NotAbsolutePath(path.to_path_buf()));
        }

        if !path.exists() {
            return Err(UploadError::FileNotFound(path.to_path_buf()));
        }

        if path.is_symlink() {
            return Err(UploadError::FileNotFound(path.to_path_buf()));
        }

        let canonical = path.canonicalize().map_err(|e| UploadError::Io(e))?;

        let content = std::fs::read(&canonical)?;
        let size = content.len();

        if size > max_size {
            return Err(UploadError::FileTooLarge {
                size,
                max: max_size,
            });
        }

        let file_name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mime_type = detect_mime(&canonical, &content);

        Ok(FileEntry {
            path: canonical,
            file_name,
            mime_type,
            content,
        })
    }
}

fn detect_mime(path: &Path, content: &[u8]) -> String {
    if let Some(mime) = mime_guess::from_path(path).first() {
        let s = mime.essence_str().to_string();
        if s != "application/octet-stream" {
            return s;
        }
    }

    match content {
        [0x25, 0x50, 0x44, 0x46, ..] => "application/pdf".to_string(),
        [0x89, 0x50, 0x4E, 0x47, ..] => "image/png".to_string(),
        [0xFF, 0xD8, 0xFF, ..] => "image/jpeg".to_string(),
        [0x47, 0x49, 0x46, 0x38, ..] => "image/gif".to_string(),
        [0x52, 0x49, 0x46, 0x46, ..] => "image/webp".to_string(),
        [0x1F, 0x8B, ..] => "application/gzip".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

pub fn validate_accept(file_name: &str, mime: &str, accept: &str) -> Result<(), UploadError> {
    let patterns: Vec<&str> = accept
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if patterns.is_empty() {
        return Ok(());
    }

    let ext = Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();

    for pattern in &patterns {
        if pattern.starts_with('.') {
            if ext == pattern.to_lowercase() {
                return Ok(());
            }
        } else if pattern.ends_with("/*") {
            let prefix = &pattern[..pattern.len() - 2];
            if mime.starts_with(prefix) {
                return Ok(());
            }
        } else if mime == *pattern {
            return Ok(());
        }
    }

    Err(UploadError::AcceptMismatch {
        mime: mime.to_string(),
        accept: accept.to_string(),
    })
}

pub fn upload_files(
    _page: &Page,
    handle: &ElementHandle,
    paths: &[PathBuf],
    max_size: usize,
) -> Result<Vec<FileEntry>, UploadError> {
    if handle.is_disabled {
        return Err(UploadError::Disabled);
    }

    if handle.action.as_deref() != Some("upload") {
        if handle.input_type.as_deref() == Some("file") {
            return Err(UploadError::NotFileInput);
        }
        return Err(UploadError::NotFileInput);
    }

    if !handle.multiple && paths.len() > 1 {
        return Err(UploadError::MultipleNotAllowed { count: paths.len() });
    }

    let accept = handle.accept.as_deref().unwrap_or("");

    let mut files = Vec::new();
    for path in paths {
        let entry = FileEntry::new(path, max_size)?;
        if !accept.is_empty() {
            validate_accept(&entry.file_name, &entry.mime_type, accept)?;
        }
        files.push(entry);
    }

    Ok(files)
}
