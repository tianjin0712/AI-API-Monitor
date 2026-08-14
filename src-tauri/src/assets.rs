//! Isolated application asset storage and custom resource protocol.
//!
//! Imports arrive as bytes, never as a user filesystem path. Validated files
//! are copied into the per-user application data directory under random names
//! and are served only through opaque `app-resource` URLs.

use image::{AnimationDecoder, ImageDecoder, ImageFormat};
use serde::Serialize;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tauri::{Manager, Runtime};

pub const MAX_ASSET_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_IMAGE_DIMENSION: u32 = 4096;
pub const MAX_GIF_FRAMES: usize = 300;
const ALLOWED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "ico", "svg"];

#[derive(Clone)]
pub struct AssetStore {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedAsset {
    pub asset_id: String,
    pub url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("不支持的图片类型")]
    UnsupportedType,
    #[error("图片文件不能为空")]
    Empty,
    #[error("图片最大允许 20 MB")]
    TooLarge,
    #[error("图片格式与扩展名不一致或文件已损坏")]
    InvalidImage,
    #[error("图片尺寸最大允许 4096x4096")]
    Dimensions,
    #[error("GIF 最大允许 300 帧")]
    GifFrames,
    #[error("SVG 包含活动内容或外部资源引用")]
    UnsafeSvg,
    #[error("图片资源存储失败")]
    Io,
    #[error("无效的图片资源标识")]
    InvalidId,
}

impl AssetStore {
    pub fn new(app_data_dir: &Path) -> Result<Self, AssetError> {
        let root = app_data_dir.join("assets");
        std::fs::create_dir_all(&root).map_err(|_| AssetError::Io)?;
        crate::platform_security::harden_private_path(&root, true).map_err(|_| AssetError::Io)?;
        Ok(Self { root })
    }

    pub fn import(&self, original_name: &str, data: &[u8]) -> Result<ImportedAsset, AssetError> {
        if data.is_empty() {
            return Err(AssetError::Empty);
        }
        if data.len() > MAX_ASSET_BYTES {
            return Err(AssetError::TooLarge);
        }
        let ext = extension_of(original_name).ok_or(AssetError::UnsupportedType)?;
        validate_asset(ext, data)?;

        let normalized_ext = if ext == "jpeg" { "jpg" } else { ext };
        let asset_id = format!("{}.{}", uuid::Uuid::new_v4(), normalized_ext);
        let path = self.root.join(&asset_id);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        use std::io::Write;
        let mut file = options.open(&path).map_err(|_| AssetError::Io)?;
        file.write_all(data).map_err(|_| AssetError::Io)?;
        file.sync_all().map_err(|_| AssetError::Io)?;
        crate::platform_security::harden_private_path(&path, false).map_err(|_| AssetError::Io)?;

        Ok(ImportedAsset {
            url: asset_url(&asset_id),
            asset_id,
        })
    }

    pub fn delete(&self, asset_id: &str) -> Result<(), AssetError> {
        validate_asset_id(asset_id)?;
        let path = self.root.join(asset_id);
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(AssetError::Io),
        }
    }

    pub fn read_bytes(&self, asset_id: &str) -> Result<Vec<u8>, AssetError> {
        self.read(asset_id).map(|(data, _)| data)
    }

    fn read(&self, asset_id: &str) -> Result<(Vec<u8>, &'static str), AssetError> {
        validate_asset_id(asset_id)?;
        let path = self.root.join(asset_id);
        let metadata = std::fs::metadata(&path).map_err(|_| AssetError::InvalidId)?;
        if !metadata.is_file() || metadata.len() > MAX_ASSET_BYTES as u64 {
            return Err(AssetError::InvalidId);
        }
        let data = std::fs::read(path).map_err(|_| AssetError::InvalidId)?;
        let ext = extension_of(asset_id).ok_or(AssetError::InvalidId)?;
        Ok((data, mime_for(ext)))
    }
}

fn extension_of(name: &str) -> Option<&str> {
    let ext = Path::new(name).extension()?.to_str()?;
    let ext = ext.trim().to_ascii_lowercase();
    ALLOWED_EXTENSIONS
        .iter()
        .find(|allowed| **allowed == ext)
        .copied()
}

fn expected_format(ext: &str) -> Option<ImageFormat> {
    match ext {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "webp" => Some(ImageFormat::WebP),
        "gif" => Some(ImageFormat::Gif),
        "ico" => Some(ImageFormat::Ico),
        _ => None,
    }
}

fn validate_asset(ext: &str, data: &[u8]) -> Result<(), AssetError> {
    if ext == "svg" {
        return validate_svg(data);
    }
    let expected = expected_format(ext).ok_or(AssetError::UnsupportedType)?;
    if image::guess_format(data).ok() != Some(expected) {
        return Err(AssetError::InvalidImage);
    }

    if expected == ImageFormat::Gif {
        let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(data))
            .map_err(|_| AssetError::InvalidImage)?;
        let (width, height) = decoder.dimensions();
        validate_dimensions(width, height)?;
        let frame_count = decoder.into_frames().take(MAX_GIF_FRAMES + 1).count();
        if frame_count > MAX_GIF_FRAMES {
            return Err(AssetError::GifFrames);
        }
        return Ok(());
    }

    let reader = image::ImageReader::with_format(Cursor::new(data), expected);
    let (width, height) = reader
        .into_dimensions()
        .map_err(|_| AssetError::InvalidImage)?;
    validate_dimensions(width, height)
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), AssetError> {
    if width == 0 || height == 0 || width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        Err(AssetError::Dimensions)
    } else {
        Ok(())
    }
}

fn validate_svg(data: &[u8]) -> Result<(), AssetError> {
    let text = std::str::from_utf8(data).map_err(|_| AssetError::InvalidImage)?;
    let lower = text.to_ascii_lowercase();
    if !lower.contains("<svg") {
        return Err(AssetError::InvalidImage);
    }
    const FORBIDDEN: &[&str] = &[
        "<script",
        "<foreignobject",
        "<!doctype",
        "<!entity",
        "javascript:",
        "data:text/html",
        "onload=",
        "onclick=",
        "onerror=",
        "href=\"http",
        "href='http",
        "xlink:href=\"http",
        "xlink:href='http",
    ];
    if FORBIDDEN.iter().any(|marker| lower.contains(marker)) {
        return Err(AssetError::UnsafeSvg);
    }
    Ok(())
}

fn validate_asset_id(asset_id: &str) -> Result<(), AssetError> {
    if asset_id.len() > 64
        || asset_id.contains(['/', '\\'])
        || asset_id.contains("..")
        || !asset_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.')
        || extension_of(asset_id).is_none()
    {
        return Err(AssetError::InvalidId);
    }
    Ok(())
}

fn mime_for(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

#[cfg(target_os = "windows")]
fn asset_url(asset_id: &str) -> String {
    format!("http://app-resource.localhost/asset/{asset_id}")
}

#[cfg(not(target_os = "windows"))]
fn asset_url(asset_id: &str) -> String {
    format!("app-resource://localhost/asset/{asset_id}")
}

pub fn protocol_response<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let asset_id = request.uri().path().strip_prefix("/asset/").unwrap_or("");
    let result = app.state::<AssetStore>().read(asset_id);
    match result {
        Ok((data, mime)) => tauri::http::Response::builder()
            .status(tauri::http::StatusCode::OK)
            .header(tauri::http::header::CONTENT_TYPE, mime)
            .header("X-Content-Type-Options", "nosniff")
            .header("Content-Security-Policy", "default-src 'none'; sandbox")
            .header("Cache-Control", "private, max-age=31536000, immutable")
            .body(data)
            .expect("valid asset response"),
        Err(_) => tauri::http::Response::builder()
            .status(tauri::http::StatusCode::NOT_FOUND)
            .header(
                tauri::http::header::CONTENT_TYPE,
                "text/plain; charset=utf-8",
            )
            .header("X-Content-Type-Options", "nosniff")
            .body(b"asset not found".to_vec())
            .expect("valid not-found response"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_executables_and_path_traversal() {
        assert!(extension_of("payload.exe").is_none());
        assert!(extension_of("payload.html").is_none());
        assert!(validate_asset_id("../secret.gif").is_err());
    }

    #[test]
    fn rejects_active_svg() {
        assert!(validate_svg(b"<svg><script>alert(1)</script></svg>").is_err());
        assert!(validate_svg(b"<svg onload='alert(1)'></svg>").is_err());
        assert!(validate_svg(b"<svg><rect width='1' height='1'/></svg>").is_ok());
    }

    #[test]
    fn rejects_oversized_payload_before_decode() {
        let payload = vec![0u8; MAX_ASSET_BYTES + 1];
        let temp = std::env::temp_dir().join(format!("ai-monitor-test-{}", uuid::Uuid::new_v4()));
        let store = AssetStore::new(&temp).unwrap();
        assert!(matches!(
            store.import("large.gif", &payload),
            Err(AssetError::TooLarge)
        ));
        let _ = std::fs::remove_dir_all(temp);
    }
}
