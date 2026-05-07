use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const MAX_STL_PARSE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_STL_PREVIEW_BYTES: u64 = 5 * 1024 * 1024;
type ParsedStl = (StlType, Option<usize>, Option<[f32; 3]>, Option<MeshData>);

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct SidecarMeta {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub printed: u32,
    #[serde(default)]
    pub print_history: Vec<PrintRecord>,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub added: Option<String>,
    #[serde(skip, default)]
    pub tag_input: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PrintRecord {
    pub date: String,
    #[serde(default)]
    pub material: String,
    #[serde(default = "default_success")]
    pub success: bool,
    #[serde(default)]
    pub notes: String,
}

fn default_success() -> bool {
    true
}

#[cfg(test)]
pub struct ScanResult {
    pub entries: Vec<StlFileInfo>,
    pub meshes: Vec<MeshData>,
    pub skipped: usize,
}

pub enum ScanEvent {
    Progress {
        scanned: usize,
        skipped: usize,
        current: String,
    },
    Entry {
        info: Box<StlFileInfo>,
        mesh: Option<MeshData>,
    },
    Done {
        skipped: usize,
    },
}

/// Parsed mesh data for thumbnail generation
#[derive(Clone)]
pub struct MeshData {
    pub hash: [u8; 32],
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<[u32; 3]>,
}

#[derive(Clone)]
pub struct StlFileInfo {
    pub path: PathBuf,
    pub filename: String,
    pub size: u64,
    pub hash: [u8; 32],
    pub stl_type: StlType,
    pub triangle_count: Option<usize>,
    pub dimensions: Option<[f32; 3]>,
    pub modified: Option<std::time::SystemTime>,
    pub meta: Option<SidecarMeta>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StlType {
    Binary,
    Ascii,
    ThreeMf,
    Obj,
    Step,
    LargeStl,
    Unknown,
}

#[cfg(test)]
pub fn scan_folder(path: &Path) -> ScanResult {
    let mut entries = Vec::new();
    let mut meshes = Vec::new();
    let mut skipped = 0usize;

    let (tx, rx) = crossbeam_channel::unbounded();
    scan_folder_stream(path, tx);
    for event in rx {
        match event {
            ScanEvent::Progress { .. } => {}
            ScanEvent::Entry { info, mesh } => {
                if let Some(mesh) = mesh {
                    meshes.push(mesh);
                }
                entries.push(*info);
            }
            ScanEvent::Done {
                skipped: done_skipped,
            } => {
                skipped = done_skipped;
                break;
            }
        }
    }

    entries.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));

    ScanResult {
        entries,
        meshes,
        skipped,
    }
}

pub fn scan_folder_stream(path: &Path, tx: crossbeam_channel::Sender<ScanEvent>) {
    let mut walk_errors = 0usize;
    let mut parse_errors = 0usize;
    let mut scanned = 0usize;

    for entry_result in WalkDir::new(path).follow_links(false) {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(err) => {
                eprintln!("WalkDir error (skipping): {}", err);
                walk_errors += 1;
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let file_path = entry.path();
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase());

        if let Some(ext) = ext.as_deref().filter(|ext| is_supported_model_ext(ext)) {
            scanned += 1;
            let _ = tx.send(ScanEvent::Progress {
                scanned,
                skipped: walk_errors + parse_errors,
                current: file_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("model file")
                    .to_string(),
            });
            match parse_supported_file(file_path, ext) {
                Ok((info, mesh_opt)) => {
                    let _ = tx.send(ScanEvent::Entry {
                        info: Box::new(info),
                        mesh: mesh_opt,
                    });
                }
                Err(err) => {
                    eprintln!("Parse error for {}: {}", file_path.display(), err);
                    parse_errors += 1;
                    let _ = tx.send(ScanEvent::Progress {
                        scanned,
                        skipped: walk_errors + parse_errors,
                        current: file_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("model file")
                            .to_string(),
                    });
                }
            }
        }
    }

    let skipped = walk_errors + parse_errors;
    let _ = tx.send(ScanEvent::Done { skipped });
}

fn is_supported_model_ext(ext: &str) -> bool {
    matches!(ext, "stl" | "3mf" | "obj" | "step" | "stp")
}

fn parse_supported_file(path: &Path, ext: &str) -> Result<(StlFileInfo, Option<MeshData>)> {
    match ext {
        "stl" => parse_stl_file(path),
        "3mf" => metadata_only_file(path, StlType::ThreeMf).map(|info| (info, None)),
        "obj" => metadata_only_file(path, StlType::Obj).map(|info| (info, None)),
        "step" | "stp" => metadata_only_file(path, StlType::Step).map(|info| (info, None)),
        _ => anyhow::bail!("Unsupported model format: {}", ext),
    }
}

fn parse_stl_file(path: &Path) -> Result<(StlFileInfo, Option<MeshData>)> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;

    let size = metadata.len();
    let modified = metadata.modified().ok();
    if size > MAX_STL_PARSE_BYTES {
        return metadata_only_file(path, StlType::LargeStl).map(|info| (info, None));
    }

    let data = std::fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;

    // blake3 hash for identity
    let hash_bytes: [u8; 32] = blake3::hash(&data).into();

    let (stl_type, triangle_count, dimensions, mesh_data) = if size > MAX_STL_PREVIEW_BYTES {
        (
            StlType::LargeStl,
            binary_stl_triangle_count(&data),
            None,
            None,
        )
    } else {
        parse_binary_stl_fast(&data, hash_bytes).unwrap_or_else(|| {
            let stl_type = if detect_stl_type(&data) == StlType::Ascii {
                StlType::Ascii
            } else {
                StlType::Unknown
            };
            (stl_type, None, None, None)
        })
    };

    // Read sidecar metadata if present
    let meta = read_sidecar(path);

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok((
        StlFileInfo {
            path: path.to_path_buf(),
            filename,
            size,
            hash: hash_bytes,
            stl_type,
            triangle_count,
            dimensions,
            modified,
            meta,
        },
        mesh_data,
    ))
}

fn metadata_only_file(path: &Path, stl_type: StlType) -> Result<StlFileInfo> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;
    let size = metadata.len();
    let modified = metadata.modified().ok();
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(StlFileInfo {
        path: path.to_path_buf(),
        filename,
        size,
        hash: metadata_hash(path, size, modified),
        stl_type,
        triangle_count: None,
        dimensions: None,
        modified,
        meta: read_sidecar(path),
    })
}

fn metadata_hash(path: &Path, size: u64, modified: Option<std::time::SystemTime>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(path.as_os_str().as_encoded_bytes());
    hasher.update(&size.to_le_bytes());
    if let Some(modified) =
        modified.and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
    {
        hasher.update(&modified.as_secs().to_le_bytes());
        hasher.update(&modified.subsec_nanos().to_le_bytes());
    }
    hasher.finalize().into()
}

fn read_sidecar(stl_path: &Path) -> Option<SidecarMeta> {
    let sidecar_path = sidecar_path(stl_path);
    let data = std::fs::read_to_string(&sidecar_path).ok()?;
    match serde_json::from_str::<SidecarMeta>(&data) {
        Ok(meta) => Some(meta),
        Err(e) => {
            eprintln!(
                "Warning: failed to parse sidecar for {}: {}",
                stl_path.display(),
                e
            );
            None
        }
    }
}

pub fn write_sidecar(stl_path: &Path, meta: &SidecarMeta) -> Result<()> {
    let sidecar_path = sidecar_path(stl_path);
    let json = serde_json::to_string_pretty(meta)
        .with_context(|| format!("Failed to serialize metadata for {}", stl_path.display()))?;
    let tmp_path = sidecar_path.with_extension("tmp");
    std::fs::write(&tmp_path, json).with_context(|| {
        format!(
            "Failed to write sidecar temp file for {}",
            stl_path.display()
        )
    })?;
    std::fs::rename(&tmp_path, &sidecar_path)
        .with_context(|| format!("Failed to rename sidecar for {}", stl_path.display()))?;
    Ok(())
}

fn sidecar_path(model_path: &Path) -> PathBuf {
    let file_name = model_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("model");
    model_path.with_file_name(format!("{}.modelrack.json", file_name))
}

fn parse_binary_stl_fast(data: &[u8], hash: [u8; 32]) -> Option<ParsedStl> {
    if data.len() < 84 {
        return None;
    }

    let triangle_count = u32::from_le_bytes(data[80..84].try_into().ok()?) as usize;
    let expected_len = 84usize.checked_add(triangle_count.checked_mul(50)?)?;
    if expected_len != data.len() {
        return None;
    }

    let vertex_count = triangle_count.checked_mul(3)?;
    if vertex_count > u32::MAX as usize {
        return Some((StlType::LargeStl, Some(triangle_count), None, None));
    }

    let mut vertices = Vec::with_capacity(vertex_count);
    let mut faces = Vec::with_capacity(triangle_count);
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];

    let mut offset = 84usize;
    for triangle_index in 0..triangle_count {
        offset += 12; // normal
        let base = (triangle_index * 3) as u32;
        for vertex_index in 0..3 {
            let start = offset + vertex_index * 12;
            let vertex = [
                read_f32_le(data, start)?,
                read_f32_le(data, start + 4)?,
                read_f32_le(data, start + 8)?,
            ];
            if !vertex.iter().all(|value| value.is_finite()) {
                return None;
            }
            for axis in 0..3 {
                min[axis] = min[axis].min(vertex[axis]);
                max[axis] = max[axis].max(vertex[axis]);
            }
            vertices.push(vertex);
        }
        faces.push([base, base + 1, base + 2]);
        offset += 50;
    }

    let dimensions = if vertices.is_empty() {
        None
    } else {
        Some([max[0] - min[0], max[1] - min[1], max[2] - min[2]])
    };

    Some((
        StlType::Binary,
        Some(triangle_count),
        dimensions,
        Some(MeshData {
            hash,
            vertices,
            faces,
        }),
    ))
}

fn binary_stl_triangle_count(data: &[u8]) -> Option<usize> {
    if data.len() < 84 {
        return None;
    }

    let triangle_count = u32::from_le_bytes(data[80..84].try_into().ok()?) as usize;
    let expected_len = 84usize.checked_add(triangle_count.checked_mul(50)?)?;
    (expected_len == data.len()).then_some(triangle_count)
}

fn read_f32_le(data: &[u8], start: usize) -> Option<f32> {
    Some(f32::from_le_bytes(
        data.get(start..start + 4)?.try_into().ok()?,
    ))
}

fn detect_stl_type(data: &[u8]) -> StlType {
    // Heuristic: ASCII STL starts with "solid " (5 bytes)
    if data.len() >= 5 && &data[..5] == b"solid" {
        StlType::Ascii
    } else {
        StlType::Binary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_binary_stl(triangles: &[[[f32; 3]; 3]]) -> Vec<u8> {
        let mut buf = Vec::new();
        // 80-byte header
        buf.extend_from_slice(&[0u8; 80]);
        // triangle count (u32 LE)
        buf.extend_from_slice(&(triangles.len() as u32).to_le_bytes());
        for tri in triangles {
            // normal (ignored, 12 bytes)
            buf.extend_from_slice(&[0u8; 12]);
            // vertices
            for v in tri {
                buf.extend_from_slice(&v[0].to_le_bytes());
                buf.extend_from_slice(&v[1].to_le_bytes());
                buf.extend_from_slice(&v[2].to_le_bytes());
            }
            // attribute byte count
            buf.extend_from_slice(&0u16.to_le_bytes());
        }
        buf
    }

    #[test]
    fn valid_binary_stl_parses() {
        let stl = make_binary_stl(&[
            [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
        ]);
        let result = stl_io::read_stl(&mut std::io::Cursor::new(&stl[..]));
        assert!(result.is_ok());
    }

    #[test]
    fn corrupt_data_skips() {
        // Not valid STL data
        let result = stl_io::read_stl(&mut std::io::Cursor::new(&b"not valid stl data"[..]));
        assert!(result.is_err());
    }

    #[test]
    fn empty_folder_scan() {
        let dir = std::env::temp_dir().join("modelrack-test-empty");
        let _ = std::fs::create_dir(&dir);
        let result = scan_folder(&dir);
        let _ = std::fs::remove_dir(&dir);
        assert_eq!(result.entries.len(), 0);
        assert_eq!(result.skipped, 0);
    }

    #[test]
    fn non_existent_folder_no_panic() {
        let result = scan_folder(std::path::Path::new("/tmp/modelrack-nonexistent-12345"));
        // walkdir just returns zero results for non-existent paths
        assert_eq!(result.entries.len(), 0);
    }

    #[test]
    fn sidecar_metadata_round_trips_through_scan() {
        let dir =
            std::env::temp_dir().join(format!("modelrack-sidecar-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();

        let stl_path = dir.join("bracket.stl");
        let stl = make_binary_stl(&[[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]]);
        std::fs::write(&stl_path, stl).unwrap();

        let meta = SidecarMeta {
            tags: vec!["fixture".to_string(), "shop".to_string()],
            notes: "Print in PETG".to_string(),
            favorite: true,
            printed: 2,
            ..Default::default()
        };
        write_sidecar(&stl_path, &meta).unwrap();

        let result = scan_folder(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        let loaded = result.entries[0].meta.as_ref().unwrap();
        assert_eq!(loaded.tags, vec!["fixture", "shop"]);
        assert_eq!(loaded.notes, "Print in PETG");
        assert!(loaded.favorite);
        assert_eq!(loaded.printed, 2);
    }

    #[test]
    fn scan_includes_metadata_only_3mf_files() {
        let dir = std::env::temp_dir().join(format!("modelrack-3mf-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("plate.3mf"), b"not parsed yet").unwrap();

        let result = scan_folder(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].filename, "plate.3mf");
        assert!(matches!(result.entries[0].stl_type, StlType::ThreeMf));
        assert!(result.meshes.is_empty());
    }

    #[test]
    fn oversized_stl_still_appears_as_metadata_only_entry() {
        let dir =
            std::env::temp_dir().join(format!("modelrack-large-stl-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("huge.stl");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_STL_PARSE_BYTES + 1).unwrap();
        drop(file);

        let result = scan_folder(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].filename, "huge.stl");
        assert!(matches!(result.entries[0].stl_type, StlType::LargeStl));
        assert!(result.meshes.is_empty());
        assert_eq!(result.skipped, 0);
    }

    #[test]
    fn ascii_stl_appears_without_mesh_preview() {
        let dir =
            std::env::temp_dir().join(format!("modelrack-ascii-stl-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("note.stl");
        std::fs::write(
            &path,
            b"solid note\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid note\n",
        )
        .unwrap();

        let result = scan_folder(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        assert!(matches!(result.entries[0].stl_type, StlType::Ascii));
        assert!(result.meshes.is_empty());
    }

    #[test]
    fn unsupported_extensions_are_not_counted_as_scan_errors() {
        let dir =
            std::env::temp_dir().join(format!("modelrack-ignore-ext-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("readme.txt"), b"notes").unwrap();
        std::fs::write(dir.join("plate.3mf"), b"placeholder").unwrap();

        let result = scan_folder(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.skipped, 0);
    }
}
