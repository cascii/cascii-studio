use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

const CFRAME_HEADER_SIZE: usize = 8;
const PACKED_CFRAME_HEADER_SIZE: usize = 12;

/// Summary of the bundled outputs written to disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameBundleArtifacts {
    pub frame_count: usize,
    pub width: u32,
    pub height: u32,
    pub frames_json_path: PathBuf,
    pub packed_cframes_path: PathBuf,
}

/// Errors returned while bundling a frame directory into web-friendly assets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameBundleError {
    DirectoryMissing { path: PathBuf },
    InvalidOutputStem,
    NoFramesFound { path: PathBuf },
    MissingColorFrame { frame_name: String },
    InvalidCframe { path: PathBuf, reason: String },
    TextMismatch { txt_path: PathBuf, cframe_path: PathBuf },
    ReadFailed { path: PathBuf, reason: String },
    WriteFailed { path: PathBuf, reason: String },
    JsonEncodeFailed { reason: String },
}

impl fmt::Display for FrameBundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectoryMissing { path } => {
                write!(f, "Frame directory does not exist: {}", path.display())
            }
            Self::InvalidOutputStem => write!(f, "Output stem must not be empty"),
            Self::NoFramesFound { path } => {
                write!(f, "No .txt or .cframe frames found in {}", path.display())
            }
            Self::MissingColorFrame { frame_name } => {
                write!(f, "Missing .cframe sidecar for frame {frame_name}")
            }
            Self::InvalidCframe { path, reason } => {
                write!(f, "Invalid cframe {}: {reason}", path.display())
            }
            Self::TextMismatch {
                txt_path,
                cframe_path,
            } => write!(
                f,
                "Text frame {} does not match decoded cframe text from {}",
                txt_path.display(),
                cframe_path.display()
            ),
            Self::ReadFailed { path, reason } => {
                write!(f, "Failed to read {}: {reason}", path.display())
            }
            Self::WriteFailed { path, reason } => {
                write!(f, "Failed to write {}: {reason}", path.display())
            }
            Self::JsonEncodeFailed { reason } => {
                write!(f, "Failed to encode bundled frame JSON: {reason}")
            }
        }
    }
}

impl std::error::Error for FrameBundleError {}

#[derive(Debug, Clone, Default)]
struct FrameVariant {
    txt_path: Option<PathBuf>,
    cframe_path: Option<PathBuf>,
}

/// Bundle a frame directory into `<stem>_frames.json` and `<stem>_cframes.bin`
/// inside the same directory.
///
/// The packed binary layout matches `cascii_core_view::parse_packed_cframes`:
/// `u32 frame_count`, `u32 width`, `u32 height`, then all frame payloads packed as
/// `(char, r, g, b)` bytes with the per-file `.cframe` headers removed.
pub fn bundle_frame_directory_in_place(
    input_dir: impl AsRef<Path>,
    output_stem: &str,
) -> Result<FrameBundleArtifacts, FrameBundleError> {
    let input_dir = input_dir.as_ref();
    bundle_frame_directory(input_dir, input_dir, output_stem)
}

/// Bundle a frame directory into `<stem>_frames.json` and `<stem>_cframes.bin`
/// inside `output_dir`.
///
/// This helper always writes both outputs. Each logical frame therefore requires a
/// `.cframe` file. When a matching `.txt` file is missing, the plain text frame is
/// reconstructed from the `.cframe` data.
pub fn bundle_frame_directory(
    input_dir: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
    output_stem: &str,
) -> Result<FrameBundleArtifacts, FrameBundleError> {
    let input_dir = input_dir.as_ref();
    let output_dir = output_dir.as_ref();

    if !input_dir.is_dir() {
        return Err(FrameBundleError::DirectoryMissing {
            path: input_dir.to_path_buf(),
        });
    }

    if output_stem.trim().is_empty() {
        return Err(FrameBundleError::InvalidOutputStem);
    }

    let frames = collect_frame_variants(input_dir)?;

    fs::create_dir_all(output_dir).map_err(|error| FrameBundleError::WriteFailed {
        path: output_dir.to_path_buf(),
        reason: error.to_string(),
    })?;

    let mut text_frames = Vec::with_capacity(frames.len());
    let mut packed_frame_bytes = Vec::new();
    let mut expected_dims: Option<(u32, u32)> = None;

    for (frame_name, variant) in frames {
        let cframe_path = variant.cframe_path.clone().ok_or_else(|| FrameBundleError::MissingColorFrame {frame_name: frame_name.clone()})?;
        let cframe_bytes = fs::read(&cframe_path).map_err(|error| FrameBundleError::ReadFailed {path: cframe_path.clone(), reason: error.to_string()})?;
        let (width, height, payload) = parse_cframe_payload(&cframe_path, &cframe_bytes)?;
        match expected_dims {
            Some((expected_width, expected_height)) if expected_width != width || expected_height != height => {
                return Err(FrameBundleError::InvalidCframe {
                    path: cframe_path, reason: format!("frame dimensions {}x{} do not match expected {}x{}", width, height, expected_width, expected_height)});
            }
            None => expected_dims = Some((width, height)),
            _ => {}
        }

        let decoded_cframe_text = cascii_core_view::parse_cframe_text(&cframe_bytes).map_err(|error| FrameBundleError::InvalidCframe {path: cframe_path.clone(), reason: error.to_string()})?;
        let text_frame = if let Some(txt_path) = variant.txt_path {
            let txt_content = fs::read_to_string(&txt_path).map_err(|error| FrameBundleError::ReadFailed {path: txt_path.clone(), reason: error.to_string()})?;
            let normalized = normalize_frame_text(txt_content);
            if normalized != decoded_cframe_text {
                return Err(FrameBundleError::TextMismatch {txt_path, cframe_path});
            }
            normalized
        } else {
            decoded_cframe_text
        };

        text_frames.push(text_frame);
        packed_frame_bytes.extend_from_slice(payload);
    }

    let (width, height) = expected_dims.expect("frame list should not be empty");
    let frames_json_path = output_dir.join(format!("{output_stem}_frames.json"));
    let packed_cframes_path = output_dir.join(format!("{output_stem}_cframes.bin"));
    let json_bytes = serde_json::to_vec(&text_frames).map_err(|error| FrameBundleError::JsonEncodeFailed {reason: error.to_string()})?;

    let frame_count = text_frames.len() as u32;
    let mut packed_blob = Vec::with_capacity(PACKED_CFRAME_HEADER_SIZE + packed_frame_bytes.len());
    packed_blob.extend_from_slice(&frame_count.to_le_bytes());
    packed_blob.extend_from_slice(&width.to_le_bytes());
    packed_blob.extend_from_slice(&height.to_le_bytes());
    packed_blob.extend_from_slice(&packed_frame_bytes);

    fs::write(&frames_json_path, json_bytes).map_err(|error| FrameBundleError::WriteFailed {path: frames_json_path.clone(), reason: error.to_string()})?;
    fs::write(&packed_cframes_path, packed_blob).map_err(|error| FrameBundleError::WriteFailed {path: packed_cframes_path.clone(), reason: error.to_string()})?;

    Ok(FrameBundleArtifacts {frame_count: text_frames.len(), width, height, frames_json_path, packed_cframes_path})
}

fn collect_frame_variants(input_dir: &Path) -> Result<Vec<(String, FrameVariant)>, FrameBundleError> {
    let mut variants: BTreeMap<(u32, String), FrameVariant> = BTreeMap::new();
    let mut fallback_index = 0u32;
    let entries = fs::read_dir(input_dir).map_err(|error| FrameBundleError::ReadFailed {path: input_dir.to_path_buf(), reason: error.to_string()})?;

    for entry in entries {
        let entry = entry.map_err(|error| FrameBundleError::ReadFailed {
            path: input_dir.to_path_buf(),
            reason: error.to_string(),
        })?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        let normalized_ext = extension.to_ascii_lowercase();
        if normalized_ext != "txt" && normalized_ext != "cframe" {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };

        let index = extract_index(stem, fallback_index);
        fallback_index = fallback_index.saturating_add(1);

        let variant = variants.entry((index, stem.to_string())).or_default();
        if normalized_ext == "txt" {
            variant.txt_path = Some(path);
        } else {
            variant.cframe_path = Some(path);
        }
    }

    if variants.is_empty() {
        return Err(FrameBundleError::NoFramesFound {path: input_dir.to_path_buf()});
    }

    Ok(variants
        .into_iter()
        .map(|((_index, stem), variant)| (stem, variant))
        .collect())
}

fn extract_index(stem: &str, fallback: u32) -> u32 {
    if let Some(suffix) = stem.strip_prefix("frame_") {
        suffix.parse::<u32>().unwrap_or(0)
    } else {
        let digits = stem
            .chars()
            .filter(|character| character.is_ascii_digit())
            .collect::<String>();
        digits.parse::<u32>().unwrap_or(fallback)
    }
}

fn parse_cframe_payload<'a>(path: &Path,bytes: &'a [u8]) -> Result<(u32, u32, &'a [u8]), FrameBundleError> {
    if bytes.len() < CFRAME_HEADER_SIZE {
        return Err(FrameBundleError::InvalidCframe {path: path.to_path_buf(), reason: format!("expected at least {} bytes, found {}", CFRAME_HEADER_SIZE, bytes.len())});
    }

    let width = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let height = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);

    if width == 0 || height == 0 {
        return Err(FrameBundleError::InvalidCframe {
            path: path.to_path_buf(),
            reason: format!("invalid frame dimensions {}x{}", width, height),
        });
    }

    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| FrameBundleError::InvalidCframe {
            path: path.to_path_buf(),
            reason: "frame dimensions overflowed usize while validating payload".to_string(),
        })?;
    let payload_len = pixel_count
        .checked_mul(4)
        .ok_or_else(|| FrameBundleError::InvalidCframe {
            path: path.to_path_buf(),
            reason: "frame payload length overflowed usize".to_string(),
        })?;
    let expected_size = CFRAME_HEADER_SIZE
        .checked_add(payload_len)
        .ok_or_else(|| FrameBundleError::InvalidCframe {
            path: path.to_path_buf(),
            reason: "frame file size overflowed usize".to_string(),
        })?;

    if bytes.len() != expected_size {
        return Err(FrameBundleError::InvalidCframe {
            path: path.to_path_buf(),
            reason: format!(
                "expected {} bytes for {}x{}, found {}",
                expected_size,
                width,
                height,
                bytes.len()
            ),
        });
    }

    Ok((width, height, &bytes[CFRAME_HEADER_SIZE..]))
}

fn normalize_frame_text(text: String) -> String {
    let mut normalized = text.replace("\r\n", "\n");
    if !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("cascii-studio-frame-bundle-{label}-{unique}"))
    }

    fn write_cframe(path: &Path, width: u32, height: u32, pixels: &[(u8, u8, u8, u8)]) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        for (ch, r, g, b) in pixels {
            bytes.push(*ch);
            bytes.push(*r);
            bytes.push(*g);
            bytes.push(*b);
        }
        fs::write(path, bytes).expect("cframe should be written");
    }

    #[test]
    fn bundles_txt_and_cframes_in_place() {
        let dir = test_dir("paired");
        fs::create_dir_all(&dir).expect("temp dir should exist");

        fs::write(dir.join("frame_0001.txt"), "AB\n").expect("txt frame should be written");
        fs::write(dir.join("frame_0002.txt"), "CD\n").expect("txt frame should be written");
        write_cframe(
            &dir.join("frame_0001.cframe"),
            2,
            1,
            &[(b'A', 255, 0, 0), (b'B', 0, 255, 0)],
        );
        write_cframe(
            &dir.join("frame_0002.cframe"),
            2,
            1,
            &[(b'C', 0, 0, 255), (b'D', 255, 255, 0)],
        );

        let bundle = bundle_frame_directory_in_place(&dir, "clip").expect("bundle should succeed");
        assert_eq!(bundle.frame_count, 2);
        assert_eq!(bundle.width, 2);
        assert_eq!(bundle.height, 1);

        let json_bytes = fs::read(&bundle.frames_json_path).expect("json bundle should exist");
        let frames: Vec<String> = serde_json::from_slice(&json_bytes).expect("json should decode");
        assert_eq!(frames, vec!["AB\n".to_string(), "CD\n".to_string()]);

        let blob = fs::read(&bundle.packed_cframes_path).expect("packed bundle should exist");
        assert_eq!(u32::from_le_bytes(blob[0..4].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(blob[4..8].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(blob[8..12].try_into().unwrap()), 1);
        assert_eq!(blob.len(), 12 + (2 * 2 * 1 * 4));

        fs::remove_dir_all(&dir).expect("temp dir should be removed");
    }

    #[test]
    fn derives_text_frames_from_cframes_when_txt_is_missing() {
        let dir = test_dir("cframe-only");
        let output_dir = dir.join("bundles");
        fs::create_dir_all(&dir).expect("temp dir should exist");

        write_cframe(
            &dir.join("frame_0001.cframe"),
            2,
            1,
            &[(b'X', 12, 34, 56), (b'Y', 78, 90, 123)],
        );

        let bundle = bundle_frame_directory(&dir, &output_dir, "single").expect("bundle should succeed");
        let frames: Vec<String> = serde_json::from_slice(
            &fs::read(&bundle.frames_json_path).expect("json bundle should exist"),
        )
        .expect("json should decode");
        assert_eq!(frames, vec!["XY\n".to_string()]);

        fs::remove_dir_all(&dir).expect("temp dir should be removed");
    }
}
