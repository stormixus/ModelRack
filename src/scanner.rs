use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct ScanResult {
    pub entries: Vec<StlFileInfo>,
    pub meshes: Vec<MeshData>,
    pub skipped: usize,
}

/// Parsed mesh data for thumbnail generation
pub struct MeshData {
    pub hash: [u8; 32],
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<[u32; 3]>,
}

pub struct StlFileInfo {
    pub path: PathBuf,
    pub filename: String,
    pub size: u64,
    pub hash: [u8; 32],
    pub stl_type: StlType,
    pub triangle_count: Option<usize>,
    pub dimensions: Option<[f32; 3]>,
}

pub enum StlType {
    Binary,
    Ascii,
    Unknown,
}

pub fn scan_folder(path: &Path) -> ScanResult {
    let mut entries = Vec::new();
    let mut meshes = Vec::new();
    let mut walk_errors = 0usize;
    let mut parse_errors = 0usize;

    for entry in WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| match e {
            Ok(entry) => Some(entry),
            Err(err) => {
                eprintln!("WalkDir error (skipping): {}", err);
                walk_errors += 1;
                None
            }
        })
    {
        if !entry.file_type().is_file() {
            continue;
        }

        let file_path = entry.path();
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase());

        match ext.as_deref() {
            Some("stl") => match parse_stl_file(file_path) {
                Ok((info, mesh_opt)) => {
                    if let Some(mesh) = mesh_opt {
                        meshes.push(mesh);
                    }
                    entries.push(info);
                }
                Err(err) => {
                    eprintln!("Parse error for {}: {}", file_path.display(), err);
                    parse_errors += 1;
                }
            },
            _ => {}
        }
    }

    let skipped = walk_errors + parse_errors;

    // Sort by filename for consistent display
    entries.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));

    ScanResult { entries, meshes, skipped }
}

fn parse_stl_file(path: &Path) -> Result<(StlFileInfo, Option<MeshData>)> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;

    let size = metadata.len();
    if size > 100 * 1024 * 1024 {
        anyhow::bail!(
            "File exceeds 100MB size cap: {} ({:.1}MB)",
            path.display(),
            size as f64 / (1024.0 * 1024.0)
        );
    }

    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    // blake3 hash for identity
    let hash_bytes: [u8; 32] = blake3::hash(&data).into();

    // read_stl handles both binary and ASCII; requires Seek so wrap in Cursor
    let (stl_type, triangle_count, dimensions, mesh_data) = match stl_io::read_stl(&mut std::io::Cursor::new(&data)) {
        Ok(indexed_mesh) => {
            let dims = compute_dimensions(&indexed_mesh.vertices);
            let st = detect_stl_type(&data);
            let tris = indexed_mesh.faces.len();
            let verts: Vec<[f32; 3]> = indexed_mesh.vertices.iter().map(|v| [v[0], v[1], v[2]]).collect();
            let faces: Vec<[u32; 3]> = indexed_mesh.faces.iter().map(|f| {
                [f.vertices[0] as u32, f.vertices[1] as u32, f.vertices[2] as u32]
            }).collect();
            let md = MeshData {
                hash: hash_bytes,
                vertices: verts,
                faces,
            };
            (st, Some(tris), dims, Some(md))
        }
        Err(_) => {
            (StlType::Unknown, None, None, None)
        }
    };

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok((StlFileInfo {
        path: path.to_path_buf(),
        filename,
        size,
        hash: hash_bytes,
        stl_type,
        triangle_count,
        dimensions,
    }, mesh_data))
}

fn detect_stl_type(data: &[u8]) -> StlType {
    // Heuristic: ASCII STL starts with "solid " (5 bytes)
    if data.len() >= 5 && &data[..5] == b"solid" {
        StlType::Ascii
    } else {
        StlType::Binary
    }
}

fn compute_dimensions(vertices: &[stl_io::Vertex]) -> Option<[f32; 3]> {
    if vertices.is_empty() {
        return None;
    }

    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];

    for v in vertices {
        min[0] = min[0].min(v[0]);
        min[1] = min[1].min(v[1]);
        min[2] = min[2].min(v[2]);
        max[0] = max[0].max(v[0]);
        max[1] = max[1].max(v[1]);
        max[2] = max[2].max(v[2]);
    }

    Some([max[0] - min[0], max[1] - min[1], max[2] - min[2]])
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
}
