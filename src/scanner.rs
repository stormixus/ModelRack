use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const MAX_STL_PARSE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_STL_PREVIEW_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TEXT_PREVIEW_BYTES: u64 = 16 * 1024 * 1024;
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
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PrintRecord {
    pub date: String,
    #[serde(default)]
    pub material: String,
    #[serde(default)]
    pub printer: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub nozzle: String,
    #[serde(default)]
    pub layer_height: String,
    #[serde(default)]
    pub duration: String,
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

/// Parsed mesh data for thumbnail/detail-preview generation.
///
/// Renderers may defensively call [`MeshData::compacted`] before projection.
/// Compaction keeps finite vertices, remaps faces from source-file indices to
/// compact indices, and drops entire faces that reference invalid or missing
/// vertices.
#[derive(Clone)]
pub struct MeshData {
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<[u32; 3]>,
}

impl MeshData {
    pub(crate) fn compacted(&self) -> Option<Self> {
        let vertices = self.vertices.iter().copied().map(Some).collect::<Vec<_>>();
        compact_mesh(&vertices, &self.faces)
    }
}

#[derive(Clone)]
pub struct ThreeMfPlate {
    pub label: String,
    pub mesh: MeshData,
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
    pub three_mf_plate_count: Option<usize>,
    pub modified: Option<std::time::SystemTime>,
    pub thumbnail_path: Option<PathBuf>,
    pub meta: Option<SidecarMeta>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StlType {
    Binary,
    Ascii,
    ThreeMf,
    Obj,
    Step,
    Scad,
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
            ScanEvent::Progress {
                scanned,
                skipped,
                current,
            } => {
                let _ = (scanned, skipped, current);
            }
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

pub(crate) fn is_supported_model_ext(ext: &str) -> bool {
    matches!(ext, "stl" | "3mf" | "obj" | "step" | "stp" | "scad")
}

fn parse_supported_file(path: &Path, ext: &str) -> Result<(StlFileInfo, Option<MeshData>)> {
    match ext {
        "stl" => parse_stl_file(path),
        "3mf" => parse_three_mf_file(path),
        "obj" => parse_obj_file(path),
        "step" | "stp" => parse_step_file(path),
        "scad" => parse_scad_file(path),
        _ => anyhow::bail!("Unsupported model format: {}", ext),
    }
}

pub(crate) fn parse_preview_mesh(path: &Path) -> Result<Option<MeshData>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !is_supported_model_ext(&ext) {
        return Ok(None);
    }
    if ext == "3mf" {
        return parse_three_mf_plates(path).map(|plates| {
            plates.and_then(|plates| plates.into_iter().next().map(|plate| plate.mesh))
        });
    }
    parse_supported_file(path, &ext).map(|(_, mesh)| mesh)
}

pub(crate) fn parse_preview_plates(path: &Path) -> Result<Option<Vec<ThreeMfPlate>>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "3mf" {
        return Ok(None);
    }
    parse_three_mf_plates(path)
}

fn parse_three_mf_file(path: &Path) -> Result<(StlFileInfo, Option<MeshData>)> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;
    let size = metadata.len();
    let modified = metadata.modified().ok();
    if size > MAX_STL_PARSE_BYTES {
        return metadata_only_file(path, StlType::ThreeMf).map(|info| (info, None));
    }

    let data = std::fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let hash_bytes: [u8; 32] = blake3::hash(&data).into();
    let mut archive = match zip::ZipArchive::new(Cursor::new(data)) {
        Ok(archive) => archive,
        Err(_) => {
            return metadata_only_file(path, StlType::ThreeMf).map(|info| (info, None));
        }
    };

    let plates = parse_three_mf_plates_from_archive(&mut archive)?;

    let mesh_data = plates.first().map(|plate| plate.mesh.clone());
    let dimensions = mesh_data.as_ref().and_then(mesh_dimensions);
    let triangle_count = if plates.is_empty() {
        None
    } else {
        Some(plates.iter().map(|plate| plate.mesh.faces.len()).sum())
    };
    let three_mf_plate_count = (plates.len() > 1).then_some(plates.len());
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
            stl_type: StlType::ThreeMf,
            triangle_count,
            dimensions,
            three_mf_plate_count,
            modified,
            thumbnail_path: None,
            meta: read_sidecar(path),
        },
        mesh_data,
    ))
}

fn parse_three_mf_plates(path: &Path) -> Result<Option<Vec<ThreeMfPlate>>> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;
    if metadata.len() > MAX_STL_PARSE_BYTES {
        return Ok(None);
    }
    let data = std::fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let mut archive = match zip::ZipArchive::new(Cursor::new(data)) {
        Ok(archive) => archive,
        Err(_) => return Ok(None),
    };

    let plates = parse_three_mf_plates_from_archive(&mut archive)?;

    Ok((!plates.is_empty()).then_some(plates))
}

fn parse_three_mf_plates_from_archive<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Vec<ThreeMfPlate>> {
    let mut model_texts = HashMap::<String, String>::new();
    let mut model_settings = None::<String>;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let name = file.name().to_string();
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with(".model") && lower != "metadata/model_settings.config" {
            continue;
        }

        let mut xml = String::new();
        file.read_to_string(&mut xml)?;
        if lower == "metadata/model_settings.config" {
            model_settings = Some(xml);
        } else {
            model_texts.insert(normalize_3mf_path(&name), xml);
        }
    }

    if let Some(settings) = model_settings.as_deref() {
        if let Some(root_xml) = model_texts.get("3D/3dmodel.model") {
            let plates = parse_bambu_plate_meshes(settings, root_xml, &model_texts);
            if !plates.is_empty() {
                return Ok(plates);
            }
        }
    }

    Ok(parse_fallback_model_file_plates(&model_texts))
}

fn parse_fallback_model_file_plates(model_texts: &HashMap<String, String>) -> Vec<ThreeMfPlate> {
    let mut entries = model_texts.iter().collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));
    entries
        .into_iter()
        .enumerate()
        .filter_map(|(index, (name, xml))| {
            parse_three_mf_mesh_xml(xml).map(|mesh| ThreeMfPlate {
                label: three_mf_plate_label(name, index),
                mesh,
            })
        })
        .collect()
}

fn parse_obj_file(path: &Path) -> Result<(StlFileInfo, Option<MeshData>)> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;
    let size = metadata.len();
    let modified = metadata.modified().ok();
    if size > MAX_STL_PARSE_BYTES {
        return metadata_only_file(path, StlType::Obj).map(|info| (info, None));
    }

    let data = std::fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    let hash_bytes: [u8; 32] = blake3::hash(&data).into();
    let text = String::from_utf8_lossy(&data);
    let mesh_data = parse_obj_mesh(&text);
    let dimensions = mesh_data.as_ref().and_then(mesh_dimensions);
    let triangle_count = mesh_data.as_ref().map(|mesh| mesh.faces.len());
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
            stl_type: StlType::Obj,
            triangle_count,
            dimensions,
            three_mf_plate_count: None,
            modified,
            thumbnail_path: None,
            meta: read_sidecar(path),
        },
        mesh_data,
    ))
}

fn parse_step_file(path: &Path) -> Result<(StlFileInfo, Option<MeshData>)> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;
    let size = metadata.len();
    let modified = metadata.modified().ok();
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let (hash, dimensions, triangle_count, mesh) = if size <= MAX_TEXT_PREVIEW_BYTES {
        let data =
            std::fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
        let hash: [u8; 32] = blake3::hash(&data).into();
        let text = String::from_utf8_lossy(&data);
        let brep_mesh = parse_step_brep_mesh(&text);
        let triangle_count = brep_mesh.as_ref().map(|mesh| mesh.faces.len());
        let mesh = brep_mesh.or_else(|| {
            parse_step_bounds(&text).and_then(|(min, max)| bounding_box_mesh(min, max))
        });
        let dimensions = mesh.as_ref().and_then(mesh_dimensions).or_else(|| {
            parse_step_bounds(&text)
                .map(|(min, max)| [max[0] - min[0], max[1] - min[1], max[2] - min[2]])
        });
        (hash, dimensions, triangle_count, mesh)
    } else {
        (metadata_hash(path, size, modified), None, None, None)
    };

    Ok((
        StlFileInfo {
            path: path.to_path_buf(),
            filename,
            size,
            hash,
            stl_type: StlType::Step,
            triangle_count,
            dimensions,
            three_mf_plate_count: None,
            modified,
            thumbnail_path: None,
            meta: read_sidecar(path),
        },
        mesh,
    ))
}

fn parse_scad_file(path: &Path) -> Result<(StlFileInfo, Option<MeshData>)> {
    parse_text_cad_file(path, StlType::Scad, |text| {
        parse_scad_dimensions(text).map(|dims| ([0.0, 0.0, 0.0], dims))
    })
}

fn parse_text_cad_file(
    path: &Path,
    stl_type: StlType,
    bounds_parser: impl FnOnce(&str) -> Option<([f32; 3], [f32; 3])>,
) -> Result<(StlFileInfo, Option<MeshData>)> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Failed to read metadata for {}", path.display()))?;
    let size = metadata.len();
    let modified = metadata.modified().ok();
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let (hash, dimensions, mesh) = if size <= MAX_TEXT_PREVIEW_BYTES {
        let data =
            std::fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
        let hash: [u8; 32] = blake3::hash(&data).into();
        let text = String::from_utf8_lossy(&data);
        let bounds = bounds_parser(&text);
        let dimensions =
            bounds.map(|(min, max)| [max[0] - min[0], max[1] - min[1], max[2] - min[2]]);
        let mesh = bounds.and_then(|(min, max)| bounding_box_mesh(min, max));
        (hash, dimensions, mesh)
    } else {
        (metadata_hash(path, size, modified), None, None)
    };

    Ok((
        StlFileInfo {
            path: path.to_path_buf(),
            filename,
            size,
            hash,
            stl_type,
            triangle_count: None,
            dimensions,
            three_mf_plate_count: None,
            modified,
            thumbnail_path: None,
            meta: read_sidecar(path),
        },
        mesh,
    ))
}

fn parse_obj_mesh(text: &str) -> Option<MeshData> {
    let mut vertices = Vec::<Option<[f32; 3]>>::new();
    let mut faces = Vec::<[u32; 3]>::new();

    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                vertices.push(finite_vertex(parse_obj_vertex(&mut parts)));
            }
            Some("f") => {
                if let Some(indices) = parse_obj_face(parts, vertices.len()) {
                    faces.extend(triangulate_face(&indices));
                }
            }
            _ => {}
        }
    }

    compact_mesh(&vertices, &faces)
}

fn parse_obj_vertex<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Option<[f32; 3]> {
    Some([
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ])
}

fn parse_obj_face<'a>(
    parts: impl Iterator<Item = &'a str>,
    vertex_count: usize,
) -> Option<Vec<u32>> {
    let indices = parts
        .map(|part| obj_vertex_index(part, vertex_count))
        .collect::<Option<Vec<_>>>()?;
    (indices.len() >= 3).then_some(indices)
}

fn triangulate_face(indices: &[u32]) -> impl Iterator<Item = [u32; 3]> + '_ {
    (1..indices.len() - 1).map(|index| [indices[0], indices[index], indices[index + 1]])
}

fn obj_vertex_index(token: &str, vertex_count: usize) -> Option<u32> {
    let raw = token.split('/').next()?.parse::<isize>().ok()?;
    let index = if raw < 0 {
        vertex_count as isize + raw
    } else {
        raw - 1
    };
    (index >= 0 && (index as usize) < vertex_count).then_some(index as u32)
}

fn parse_three_mf_mesh_xml(xml: &str) -> Option<MeshData> {
    #[derive(Default)]
    struct ObjectBuilder {
        id: u32,
        vertices: Vec<Option<[f32; 3]>>,
        faces: Vec<[u32; 3]>,
    }

    struct BuildItem {
        object_id: u32,
        transform: Option<[f32; 12]>,
    }

    let mut objects = HashMap::<u32, MeshData>::new();
    let mut current = None::<ObjectBuilder>;
    let mut build_items = Vec::<BuildItem>::new();

    for tag in xml.split('<').filter_map(|part| part.split('>').next()) {
        let tag = tag.trim();
        if tag.is_empty() || tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        let closing = tag.starts_with('/');
        let self_closing = tag.ends_with('/');
        let name = tag
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_start_matches('/')
            .trim_end_matches('/');

        if !closing && name.ends_with("object") {
            if let Some(id) = attr_u32(tag, "id") {
                current = Some(ObjectBuilder {
                    id,
                    ..ObjectBuilder::default()
                });
            }
            continue;
        }

        if closing && name.ends_with("object") {
            if let Some(object) = current.take() {
                if let Some(mesh) = compact_mesh(&object.vertices, &object.faces) {
                    objects.insert(object.id, mesh);
                }
            }
            continue;
        }

        if !closing && name.ends_with("vertex") {
            if let Some(object) = current.as_mut() {
                let vertex = match (attr_f32(tag, "x"), attr_f32(tag, "y"), attr_f32(tag, "z")) {
                    (Some(x), Some(y), Some(z)) => Some([x, y, z]),
                    _ => None,
                };
                object.vertices.push(finite_vertex(vertex));
            }
        } else if !closing && name.ends_with("triangle") {
            if let Some(object) = current.as_mut() {
                if let (Some(v1), Some(v2), Some(v3)) = (
                    attr_u32(tag, "v1"),
                    attr_u32(tag, "v2"),
                    attr_u32(tag, "v3"),
                ) {
                    object.faces.push([v1, v2, v3]);
                }
            }
        } else if !closing && name.ends_with("item") {
            if let Some(object_id) = attr_u32(tag, "objectid") {
                build_items.push(BuildItem {
                    object_id,
                    transform: attr_value(tag, "transform").and_then(parse_three_mf_transform),
                });
            }
        }

        if self_closing && name.ends_with("object") {
            current = None;
        }
    }

    if let Some(object) = current.take() {
        if let Some(mesh) = compact_mesh(&object.vertices, &object.faces) {
            objects.insert(object.id, mesh);
        }
    }

    if objects.is_empty() {
        return None;
    }

    let meshes = if build_items.is_empty() {
        objects.values().cloned().collect::<Vec<_>>()
    } else {
        build_items
            .iter()
            .filter_map(|item| {
                objects
                    .get(&item.object_id)
                    .map(|mesh| transformed_mesh(mesh, item.transform))
            })
            .collect::<Vec<_>>()
    };

    merge_meshes(&meshes)
}

struct BambuPlateDef {
    label: String,
    object_ids: Vec<u32>,
}

struct ThreeMfComponentRef {
    path: String,
    transform: Option<[f32; 12]>,
}

fn parse_bambu_plate_meshes(
    settings_xml: &str,
    root_xml: &str,
    model_texts: &HashMap<String, String>,
) -> Vec<ThreeMfPlate> {
    let plate_defs = parse_bambu_plate_defs(settings_xml);
    if plate_defs.is_empty() {
        return Vec::new();
    }
    let build_transforms = parse_three_mf_build_transforms(root_xml);
    let component_refs = parse_three_mf_root_components(root_xml);

    plate_defs
        .into_iter()
        .filter_map(|plate| {
            let mut meshes = Vec::new();
            for object_id in plate.object_ids {
                let item_transform = build_transforms.get(&object_id).copied().flatten();
                let Some(components) = component_refs.get(&object_id) else {
                    continue;
                };
                for component in components {
                    let Some(xml) = model_texts.get(&component.path) else {
                        continue;
                    };
                    let Some(mesh) = parse_three_mf_mesh_xml(xml) else {
                        continue;
                    };
                    let mesh = transformed_mesh(&mesh, component.transform);
                    let mesh = transformed_mesh(&mesh, item_transform);
                    meshes.push(mesh);
                }
            }
            merge_meshes(&meshes).map(|mesh| ThreeMfPlate {
                label: plate.label,
                mesh,
            })
        })
        .collect()
}

fn parse_bambu_plate_defs(settings_xml: &str) -> Vec<BambuPlateDef> {
    xml_element_blocks(settings_xml, "plate")
        .into_iter()
        .enumerate()
        .filter_map(|(index, block)| {
            let label = metadata_value(&block, "plater_name")
                .filter(|label| !label.trim().is_empty())
                .unwrap_or_else(|| format!("Plate {}", index + 1));
            let object_ids = xml_element_blocks(&block, "model_instance")
                .into_iter()
                .filter_map(|instance| metadata_value(&instance, "object_id")?.parse().ok())
                .collect::<Vec<_>>();
            (!object_ids.is_empty()).then_some(BambuPlateDef { label, object_ids })
        })
        .collect()
}

fn parse_three_mf_build_transforms(root_xml: &str) -> HashMap<u32, Option<[f32; 12]>> {
    tags_named(root_xml, "item")
        .into_iter()
        .filter_map(|tag| {
            let object_id = attr_u32(&tag, "objectid")?;
            let transform = attr_value(&tag, "transform").and_then(parse_three_mf_transform);
            Some((object_id, transform))
        })
        .collect()
}

fn parse_three_mf_root_components(root_xml: &str) -> HashMap<u32, Vec<ThreeMfComponentRef>> {
    let mut refs = HashMap::<u32, Vec<ThreeMfComponentRef>>::new();
    for block in xml_element_blocks(root_xml, "object") {
        let Some(open_tag) = block
            .split('<')
            .filter_map(|part| part.split('>').next())
            .find(|tag| tag.trim_start().starts_with("object"))
        else {
            continue;
        };
        let Some(object_id) = attr_u32(open_tag, "id") else {
            continue;
        };
        let components = tags_named(&block, "component")
            .into_iter()
            .filter_map(|tag| {
                let path = attr_value(&tag, "p:path").or_else(|| attr_value(&tag, "path"))?;
                Some(ThreeMfComponentRef {
                    path: normalize_3mf_path(path),
                    transform: attr_value(&tag, "transform").and_then(parse_three_mf_transform),
                })
            })
            .collect::<Vec<_>>();
        if !components.is_empty() {
            refs.insert(object_id, components);
        }
    }
    refs
}

fn metadata_value(block: &str, key: &str) -> Option<String> {
    tags_named(block, "metadata").into_iter().find_map(|tag| {
        let tag_key = attr_value(&tag, "key").or_else(|| attr_value(&tag, "name"))?;
        (tag_key == key).then(|| xml_unescape_attr(attr_value(&tag, "value").unwrap_or_default()))
    })
}

fn tags_named(xml: &str, tag_name: &str) -> Vec<String> {
    xml.split('<')
        .filter_map(|part| part.split('>').next())
        .map(str::trim)
        .filter(|tag| {
            !tag.starts_with('/') && {
                let name = tag
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .trim_end_matches('/');
                name == tag_name || name.ends_with(&format!(":{tag_name}"))
            }
        })
        .map(str::to_string)
        .collect()
}

fn xml_element_blocks(xml: &str, tag_name: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut offset = 0usize;
    let open_pattern = format!("<{tag_name}");
    let close_pattern = format!("</{tag_name}>");
    while let Some(start_rel) = xml[offset..].find(&open_pattern) {
        let start = offset + start_rel;
        let Some(end_rel) = xml[start..].find(&close_pattern) else {
            break;
        };
        let end = start + end_rel + close_pattern.len();
        blocks.push(xml[start..end].to_string());
        offset = end;
    }
    blocks
}

fn normalize_3mf_path(path: &str) -> String {
    path.trim_start_matches('/')
        .replace('\\', "/")
        .trim()
        .to_string()
}

fn xml_unescape_attr(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn parse_three_mf_transform(value: &str) -> Option<[f32; 12]> {
    let numbers = value
        .split_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let transform: [f32; 12] = numbers.try_into().ok()?;
    transform
        .iter()
        .all(|value| value.is_finite())
        .then_some(transform)
}

fn transformed_mesh(mesh: &MeshData, transform: Option<[f32; 12]>) -> MeshData {
    let Some(m) = transform else {
        return mesh.clone();
    };
    // 3MF stores transform matrices as:
    // [ m00 m01 m02 m10 m11 m12 m20 m21 m22 tx ty tz ]
    // for row-vector application: [x y z 1] * M.
    // The final three values are translation, not the fourth value of each row.
    MeshData {
        vertices: mesh
            .vertices
            .iter()
            .map(|[x, y, z]| {
                [
                    m[0] * x + m[3] * y + m[6] * z + m[9],
                    m[1] * x + m[4] * y + m[7] * z + m[10],
                    m[2] * x + m[5] * y + m[8] * z + m[11],
                ]
            })
            .collect(),
        faces: mesh.faces.clone(),
    }
}

pub(crate) fn arranged_three_mf_overview_mesh(plates: &[ThreeMfPlate]) -> Option<MeshData> {
    let mut offset_x = 0.0_f32;
    let mut arranged = Vec::new();
    for plate in plates {
        let (min, max) = mesh_bounds(&plate.mesh)?;
        let width = (max[0] - min[0]).max(1.0);
        let gap = width.max(30.0) * 0.18 + 18.0;
        let translated = translate_mesh(&plate.mesh, [offset_x - min[0], -min[1], -min[2]]);
        arranged.push(translated);
        offset_x += width + gap;
    }
    merge_meshes(&arranged)
}

fn translate_mesh(mesh: &MeshData, offset: [f32; 3]) -> MeshData {
    MeshData {
        vertices: mesh
            .vertices
            .iter()
            .map(|[x, y, z]| [x + offset[0], y + offset[1], z + offset[2]])
            .collect(),
        faces: mesh.faces.clone(),
    }
}

fn merge_meshes(meshes: &[MeshData]) -> Option<MeshData> {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for mesh in meshes {
        let base = u32::try_from(vertices.len()).ok()?;
        vertices.extend(mesh.vertices.iter().copied());
        faces.extend(mesh.faces.iter().filter_map(|face| {
            Some([
                base.checked_add(face[0])?,
                base.checked_add(face[1])?,
                base.checked_add(face[2])?,
            ])
        }));
    }
    (!vertices.is_empty() && !faces.is_empty()).then_some(MeshData { vertices, faces })
}

pub(crate) fn compact_mesh(vertices: &[Option<[f32; 3]>], faces: &[[u32; 3]]) -> Option<MeshData> {
    let mut remap = vec![None; vertices.len()];
    let mut compact_vertices = Vec::new();

    for (old_index, vertex) in vertices.iter().enumerate() {
        if let Some(vertex) = finite_vertex(*vertex) {
            remap[old_index] = Some(compact_vertices.len() as u32);
            compact_vertices.push(vertex);
        }
    }

    let compact_faces = faces
        .iter()
        .filter_map(|face| {
            Some([
                remap.get(face[0] as usize).copied().flatten()?,
                remap.get(face[1] as usize).copied().flatten()?,
                remap.get(face[2] as usize).copied().flatten()?,
            ])
        })
        .collect::<Vec<_>>();

    (!compact_vertices.is_empty() && !compact_faces.is_empty()).then_some(MeshData {
        vertices: compact_vertices,
        faces: compact_faces,
    })
}

fn finite_vertex(vertex: Option<[f32; 3]>) -> Option<[f32; 3]> {
    vertex.filter(|vertex| vertex.iter().all(|value| value.is_finite()))
}

fn attr_f32(tag: &str, attr: &str) -> Option<f32> {
    attr_value(tag, attr)?.parse().ok()
}

fn attr_u32(tag: &str, attr: &str) -> Option<u32> {
    attr_value(tag, attr)?.parse().ok()
}

fn attr_value<'a>(tag: &'a str, attr: &str) -> Option<&'a str> {
    for (start, _) in tag.match_indices(attr) {
        let before = tag[..start].chars().next_back();
        if before.is_some_and(|ch| !ch.is_whitespace()) {
            continue;
        }
        let rest = &tag[start + attr.len()..];
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let rest = &rest[quote.len_utf8()..];
        let end = rest.find(quote)?;
        return Some(&rest[..end]);
    }
    None
}

fn three_mf_plate_label(name: &str, index: usize) -> String {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    let normalized = stem.replace(['_', '-'], " ");
    let cleaned = normalized.trim();
    if cleaned.is_empty()
        || cleaned.eq_ignore_ascii_case("3dmodel")
        || cleaned.eq_ignore_ascii_case("model")
    {
        format!("Plate {}", index + 1)
    } else if cleaned.to_ascii_lowercase().starts_with("plate") {
        let mut chars = cleaned.chars();
        chars
            .next()
            .map(|first| first.to_uppercase().chain(chars).collect())
            .unwrap_or_else(|| format!("Plate {}", index + 1))
    } else {
        format!("Plate {} · {}", index + 1, cleaned)
    }
}

fn mesh_bounds(mesh: &MeshData) -> Option<([f32; 3], [f32; 3])> {
    let first = *mesh.vertices.first()?;
    let (mut min, mut max) = (first, first);
    for vertex in &mesh.vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex[axis]);
            max[axis] = max[axis].max(vertex[axis]);
        }
    }
    Some((min, max))
}

pub(crate) fn mesh_dimensions(mesh: &MeshData) -> Option<[f32; 3]> {
    let (min, max) = mesh_bounds(mesh)?;
    Some([max[0] - min[0], max[1] - min[1], max[2] - min[2]])
}

pub(crate) fn mesh_volume_cm3(mesh: &MeshData) -> Option<f32> {
    let mut signed_volume_mm3 = 0.0_f64;
    for [a, b, c] in &mesh.faces {
        let va = *mesh.vertices.get(*a as usize)?;
        let vb = *mesh.vertices.get(*b as usize)?;
        let vc = *mesh.vertices.get(*c as usize)?;
        let cross = [
            vb[1] as f64 * vc[2] as f64 - vb[2] as f64 * vc[1] as f64,
            vb[2] as f64 * vc[0] as f64 - vb[0] as f64 * vc[2] as f64,
            vb[0] as f64 * vc[1] as f64 - vb[1] as f64 * vc[0] as f64,
        ];
        signed_volume_mm3 +=
            (va[0] as f64 * cross[0] + va[1] as f64 * cross[1] + va[2] as f64 * cross[2]) / 6.0;
    }
    let volume_cm3 = (signed_volume_mm3.abs() / 1000.0) as f32;
    (volume_cm3.is_finite() && volume_cm3 > 0.0001).then_some(volume_cm3)
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
        let parsed = parse_binary_stl_fast(&data).unwrap_or_else(|| parse_ascii_stl(&data));
        if parsed.0 == StlType::Unknown || parsed.3.is_none() {
            parse_stl_io_mesh(&data).unwrap_or(parsed)
        } else {
            parsed
        }
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
            three_mf_plate_count: None,
            modified,
            thumbnail_path: None,
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
        three_mf_plate_count: None,
        modified,
        thumbnail_path: None,
        meta: read_sidecar(path),
    })
}

fn parse_stl_io_mesh(data: &[u8]) -> Option<ParsedStl> {
    let mut cursor = Cursor::new(data);
    let indexed = stl_io::read_stl(&mut cursor).ok()?;
    if indexed.vertices.is_empty() || indexed.faces.is_empty() {
        return None;
    }

    let vertices = indexed
        .vertices
        .iter()
        .map(|vertex| vertex.0)
        .collect::<Vec<_>>();
    if vertices.iter().flatten().any(|value| !value.is_finite()) {
        return None;
    }

    let faces = indexed
        .faces
        .iter()
        .map(|face| {
            Some([
                u32::try_from(face.vertices[0]).ok()?,
                u32::try_from(face.vertices[1]).ok()?,
                u32::try_from(face.vertices[2]).ok()?,
            ])
        })
        .collect::<Option<Vec<_>>>()?;

    let mesh = MeshData { vertices, faces }.compacted()?;
    let dimensions = mesh_dimensions(&mesh);
    Some((
        if detect_stl_type(data) == StlType::Ascii {
            StlType::Ascii
        } else {
            StlType::Binary
        },
        Some(mesh.faces.len()),
        dimensions,
        Some(mesh),
    ))
}

fn parse_step_bounds(text: &str) -> Option<([f32; 3], [f32; 3])> {
    let mut points = text
        .lines()
        .filter_map(|line| {
            let start = line.find("CARTESIAN_POINT")?;
            let numbers = numbers_in_text(&line[start..]);
            if numbers.len() >= 3 {
                Some([numbers[0], numbers[1], numbers[2]])
            } else {
                None
            }
        })
        .filter(|point| point.iter().all(|value| value.is_finite()));

    let first = points.next()?;
    let (mut min, mut max) = (first, first);
    for point in points {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    Some((min, max))
}

fn parse_step_brep_mesh(text: &str) -> Option<MeshData> {
    let entities = parse_step_entities(text);
    if entities.is_empty() {
        return None;
    }

    let points = entities
        .iter()
        .filter_map(|(id, body)| {
            body.contains("CARTESIAN_POINT")
                .then(|| parse_step_cartesian_point(body).map(|point| (*id, point)))?
        })
        .collect::<HashMap<_, _>>();

    let vertices = entities
        .iter()
        .filter_map(|(id, body)| {
            body.contains("VERTEX_POINT").then(|| {
                entity_refs(body)
                    .first()
                    .copied()
                    .map(|point_id| (*id, point_id))
            })?
        })
        .collect::<HashMap<_, _>>();

    let mut mesh_vertices = Vec::<[f32; 3]>::new();
    let mut mesh_faces = Vec::<[u32; 3]>::new();
    let mut vertex_remap = HashMap::<[i64; 3], u32>::new();

    for body in entities
        .values()
        .filter(|body| body.contains("ADVANCED_FACE"))
    {
        let face_refs = entity_refs(body);
        for bound_id in face_refs {
            let Some(bound) = entities.get(&bound_id) else {
                continue;
            };
            if !bound.contains("FACE_OUTER_BOUND") {
                continue;
            }
            let Some(loop_id) = entity_refs(bound).first().copied() else {
                continue;
            };
            let Some(edge_loop) = entities.get(&loop_id) else {
                continue;
            };
            if !edge_loop.contains("EDGE_LOOP") {
                continue;
            }

            let mut loop_points = Vec::<[f32; 3]>::new();
            for oriented_edge_id in entity_refs(edge_loop) {
                let Some(oriented_edge) = entities.get(&oriented_edge_id) else {
                    continue;
                };
                if !oriented_edge.contains("ORIENTED_EDGE") {
                    continue;
                }
                let Some(edge_curve_id) = entity_refs(oriented_edge).last().copied() else {
                    continue;
                };
                let Some(edge_curve) = entities.get(&edge_curve_id) else {
                    continue;
                };
                if !edge_curve.contains("EDGE_CURVE") {
                    continue;
                }
                let edge_refs = entity_refs(edge_curve);
                if edge_refs.len() < 2 {
                    continue;
                }
                let edge_forward = step_final_bool(edge_curve).unwrap_or(true);
                let oriented_forward = step_final_bool(oriented_edge).unwrap_or(true);
                let forward = edge_forward == oriented_forward;
                let (start_vertex, end_vertex) = if forward {
                    (edge_refs[0], edge_refs[1])
                } else {
                    (edge_refs[1], edge_refs[0])
                };
                let Some(start) = step_vertex_point(start_vertex, &vertices, &points) else {
                    continue;
                };
                let Some(end) = step_vertex_point(end_vertex, &vertices, &points) else {
                    continue;
                };

                if loop_points
                    .last()
                    .is_none_or(|last| !points_near(*last, start))
                {
                    loop_points.push(start);
                }
                if loop_points
                    .last()
                    .is_none_or(|last| !points_near(*last, end))
                {
                    loop_points.push(end);
                }
            }

            if loop_points.len() > 3
                && points_near(
                    loop_points[0],
                    *loop_points.last().unwrap_or(&loop_points[0]),
                )
            {
                loop_points.pop();
            }
            append_step_polygon_mesh(
                &loop_points,
                &mut mesh_vertices,
                &mut mesh_faces,
                &mut vertex_remap,
            );
        }
    }

    MeshData {
        vertices: mesh_vertices,
        faces: mesh_faces,
    }
    .compacted()
}

fn parse_step_entities(text: &str) -> HashMap<u32, String> {
    let mut entities = HashMap::new();
    for raw in text.split(';') {
        let Some(hash) = raw.find('#') else {
            continue;
        };
        let rest = &raw[hash + 1..];
        let Some(eq) = rest.find('=') else {
            continue;
        };
        let Ok(id) = rest[..eq].trim().parse::<u32>() else {
            continue;
        };
        entities.insert(id, rest[eq + 1..].trim().to_string());
    }
    entities
}

fn parse_step_cartesian_point(body: &str) -> Option<[f32; 3]> {
    let start = body.find("CARTESIAN_POINT")?;
    let numbers = numbers_in_text(&body[start..]);
    (numbers.len() >= 3).then_some([numbers[0], numbers[1], numbers[2]])
}

fn entity_refs(body: &str) -> Vec<u32> {
    let mut refs = Vec::new();
    let bytes = body.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == b'#' {
            let start = idx + 1;
            idx = start;
            while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
            if idx > start {
                if let Ok(value) = body[start..idx].parse::<u32>() {
                    refs.push(value);
                }
            }
        } else {
            idx += 1;
        }
    }
    refs
}

fn step_final_bool(body: &str) -> Option<bool> {
    let true_pos = body.rfind(".T.");
    let false_pos = body.rfind(".F.");
    match (true_pos, false_pos) {
        (Some(t), Some(f)) => Some(t > f),
        (Some(_), None) => Some(true),
        (None, Some(_)) => Some(false),
        (None, None) => None,
    }
}

fn step_vertex_point(
    vertex_id: u32,
    vertices: &HashMap<u32, u32>,
    points: &HashMap<u32, [f32; 3]>,
) -> Option<[f32; 3]> {
    points.get(vertices.get(&vertex_id)?).copied()
}

fn append_step_polygon_mesh(
    points: &[[f32; 3]],
    vertices: &mut Vec<[f32; 3]>,
    faces: &mut Vec<[u32; 3]>,
    remap: &mut HashMap<[i64; 3], u32>,
) {
    if points.len() < 3 || polygon_area_estimate(points) < 0.0001 {
        return;
    }

    let base = push_step_vertex(points[0], vertices, remap);
    for idx in 1..points.len() - 1 {
        if triangle_area(points[0], points[idx], points[idx + 1]) < 0.0001 {
            continue;
        }
        let b = push_step_vertex(points[idx], vertices, remap);
        let c = push_step_vertex(points[idx + 1], vertices, remap);
        if base != b && b != c && c != base {
            faces.push([base, b, c]);
        }
    }
}

fn push_step_vertex(
    point: [f32; 3],
    vertices: &mut Vec<[f32; 3]>,
    remap: &mut HashMap<[i64; 3], u32>,
) -> u32 {
    let key = [
        (point[0] * 10_000.0).round() as i64,
        (point[1] * 10_000.0).round() as i64,
        (point[2] * 10_000.0).round() as i64,
    ];
    if let Some(index) = remap.get(&key) {
        return *index;
    }
    let index = vertices.len() as u32;
    vertices.push(point);
    remap.insert(key, index);
    index
}

fn points_near(a: [f32; 3], b: [f32; 3]) -> bool {
    (a[0] - b[0]).abs() < 0.0001 && (a[1] - b[1]).abs() < 0.0001 && (a[2] - b[2]).abs() < 0.0001
}

fn polygon_area_estimate(points: &[[f32; 3]]) -> f32 {
    let mut normal = [0.0, 0.0, 0.0];
    for idx in 0..points.len() {
        let current = points[idx];
        let next = points[(idx + 1) % points.len()];
        normal[0] += (current[1] - next[1]) * (current[2] + next[2]);
        normal[1] += (current[2] - next[2]) * (current[0] + next[0]);
        normal[2] += (current[0] - next[0]) * (current[1] + next[1]);
    }
    (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt() * 0.5
}

fn triangle_area(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt() * 0.5
}

fn parse_scad_dimensions(text: &str) -> Option<[f32; 3]> {
    let lower = text.to_ascii_lowercase();
    let mut candidates = Vec::new();

    for segment in lower
        .match_indices("cube")
        .map(|(idx, _)| preview_segment(&lower, idx))
    {
        let numbers = numbers_in_text(segment);
        match numbers.as_slice() {
            [x, y, z, ..] => candidates.push([x.abs(), y.abs(), z.abs()]),
            [size] => candidates.push([size.abs(), size.abs(), size.abs()]),
            _ => {}
        }
    }

    for segment in lower
        .match_indices("cylinder")
        .map(|(idx, _)| preview_segment(&lower, idx))
    {
        let height =
            named_number(segment, "h").or_else(|| numbers_in_text(segment).first().copied());
        let radius = named_number(segment, "r")
            .or_else(|| named_number(segment, "r1"))
            .or_else(|| named_number(segment, "r2"))
            .or_else(|| named_number(segment, "d").map(|d| d / 2.0));
        if let (Some(height), Some(radius)) = (height, radius) {
            let diameter = (radius.abs() * 2.0).max(0.001);
            candidates.push([diameter, diameter, height.abs()]);
        }
    }

    for segment in lower
        .match_indices("sphere")
        .map(|(idx, _)| preview_segment(&lower, idx))
    {
        let radius = named_number(segment, "r")
            .or_else(|| named_number(segment, "d").map(|d| d / 2.0))
            .or_else(|| numbers_in_text(segment).first().copied());
        if let Some(radius) = radius {
            let diameter = (radius.abs() * 2.0).max(0.001);
            candidates.push([diameter, diameter, diameter]);
        }
    }

    candidates.into_iter().max_by(|a, b| {
        volume(*a)
            .partial_cmp(&volume(*b))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

fn preview_segment(text: &str, start: usize) -> &str {
    &text[start..text.len().min(start + 220)]
}

fn named_number(segment: &str, name: &str) -> Option<f32> {
    for (start, _) in segment.match_indices(name) {
        let before = segment[..start].chars().next_back();
        if before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            continue;
        }
        let after_name = start + name.len();
        let after = segment[after_name..].chars().next();
        if after.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
            continue;
        }
        let rest = segment[after_name..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        if let Some(value) = numbers_in_text(rest.trim_start()).first().copied() {
            return Some(value);
        }
    }
    None
}

fn volume(dimensions: [f32; 3]) -> f32 {
    dimensions[0].max(0.0) * dimensions[1].max(0.0) * dimensions[2].max(0.0)
}

fn numbers_in_text(text: &str) -> Vec<f32> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch.is_ascii_digit() || ch == '-' || ch == '+' || ch == '.' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch.is_ascii_digit() || matches!(ch, '.' | 'e' | 'E' | '-' | '+') {
                    i += 1;
                } else {
                    break;
                }
            }
            if let Ok(value) = text[start..i].parse::<f32>() {
                out.push(value);
            }
        } else {
            i += 1;
        }
    }
    out
}

fn bounding_box_mesh(min: [f32; 3], max: [f32; 3]) -> Option<MeshData> {
    let mut visual_min = min;
    let mut visual_max = max;
    let max_span = (0..3)
        .map(|axis| visual_max[axis] - visual_min[axis])
        .fold(0.0_f32, f32::max)
        .max(1.0);
    for axis in 0..3 {
        if (visual_max[axis] - visual_min[axis]).abs() < 0.001 {
            let pad = max_span * 0.04;
            visual_min[axis] -= pad;
            visual_max[axis] += pad;
        }
    }

    let [min_x, min_y, min_z] = visual_min;
    let [max_x, max_y, max_z] = visual_max;
    let vertices = vec![
        [min_x, min_y, min_z],
        [max_x, min_y, min_z],
        [max_x, max_y, min_z],
        [min_x, max_y, min_z],
        [min_x, min_y, max_z],
        [max_x, min_y, max_z],
        [max_x, max_y, max_z],
        [min_x, max_y, max_z],
    ];
    let faces = vec![
        [0, 1, 2],
        [0, 2, 3],
        [4, 6, 5],
        [4, 7, 6],
        [0, 4, 5],
        [0, 5, 1],
        [1, 5, 6],
        [1, 6, 2],
        [2, 6, 7],
        [2, 7, 3],
        [3, 7, 4],
        [3, 4, 0],
    ];
    MeshData { vertices, faces }.compacted()
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

pub(crate) fn sidecar_path(model_path: &Path) -> PathBuf {
    let file_name = model_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("model");
    model_path.with_file_name(format!("{}.modelrack.json", file_name))
}

fn parse_binary_stl_fast(data: &[u8]) -> Option<ParsedStl> {
    if data.len() < 84 {
        return None;
    }

    let triangle_count = u32::from_le_bytes(data[80..84].try_into().ok()?) as usize;
    let expected_len = 84usize.checked_add(triangle_count.checked_mul(50)?)?;
    if expected_len > data.len() {
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
        Some(MeshData { vertices, faces }),
    ))
}

fn parse_ascii_stl(data: &[u8]) -> ParsedStl {
    if detect_stl_type(data) != StlType::Ascii {
        return (StlType::Unknown, None, None, None);
    }

    let text = String::from_utf8_lossy(data);
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];

    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if !parts
            .next()
            .is_some_and(|token| token.eq_ignore_ascii_case("vertex"))
        {
            continue;
        }
        let Some(vertex) = parse_ascii_vertex(&mut parts) else {
            continue;
        };
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex[axis]);
            max[axis] = max[axis].max(vertex[axis]);
        }
        vertices.push(vertex);
        if vertices.len() % 3 == 0 {
            let base = (vertices.len() - 3) as u32;
            faces.push([base, base + 1, base + 2]);
        }
    }

    if vertices.is_empty() || faces.is_empty() {
        return (StlType::Ascii, None, None, None);
    }

    let dimensions = Some([max[0] - min[0], max[1] - min[1], max[2] - min[2]]);
    (
        StlType::Ascii,
        Some(faces.len()),
        dimensions,
        Some(MeshData { vertices, faces }),
    )
}

fn parse_ascii_vertex<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Option<[f32; 3]> {
    let vertex: [f32; 3] = [
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    ];
    vertex
        .iter()
        .all(|value| value.is_finite())
        .then_some(vertex)
}

fn binary_stl_triangle_count(data: &[u8]) -> Option<usize> {
    if data.len() < 84 {
        return None;
    }

    let triangle_count = u32::from_le_bytes(data[80..84].try_into().ok()?) as usize;
    let expected_len = 84usize.checked_add(triangle_count.checked_mul(50)?)?;
    (expected_len <= data.len()).then_some(triangle_count)
}

fn read_f32_le(data: &[u8], start: usize) -> Option<f32> {
    Some(f32::from_le_bytes(
        data.get(start..start + 4)?.try_into().ok()?,
    ))
}

fn detect_stl_type(data: &[u8]) -> StlType {
    if data
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"solid"))
    {
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
    fn scan_includes_3mf_mesh_preview_data() {
        let dir = std::env::temp_dir().join(format!("modelrack-3mf-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let file = std::fs::File::create(dir.join("plate.3mf")).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(
            "3D/3dmodel.model",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        std::io::Write::write_all(
            &mut zip,
            br#"<model><resources><object id="1"><mesh><vertices>
                <vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/>
                </vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#,
        )
        .unwrap();
        zip.finish().unwrap();

        let result = scan_folder(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].filename, "plate.3mf");
        assert!(matches!(result.entries[0].stl_type, StlType::ThreeMf));
        assert_eq!(result.entries[0].triangle_count, Some(1));
        assert_eq!(result.meshes.len(), 1);
    }

    #[test]
    fn multi_model_3mf_preserves_plate_count_and_preview_plates() {
        let dir =
            std::env::temp_dir().join(format!("modelrack-3mf-plates-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("multi.3mf");
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (name, x_offset) in [
            ("3D/3dmodel.model", 0.0_f32),
            ("Metadata/plate_2.model", 10.0),
        ] {
            zip.start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            let xml = format!(
                r#"<model><resources><object id="1"><mesh><vertices>
                <vertex x="{x_offset}" y="0" z="0"/><vertex x="{}" y="0" z="0"/><vertex x="{x_offset}" y="1" z="0"/>
                </vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#,
                x_offset + 1.0
            );
            std::io::Write::write_all(&mut zip, xml.as_bytes()).unwrap();
        }
        zip.finish().unwrap();

        let result = scan_folder(&dir);
        let plates = parse_preview_plates(&path).unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].filename, "multi.3mf");
        assert_eq!(result.entries[0].triangle_count, Some(2));
        assert_eq!(result.entries[0].three_mf_plate_count, Some(2));
        assert_eq!(result.meshes.len(), 1);
        assert_eq!(result.meshes[0].faces.len(), 1);
        assert_eq!(plates.len(), 2);
        assert_eq!(plates[0].label, "Plate 1");
        assert_eq!(plates[1].label, "Plate 2");
    }

    #[test]
    fn bambu_3mf_uses_plater_names_for_preview_plates() {
        let dir = std::env::temp_dir().join(format!(
            "modelrack-bambu-plate-names-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("named-plates.3mf");
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);

        zip.start_file(
            "3D/3dmodel.model",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        std::io::Write::write_all(
            &mut zip,
            br#"<model><resources>
                <object id="10" type="model"><components><component p:path="/3D/Objects/object_10.model" objectid="1" transform="1 0 0 0 1 0 0 0 1 0 0 0"/></components></object>
                <object id="20" type="model"><components><component p:path="/3D/Objects/object_20.model" objectid="1" transform="1 0 0 0 1 0 0 0 1 0 0 0"/></components></object>
                </resources><build>
                <item objectid="10" transform="1 0 0 0 1 0 0 0 1 0 0 0"/>
                <item objectid="20" transform="1 0 0 0 1 0 0 0 1 10 0 0"/>
                </build></model>"#,
        )
        .unwrap();

        for object_name in ["object_10", "object_20"] {
            zip.start_file(
                format!("3D/Objects/{object_name}.model"),
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            std::io::Write::write_all(
                &mut zip,
                br#"<model><resources><object id="1"><mesh><vertices>
                <vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/>
                </vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources></model>"#,
            )
            .unwrap();
        }

        zip.start_file(
            "Metadata/model_settings.config",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        std::io::Write::write_all(
            &mut zip,
            br#"<config>
            <plate><metadata key="plater_id" value="1"/><metadata key="plater_name" value="Paintable Cover 2 for Vertical Orientation"/><model_instance><metadata key="object_id" value="10"/></model_instance></plate>
            <plate><metadata key="plater_id" value="2"/><metadata key="plater_name" value="Action Covers (Choose one)"/><model_instance><metadata key="object_id" value="20"/></model_instance></plate>
            </config>"#,
        )
        .unwrap();
        zip.finish().unwrap();

        let result = scan_folder(&dir);
        let plates = parse_preview_plates(&path).unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].three_mf_plate_count, Some(2));
        assert_eq!(result.entries[0].triangle_count, Some(2));
        assert_eq!(result.meshes[0].faces.len(), 1);
        assert_eq!(plates.len(), 2);
        assert_eq!(
            plates
                .iter()
                .map(|plate| plate.label.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Paintable Cover 2 for Vertical Orientation",
                "Action Covers (Choose one)"
            ]
        );
    }

    #[test]
    fn three_mf_build_items_apply_object_transforms() {
        let xml = r#"<model><resources><object id="7"><mesh><vertices>
            <vertex x="0" y="0" z="0"/><vertex x="1" y="0" z="0"/><vertex x="0" y="1" z="0"/>
            </vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object></resources>
            <build>
                <item objectid="7" transform="1 0 0 0 1 0 0 0 1 10 20 30"/>
                <item objectid="7" transform="1 0 0 0 1 0 0 0 1 40 50 60"/>
            </build></model>"#;

        let mesh = parse_three_mf_mesh_xml(xml).unwrap();
        let dims = mesh_dimensions(&mesh).unwrap();

        assert_eq!(mesh.faces.len(), 2);
        assert!(mesh
            .vertices
            .iter()
            .any(|vertex| *vertex == [10.0, 20.0, 30.0]));
        assert!(mesh
            .vertices
            .iter()
            .any(|vertex| *vertex == [40.0, 50.0, 60.0]));
        assert_eq!(dims, [31.0, 31.0, 30.0]);
    }

    #[test]
    fn three_mf_transform_uses_spec_translation_slots() {
        let mesh = MeshData {
            vertices: vec![[1.0, 2.0, 3.0]],
            faces: vec![[0, 0, 0]],
        };
        let transformed = transformed_mesh(
            &mesh,
            Some([
                1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 10.0, 20.0, 30.0,
            ]),
        );

        assert_eq!(transformed.vertices, vec![[11.0, 22.0, 33.0]]);
    }

    #[test]
    fn mesh_volume_estimates_closed_box_volume() {
        let mesh = bounding_box_mesh([0.0, 0.0, 0.0], [10.0, 20.0, 30.0]).unwrap();

        assert_eq!(mesh_dimensions(&mesh), Some([10.0, 20.0, 30.0]));
        assert!((mesh_volume_cm3(&mesh).unwrap() - 6.0).abs() < 0.001);
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
    fn ascii_stl_appears_with_mesh_preview() {
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
        assert_eq!(result.entries[0].triangle_count, Some(1));
        assert_eq!(result.meshes.len(), 1);
    }

    #[test]
    fn stl_library_fallback_handles_uppercase_ascii_stl() {
        let dir = std::env::temp_dir().join(format!(
            "modelrack-uppercase-ascii-stl-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("upper.stl");
        std::fs::write(
            &path,
            b"SOLID upper\nFACET NORMAL 0 0 1\nOUTER LOOP\nVERTEX 0 0 0\nVERTEX 1 0 0\nVERTEX 0 1 0\nENDLOOP\nENDFACET\nENDSOLID upper\n",
        )
        .unwrap();

        let result = scan_folder(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        assert_ne!(result.entries[0].stl_type, StlType::Unknown);
        assert_eq!(result.entries[0].triangle_count, Some(1));
        assert_eq!(result.meshes.len(), 1);
    }

    #[test]
    fn obj_file_appears_with_mesh_preview() {
        let dir = std::env::temp_dir().join(format!("modelrack-obj-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("quad.obj");
        std::fs::write(&path, b"v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n").unwrap();

        let result = scan_folder(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        assert!(matches!(result.entries[0].stl_type, StlType::Obj));
        assert_eq!(result.entries[0].triangle_count, Some(2));
        assert_eq!(result.meshes.len(), 1);
    }

    #[test]
    fn step_file_extracts_cartesian_point_bounds_for_preview() {
        let dir = std::env::temp_dir().join(format!("modelrack-step-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("adapter.step");
        std::fs::write(
            &path,
            "#10 = CARTESIAN_POINT('',(-12.5,0.,1.));\n\
             #11 = CARTESIAN_POINT('',(37.5,20.,11.));\n",
        )
        .unwrap();

        let result = scan_folder(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        assert!(matches!(result.entries[0].stl_type, StlType::Step));
        assert_eq!(result.entries[0].dimensions, Some([50.0, 20.0, 10.0]));
        assert_eq!(result.entries[0].triangle_count, None);
        assert_eq!(result.meshes.len(), 1);
    }

    #[test]
    fn step_preview_ignores_incomplete_cartesian_points_without_panic() {
        let bounds = parse_step_bounds(
            "#1 = CARTESIAN_POINT('',(0.,0.));\n\
             #2 = CARTESIAN_POINT('',(0.,0.,0.));\n\
             #3 = CARTESIAN_POINT('',(10.,5.,2.));\n",
        )
        .unwrap();

        assert_eq!(bounds, ([0.0, 0.0, 0.0], [10.0, 5.0, 2.0]));
    }

    #[test]
    fn step_brep_faces_create_preview_mesh_instead_of_box_only() {
        let text = "\
            #1=CARTESIAN_POINT('',(0.,0.,0.));\
            #2=CARTESIAN_POINT('',(10.,0.,0.));\
            #3=CARTESIAN_POINT('',(10.,5.,0.));\
            #4=CARTESIAN_POINT('',(0.,5.,0.));\
            #11=VERTEX_POINT('',#1);\
            #12=VERTEX_POINT('',#2);\
            #13=VERTEX_POINT('',#3);\
            #14=VERTEX_POINT('',#4);\
            #21=EDGE_CURVE('',#11,#12,#101,.T.);\
            #22=EDGE_CURVE('',#12,#13,#102,.T.);\
            #23=EDGE_CURVE('',#13,#14,#103,.T.);\
            #24=EDGE_CURVE('',#14,#11,#104,.T.);\
            #31=ORIENTED_EDGE('',*,*,#21,.T.);\
            #32=ORIENTED_EDGE('',*,*,#22,.T.);\
            #33=ORIENTED_EDGE('',*,*,#23,.T.);\
            #34=ORIENTED_EDGE('',*,*,#24,.T.);\
            #41=EDGE_LOOP('',(#31,#32,#33,#34));\
            #51=FACE_OUTER_BOUND('',#41,.T.);\
            #61=ADVANCED_FACE('',(#51),#71,.T.);";

        let mesh = parse_step_brep_mesh(text).unwrap();

        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.faces.len(), 2);
        assert_eq!(mesh_dimensions(&mesh), Some([10.0, 5.0, 0.0]));
    }

    #[test]
    fn scad_file_estimates_primitive_dimensions_for_preview() {
        let dir = std::env::temp_dir().join(format!("modelrack-scad-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("bracket.scad");
        std::fs::write(
            &path,
            "cube([18, 32, 6]);\ntranslate([0,0,6]) cylinder(h=12, r=4);\n",
        )
        .unwrap();

        let result = scan_folder(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        assert!(matches!(result.entries[0].stl_type, StlType::Scad));
        assert_eq!(result.entries[0].dimensions, Some([18.0, 32.0, 6.0]));
        assert_eq!(result.meshes.len(), 1);
    }

    #[test]
    fn obj_mesh_remaps_faces_after_invalid_vertices() {
        let mesh = parse_obj_mesh(
            "v 0 0 0\n\
             v nope 0 0\n\
             v 1 0 0\n\
             v 0 1 0\n\
             f 1 3 4\n\
             f 1 2 4\n",
        )
        .unwrap();

        assert_eq!(
            mesh.vertices,
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        );
        assert_eq!(mesh.faces, vec![[0, 1, 2]]);
    }

    #[test]
    fn obj_mesh_skips_entire_face_with_out_of_range_index() {
        let mesh = parse_obj_mesh(
            "v 0 0 0\n\
             v 1 0 0\n\
             v 1 1 0\n\
             v 0 1 0\n\
             f 1 2 999 3\n\
             f 1 2 3 4\n",
        )
        .unwrap();

        assert_eq!(mesh.faces, vec![[0, 1, 2], [0, 2, 3]]);
    }

    #[test]
    fn three_mf_mesh_remaps_faces_after_invalid_vertices() {
        let mesh = parse_three_mf_mesh_xml(
            r#"<model><resources><object id="1"><mesh><vertices>
                <vertex x="0" y="0" z="0"/>
                <vertex x="nan" y="0" z="0"/>
                <vertex x="1" y="0" z="0"/>
                <vertex x="0" y="1" z="0"/>
                </vertices><triangles>
                <triangle v1="0" v2="2" v3="3"/>
                <triangle v1="0" v2="1" v3="3"/>
                </triangles></mesh></object></resources></model>"#,
        )
        .unwrap();

        assert_eq!(
            mesh.vertices,
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        );
        assert_eq!(mesh.faces, vec![[0, 1, 2]]);
    }

    #[test]
    fn binary_stl_with_trailing_bytes_still_parses_mesh_preview() {
        let dir = std::env::temp_dir().join(format!(
            "modelrack-trailing-stl-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("trailing.stl");
        let mut stl = make_binary_stl(&[[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]]);
        stl.extend_from_slice(b"extra metadata");
        std::fs::write(&path, stl).unwrap();

        let result = scan_folder(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(result.entries.len(), 1);
        assert!(matches!(result.entries[0].stl_type, StlType::Binary));
        assert_eq!(result.entries[0].triangle_count, Some(1));
        assert_eq!(result.meshes.len(), 1);
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
