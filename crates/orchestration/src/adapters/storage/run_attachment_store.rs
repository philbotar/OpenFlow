use crate::run::ports::{
    AttachmentError, AttachmentPreview, RunAttachmentStore, StagedAttachment, MAX_CHAT_ATTACHMENTS,
    MAX_CHAT_ATTACHMENT_BYTES, MAX_CHAT_ATTACHMENT_TOTAL_BYTES,
};
use engine::{ChatAttachmentKind, ChatAttachmentRef};
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageReader};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

const ATTACHMENT_PREVIEW_MAX_DIMENSION: u32 = 512;
const ATTACHMENT_PREVIEW_MAX_BYTES: usize = 512 * 1024;
const COPY_BUFFER_BYTES: usize = 64 * 1024;
const STAGED_TOKEN_PREFIX: &str = "openflow-staged:";
const STAGING_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Copy)]
struct AttachmentFormat {
    media_type: &'static str,
    extension: &'static str,
    kind: ChatAttachmentKind,
}

#[derive(Debug, Default)]
struct BatchCleanup {
    paths: Vec<PathBuf>,
    committed: bool,
}

impl BatchCleanup {
    fn track(&mut self, path: PathBuf) {
        self.paths.push(path);
    }

    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for BatchCleanup {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for path in &self.paths {
            let _ = fs::remove_file(path);
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileRunAttachmentStore {
    staging_root: PathBuf,
}

impl Default for FileRunAttachmentStore {
    fn default() -> Self {
        Self {
            staging_root: default_staging_root(),
        }
    }
}

impl FileRunAttachmentStore {
    #[cfg(test)]
    fn with_staging_root(staging_root: PathBuf) -> Self {
        Self { staging_root }
    }

    fn ingest_one(
        attachment_root: &Path,
        source_path: &Path,
        file_name: String,
        total_bytes: &mut u64,
        cleanup: &mut BatchCleanup,
    ) -> Result<ChatAttachmentRef, AttachmentError> {
        let metadata = fs::symlink_metadata(source_path).map_err(|error| {
            storage_error(&file_name, format!("could not inspect source ({error})"))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(AttachmentError::InvalidSource {
                file_name,
                reason: "symbolic links are not allowed",
            });
        }
        if !metadata.is_file() {
            return Err(AttachmentError::InvalidSource {
                file_name,
                reason: "select a regular file",
            });
        }
        if metadata.len() == 0 {
            return Err(AttachmentError::Empty { file_name });
        }
        if metadata.len() > MAX_CHAT_ATTACHMENT_BYTES {
            return Err(file_too_large(file_name));
        }
        if total_bytes.saturating_add(metadata.len()) > MAX_CHAT_ATTACHMENT_TOTAL_BYTES {
            return Err(total_too_large());
        }
        let format =
            format_for_path(source_path).ok_or_else(|| AttachmentError::UnsupportedType {
                file_name: file_name.clone(),
            })?;

        fs::create_dir_all(attachment_root)
            .map_err(|error| storage_error(&file_name, error.to_string()))?;
        let id = Uuid::new_v4().to_string();
        let final_path = attachment_root.join(format!("{id}.{}", format.extension));
        let temp_path = attachment_root.join(format!(".{id}.tmp"));
        cleanup.track(temp_path.clone());
        cleanup.track(final_path.clone());

        let source = File::open(source_path).map_err(|error| {
            storage_error(&file_name, format!("could not open source ({error})"))
        })?;
        let mut reader = BufReader::new(source);
        let mut target = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|error| storage_error(&file_name, error.to_string()))?;
        let mut hasher = Sha256::new();
        let mut bytes = Vec::with_capacity(
            usize::try_from(metadata.len().min(MAX_CHAT_ATTACHMENT_BYTES)).unwrap_or(0),
        );
        let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
        let mut copied = 0_u64;
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|error| storage_error(&file_name, error.to_string()))?;
            if read == 0 {
                break;
            }
            copied = copied.saturating_add(read as u64);
            if copied > MAX_CHAT_ATTACHMENT_BYTES {
                return Err(file_too_large(file_name));
            }
            if total_bytes.saturating_add(copied) > MAX_CHAT_ATTACHMENT_TOTAL_BYTES {
                return Err(total_too_large());
            }
            let chunk = &buffer[..read];
            hasher.update(chunk);
            bytes.extend_from_slice(chunk);
            target
                .write_all(chunk)
                .map_err(|error| storage_error(&file_name, error.to_string()))?;
        }
        if copied == 0 {
            return Err(AttachmentError::Empty { file_name });
        }
        validate_bytes(format, &bytes).map_err(|()| AttachmentError::TypeMismatch {
            file_name: file_name.clone(),
        })?;
        target
            .flush()
            .and_then(|()| target.sync_all())
            .map_err(|error| storage_error(&file_name, error.to_string()))?;
        drop(target);
        fs::rename(&temp_path, &final_path)
            .map_err(|error| storage_error(&file_name, error.to_string()))?;

        *total_bytes = total_bytes.saturating_add(copied);
        Ok(ChatAttachmentRef {
            id,
            file_name,
            media_type: format.media_type.to_string(),
            size_bytes: copied,
            sha256: format!("{:x}", hasher.finalize()),
            kind: format.kind,
        })
    }
}

impl RunAttachmentStore for FileRunAttachmentStore {
    fn stage(&self, file_name: &str, bytes: &[u8]) -> Result<StagedAttachment, AttachmentError> {
        let file_name = sanitized_file_name(Path::new(file_name));
        if bytes.is_empty() {
            return Err(AttachmentError::Empty { file_name });
        }
        if bytes.len() as u64 > MAX_CHAT_ATTACHMENT_BYTES {
            return Err(file_too_large(file_name));
        }
        let format = format_for_path(Path::new(&file_name)).ok_or_else(|| {
            AttachmentError::UnsupportedType {
                file_name: file_name.clone(),
            }
        })?;
        validate_bytes(format, bytes).map_err(|()| AttachmentError::TypeMismatch {
            file_name: file_name.clone(),
        })?;
        cleanup_stale_staging(&self.staging_root);
        fs::create_dir_all(&self.staging_root)
            .map_err(|error| storage_error(&file_name, error.to_string()))?;
        let stored_name = format!("{}--{file_name}", Uuid::new_v4());
        let final_path = self.staging_root.join(&stored_name);
        let temp_path = self.staging_root.join(format!(".{stored_name}.tmp"));
        let mut target = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp_path)
            .map_err(|error| storage_error(&file_name, error.to_string()))?;
        let write_result = target
            .write_all(bytes)
            .and_then(|()| target.flush())
            .and_then(|()| target.sync_all())
            .and_then(|()| fs::rename(&temp_path, &final_path));
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(storage_error(&file_name, error.to_string()));
        }
        Ok(StagedAttachment {
            token: format!("{STAGED_TOKEN_PREFIX}{stored_name}"),
            file_name,
            size_bytes: bytes.len() as u64,
            kind: format.kind,
        })
    }

    fn remove_staged(&self, token: &str) -> Result<(), AttachmentError> {
        let (path, file_name) = resolve_staged_token(&self.staging_root, token)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(storage_error(&file_name, error.to_string())),
        }
    }

    fn ingest_batch(
        &self,
        attachment_root: &Path,
        source_paths: &[PathBuf],
    ) -> Result<Vec<ChatAttachmentRef>, AttachmentError> {
        if source_paths.len() > MAX_CHAT_ATTACHMENTS {
            return Err(AttachmentError::TooMany {
                max: MAX_CHAT_ATTACHMENTS,
            });
        }
        let mut declared_total = 0_u64;
        let sources = source_paths
            .iter()
            .map(|source_path| resolve_source(&self.staging_root, source_path))
            .collect::<Result<Vec<_>, _>>()?;
        for (source_path, file_name) in &sources {
            if let Ok(metadata) = fs::symlink_metadata(source_path) {
                declared_total = declared_total.saturating_add(metadata.len());
                if declared_total > MAX_CHAT_ATTACHMENT_TOTAL_BYTES {
                    return Err(total_too_large());
                }
                if metadata.len() > MAX_CHAT_ATTACHMENT_BYTES {
                    return Err(file_too_large(file_name.clone()));
                }
            }
        }

        let mut cleanup = BatchCleanup::default();
        let mut total_bytes = 0_u64;
        let mut attachments = Vec::with_capacity(source_paths.len());
        for (source_path, file_name) in &sources {
            attachments.push(Self::ingest_one(
                attachment_root,
                source_path,
                file_name.clone(),
                &mut total_bytes,
                &mut cleanup,
            )?);
        }
        cleanup.commit();
        Ok(attachments)
    }

    fn read(
        &self,
        attachment_root: &Path,
        attachment: &ChatAttachmentRef,
    ) -> Result<Vec<u8>, AttachmentError> {
        let path = stored_path(attachment_root, attachment)?;
        let mut file = File::open(path).map_err(|_| AttachmentError::Corrupt {
            file_name: attachment.file_name.clone(),
        })?;
        let mut bytes = Vec::with_capacity(
            usize::try_from(attachment.size_bytes.min(MAX_CHAT_ATTACHMENT_BYTES)).unwrap_or(0),
        );
        std::io::Read::by_ref(&mut file)
            .take(MAX_CHAT_ATTACHMENT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| AttachmentError::Corrupt {
                file_name: attachment.file_name.clone(),
            })?;
        let actual_size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        let actual_hash = format!("{:x}", Sha256::digest(&bytes));
        let format = format_for_media_type(&attachment.media_type).ok_or_else(|| {
            AttachmentError::Corrupt {
                file_name: attachment.file_name.clone(),
            }
        })?;
        if actual_size != attachment.size_bytes
            || actual_size > MAX_CHAT_ATTACHMENT_BYTES
            || actual_hash != attachment.sha256
            || format.kind != attachment.kind
            || validate_bytes(format, &bytes).is_err()
        {
            return Err(AttachmentError::Corrupt {
                file_name: attachment.file_name.clone(),
            });
        }
        Ok(bytes)
    }

    fn preview(
        &self,
        attachment_root: &Path,
        attachment: &ChatAttachmentRef,
    ) -> Result<AttachmentPreview, AttachmentError> {
        if attachment.kind != ChatAttachmentKind::Image {
            return Err(AttachmentError::UnsupportedType {
                file_name: attachment.file_name.clone(),
            });
        }
        let bytes = self.read(attachment_root, attachment)?;
        let image = image::load_from_memory(&bytes).map_err(|_| AttachmentError::Corrupt {
            file_name: attachment.file_name.clone(),
        })?;
        let encoded = encode_bounded_preview(&image).ok_or_else(|| AttachmentError::Corrupt {
            file_name: attachment.file_name.clone(),
        })?;
        Ok(AttachmentPreview {
            media_type: "image/jpeg".to_string(),
            bytes: encoded,
        })
    }

    fn remove_batch(
        &self,
        attachment_root: &Path,
        attachments: &[ChatAttachmentRef],
    ) -> Result<(), AttachmentError> {
        for attachment in attachments {
            let path = stored_path(attachment_root, attachment)?;
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(storage_error(&attachment.file_name, error.to_string()));
                }
            }
        }
        Ok(())
    }
}

fn default_staging_root() -> PathBuf {
    super::run_checkpoint_store::FileRunCheckpointStore::app_runs_root().join(".attachment-staging")
}

fn cleanup_stale_staging(staging_root: &Path) {
    let Ok(entries) = fs::read_dir(staging_root) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        let stale = entry
            .metadata()
            .ok()
            .filter(|metadata| metadata.is_file())
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > STAGING_MAX_AGE);
        if stale {
            let _ = fs::remove_file(path);
        }
    }
}

fn resolve_source(
    staging_root: &Path,
    source: &Path,
) -> Result<(PathBuf, String), AttachmentError> {
    let Some(token) = source
        .to_str()
        .filter(|value| value.starts_with(STAGED_TOKEN_PREFIX))
    else {
        return Ok((source.to_path_buf(), sanitized_file_name(source)));
    };
    let (path, file_name) = resolve_staged_token(staging_root, token)?;
    Ok((path, file_name))
}

fn resolve_staged_token(
    staging_root: &Path,
    token: &str,
) -> Result<(PathBuf, String), AttachmentError> {
    let stored_name =
        token
            .strip_prefix(STAGED_TOKEN_PREFIX)
            .ok_or_else(|| AttachmentError::InvalidSource {
                file_name: "attachment".to_string(),
                reason: "invalid staged attachment token",
            })?;
    if Path::new(stored_name)
        .file_name()
        .and_then(|name| name.to_str())
        != Some(stored_name)
    {
        return Err(AttachmentError::InvalidSource {
            file_name: "attachment".to_string(),
            reason: "invalid staged attachment token",
        });
    }
    let (id, file_name) =
        stored_name
            .split_once("--")
            .ok_or_else(|| AttachmentError::InvalidSource {
                file_name: "attachment".to_string(),
                reason: "invalid staged attachment token",
            })?;
    Uuid::parse_str(id).map_err(|_| AttachmentError::InvalidSource {
        file_name: "attachment".to_string(),
        reason: "invalid staged attachment token",
    })?;
    if file_name.is_empty() {
        return Err(AttachmentError::InvalidSource {
            file_name: "attachment".to_string(),
            reason: "invalid staged attachment token",
        });
    }
    Ok((staging_root.join(stored_name), file_name.to_string()))
}

fn sanitized_file_name(path: &Path) -> String {
    let raw = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("attachment");
    let cleaned = raw
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "attachment".to_string()
    } else {
        trimmed.chars().take(255).collect()
    }
}

fn format_for_path(path: &Path) -> Option<AttachmentFormat> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    format_for_extension(&extension)
}

fn format_for_extension(extension: &str) -> Option<AttachmentFormat> {
    let (media_type, extension, kind) = match extension {
        "jpg" | "jpeg" => ("image/jpeg", "jpg", ChatAttachmentKind::Image),
        "png" => ("image/png", "png", ChatAttachmentKind::Image),
        "gif" => ("image/gif", "gif", ChatAttachmentKind::Image),
        "webp" => ("image/webp", "webp", ChatAttachmentKind::Image),
        "pdf" => ("application/pdf", "pdf", ChatAttachmentKind::Document),
        "txt" => ("text/plain", "txt", ChatAttachmentKind::Document),
        "md" | "markdown" => ("text/markdown", "md", ChatAttachmentKind::Document),
        "csv" => ("text/csv", "csv", ChatAttachmentKind::Document),
        "json" => ("application/json", "json", ChatAttachmentKind::Document),
        "html" | "htm" => ("text/html", "html", ChatAttachmentKind::Document),
        "css" => ("text/css", "css", ChatAttachmentKind::Document),
        "js" | "mjs" | "cjs" => ("application/javascript", "js", ChatAttachmentKind::Document),
        "py" => ("text/x-python", "py", ChatAttachmentKind::Document),
        _ => return None,
    };
    Some(AttachmentFormat {
        media_type,
        extension,
        kind,
    })
}

fn format_for_media_type(media_type: &str) -> Option<AttachmentFormat> {
    [
        "jpg", "png", "gif", "webp", "pdf", "txt", "md", "csv", "json", "html", "css", "js", "py",
    ]
    .into_iter()
    .filter_map(format_for_extension)
    .find(|format| format.media_type == media_type)
}

fn validate_bytes(format: AttachmentFormat, bytes: &[u8]) -> Result<(), ()> {
    match format.media_type {
        "image/jpeg" => {
            if !bytes.starts_with(&[0xff, 0xd8, 0xff]) {
                return Err(());
            }
            validate_decodable_image(bytes)
        }
        "image/png" => {
            if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                return Err(());
            }
            validate_decodable_image(bytes)
        }
        "image/gif" => {
            if !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
                return Err(());
            }
            validate_decodable_image(bytes)
        }
        "image/webp" => {
            if bytes.len() < 12 || !bytes.starts_with(b"RIFF") || &bytes[8..12] != b"WEBP" {
                return Err(());
            }
            validate_decodable_image(bytes)
        }
        "application/pdf" => bytes.starts_with(b"%PDF-").then_some(()).ok_or(()),
        _ => validate_text(bytes),
    }
}

fn validate_decodable_image(bytes: &[u8]) -> Result<(), ()> {
    let reader = ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| ())?;
    reader.decode().map(|_| ()).map_err(|_| ())
}

fn validate_text(bytes: &[u8]) -> Result<(), ()> {
    let text = std::str::from_utf8(bytes).map_err(|_| ())?;
    if text.as_bytes().contains(&0) {
        return Err(());
    }
    Ok(())
}

fn stored_path(
    attachment_root: &Path,
    attachment: &ChatAttachmentRef,
) -> Result<PathBuf, AttachmentError> {
    Uuid::parse_str(&attachment.id).map_err(|_| AttachmentError::Corrupt {
        file_name: attachment.file_name.clone(),
    })?;
    let format =
        format_for_media_type(&attachment.media_type).ok_or_else(|| AttachmentError::Corrupt {
            file_name: attachment.file_name.clone(),
        })?;
    Ok(attachment_root.join(format!("{}.{}", attachment.id, format.extension)))
}

fn encode_bounded_preview(image: &DynamicImage) -> Option<Vec<u8>> {
    const DIMENSIONS: [u32; 5] = [512, 448, 384, 320, 256];
    const QUALITIES: [u8; 4] = [82, 70, 55, 40];
    for dimension in DIMENSIONS {
        let bounded = image.thumbnail(
            dimension.min(ATTACHMENT_PREVIEW_MAX_DIMENSION),
            dimension.min(ATTACHMENT_PREVIEW_MAX_DIMENSION),
        );
        for quality in QUALITIES {
            let mut bytes = Vec::new();
            if JpegEncoder::new_with_quality(&mut bytes, quality)
                .encode_image(&bounded)
                .is_ok()
                && bytes.len() <= ATTACHMENT_PREVIEW_MAX_BYTES
            {
                return Some(bytes);
            }
        }
    }
    None
}

fn file_too_large(file_name: String) -> AttachmentError {
    AttachmentError::FileTooLarge {
        file_name,
        max_mib: MAX_CHAT_ATTACHMENT_BYTES / (1024 * 1024),
    }
}

fn total_too_large() -> AttachmentError {
    AttachmentError::TotalTooLarge {
        max_mib: MAX_CHAT_ATTACHMENT_TOTAL_BYTES / (1024 * 1024),
    }
}

fn storage_error(file_name: &str, detail: String) -> AttachmentError {
    AttachmentError::Storage {
        file_name: file_name.to_string(),
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::ports::RunAttachmentStore;
    use engine::ChatAttachmentKind;
    use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn write_png(path: &Path, width: u32, height: u32) {
        let image = RgbaImage::from_pixel(width, height, Rgba([20, 40, 60, 255]));
        image
            .save_with_format(path, ImageFormat::Png)
            .expect("write png");
    }

    fn write_jpeg(path: &Path) {
        let image = RgbImage::from_pixel(2, 2, Rgb([90, 30, 10]));
        image
            .save_with_format(path, ImageFormat::Jpeg)
            .expect("write jpeg");
    }

    fn source_paths(paths: &[&Path]) -> Vec<PathBuf> {
        paths.iter().map(|path| (*path).to_path_buf()).collect()
    }

    #[test]
    fn ingests_valid_image_with_safe_ref_hash_and_bounded_preview() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("family photo.png");
        let attachment_root = dir.path().join("run").join("attachments");
        write_png(&source, 900, 700);
        let store = FileRunAttachmentStore::default();

        let refs = store
            .ingest_batch(&attachment_root, &source_paths(&[&source]))
            .expect("ingest");
        let attachment = refs.first().expect("attachment");

        assert_eq!(attachment.file_name, "family photo.png");
        assert_eq!(attachment.media_type, "image/png");
        assert_eq!(attachment.kind, ChatAttachmentKind::Image);
        assert_eq!(attachment.sha256.len(), 64);
        assert!(!attachment.id.contains('/'));
        assert!(!serde_json::to_string(attachment)
            .expect("serialize")
            .contains(dir.path().to_string_lossy().as_ref()));

        let resolved = store
            .read(&attachment_root, attachment)
            .expect("read stored attachment");
        assert!(!resolved.is_empty());

        let preview = store
            .preview(&attachment_root, attachment)
            .expect("preview");
        assert_eq!(preview.media_type, "image/jpeg");
        assert!(preview.bytes.len() <= 512 * 1024);
        let decoded = image::load_from_memory(&preview.bytes).expect("decode preview");
        assert!(decoded.width() <= 512);
        assert!(decoded.height() <= 512);
    }

    #[test]
    fn stages_browser_bytes_behind_opaque_token_then_consumes_them() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source.png");
        write_png(&source, 2, 2);
        let bytes = fs::read(&source).expect("read png");
        let staging_root = dir.path().join("staging");
        let attachment_root = dir.path().join("attachments");
        let store = FileRunAttachmentStore::with_staging_root(staging_root.clone());

        let staged = store.stage("pasted image.png", &bytes).expect("stage");

        assert!(staged.token.starts_with(STAGED_TOKEN_PREFIX));
        assert!(!staged.token.contains(dir.path().to_string_lossy().as_ref()));
        assert_eq!(staged.file_name, "pasted image.png");
        let refs = store
            .ingest_batch(&attachment_root, &[PathBuf::from(&staged.token)])
            .expect("ingest staged");
        assert_eq!(refs[0].file_name, "pasted image.png");
        store
            .remove_staged(&staged.token)
            .expect("remove staged source");
        assert!(
            !staging_root.exists()
                || fs::read_dir(staging_root)
                    .expect("read staging root")
                    .next()
                    .is_none()
        );
    }

    #[test]
    fn preserves_batch_order_and_rolls_back_when_any_item_is_invalid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let first = dir.path().join("first.png");
        let second = dir.path().join("second.jpg");
        let invalid = dir.path().join("fake.png");
        let attachment_root = dir.path().join("attachments");
        write_png(&first, 1, 1);
        write_jpeg(&second);
        fs::write(&invalid, b"not an image").expect("invalid fixture");
        let store = FileRunAttachmentStore::default();

        let refs = store
            .ingest_batch(&attachment_root, &source_paths(&[&first, &second]))
            .expect("batch");
        assert_eq!(
            refs.iter()
                .map(|attachment| attachment.file_name.as_str())
                .collect::<Vec<_>>(),
            vec!["first.png", "second.jpg"]
        );
        store
            .remove_batch(&attachment_root, &refs)
            .expect("remove accepted batch");

        let error = store
            .ingest_batch(
                &attachment_root,
                &source_paths(&[&first, &second, &invalid]),
            )
            .expect_err("invalid batch");
        assert!(error.to_string().contains("fake.png"));
        assert!(
            !attachment_root.exists()
                || fs::read_dir(&attachment_root)
                    .expect("read attachment root")
                    .next()
                    .is_none()
        );
    }

    #[test]
    fn accepts_pdf_and_allowlisted_utf8_documents() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pdf = dir.path().join("brief.pdf");
        let markdown = dir.path().join("notes.md");
        let json = dir.path().join("data.json");
        fs::write(&pdf, b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\n%%EOF").expect("pdf");
        fs::write(&markdown, "Hello, OpenFlow.\n").expect("markdown");
        fs::write(&json, "{\"ok\":true}\n").expect("json");
        let attachment_root = dir.path().join("attachments");
        let store = FileRunAttachmentStore::default();

        let refs = store
            .ingest_batch(&attachment_root, &source_paths(&[&pdf, &markdown, &json]))
            .expect("documents");

        assert_eq!(refs[0].media_type, "application/pdf");
        assert_eq!(refs[1].media_type, "text/markdown");
        assert_eq!(refs[2].media_type, "application/json");
        assert!(refs
            .iter()
            .all(|attachment| attachment.kind == ChatAttachmentKind::Document));
    }

    #[test]
    fn rejects_count_mismatch_empty_binary_and_unsupported_sources() {
        let dir = tempfile::tempdir().expect("tempdir");
        let attachment_root = dir.path().join("attachments");
        let store = FileRunAttachmentStore::default();
        let mut images = Vec::new();
        for index in 0..5 {
            let path = dir.path().join(format!("{index}.png"));
            write_png(&path, 1, 1);
            images.push(path);
        }
        assert!(store.ingest_batch(&attachment_root, &images).is_err());

        let empty = dir.path().join("empty.txt");
        fs::write(&empty, b"").expect("empty");
        assert!(store
            .ingest_batch(&attachment_root, std::slice::from_ref(&empty))
            .is_err());

        let fake_pdf = dir.path().join("fake.pdf");
        fs::write(&fake_pdf, b"plain text").expect("fake pdf");
        assert!(store
            .ingest_batch(&attachment_root, std::slice::from_ref(&fake_pdf))
            .is_err());

        let binary = dir.path().join("binary.txt");
        fs::write(&binary, [0, 1, 2, 3]).expect("binary");
        assert!(store
            .ingest_batch(&attachment_root, std::slice::from_ref(&binary))
            .is_err());

        let docx = dir.path().join("report.docx");
        fs::write(&docx, b"PK\x03\x04").expect("docx");
        assert!(store
            .ingest_batch(&attachment_root, std::slice::from_ref(&docx))
            .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_leaf_symlinks_and_directories() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source.png");
        let link = dir.path().join("link.png");
        write_png(&source, 1, 1);
        symlink(&source, &link).expect("symlink");
        let attachment_root = dir.path().join("attachments");
        let store = FileRunAttachmentStore::default();

        assert!(store
            .ingest_batch(&attachment_root, std::slice::from_ref(&link))
            .is_err());
        assert!(store
            .ingest_batch(&attachment_root, &[dir.path().to_path_buf()])
            .is_err());
    }
}
