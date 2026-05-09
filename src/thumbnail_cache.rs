use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::scanner::{MeshData, StlFileInfo};

const CACHE_VERSION: &str = "v3";
const THUMB_SIZE: u32 = 224;

pub fn ensure_thumbnail(entry: &StlFileInfo, mesh: Option<&MeshData>) -> io::Result<PathBuf> {
    let root = platform_cache_root().join("thumbnails").join(CACHE_VERSION);
    ensure_thumbnail_in(entry, mesh, &root)
}

fn ensure_thumbnail_in(
    entry: &StlFileInfo,
    mesh: Option<&MeshData>,
    root: &Path,
) -> io::Result<PathBuf> {
    let path = thumbnail_path_in(entry, root);
    if path.is_file() {
        return Ok(path);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let pixels = render_thumbnail(entry, mesh, THUMB_SIZE, THUMB_SIZE);
    let png = encode_rgba_png(THUMB_SIZE, THUMB_SIZE, &pixels);
    let tmp_path = path.with_extension("png.tmp");
    fs::write(&tmp_path, png)?;
    fs::rename(&tmp_path, &path)?;
    Ok(path)
}

fn thumbnail_path_in(entry: &StlFileInfo, root: &Path) -> PathBuf {
    root.join(format!("{}-{}.png", hash_hex(&entry.hash), CACHE_VERSION))
}

fn platform_cache_root() -> PathBuf {
    if let Ok(path) = std::env::var("MODELRACK_THUMBNAIL_CACHE_DIR") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Caches")
                .join("ModelRack");
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local_app_data)
                .join("ModelRack")
                .join("Cache");
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(cache_home) = std::env::var_os("XDG_CACHE_HOME") {
            return PathBuf::from(cache_home).join("modelrack");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".cache").join("modelrack");
        }
    }

    std::env::temp_dir().join("modelrack-cache")
}

fn render_thumbnail(
    entry: &StlFileInfo,
    mesh: Option<&MeshData>,
    width: u32,
    height: u32,
) -> Vec<u8> {
    let mut canvas = Canvas::new(width, height);
    let accent = accent_color(&entry.hash);
    let low = [accent[0] / 2, accent[1] / 2, accent[2] / 2, 150];
    let high = [
        accent[0].saturating_add(58),
        accent[1].saturating_add(58),
        accent[2].saturating_add(58),
        235,
    ];

    canvas.draw_soft_ellipse(
        (width as f32 * 0.50) as i32,
        (height as f32 * 0.78) as i32,
        (width as f32 * 0.33) as i32,
        (height as f32 * 0.08) as i32,
        [0, 0, 0, 64],
    );

    if let Some(mesh) = mesh.filter(|mesh| !mesh.vertices.is_empty() && !mesh.faces.is_empty()) {
        draw_mesh_shaded(&mut canvas, mesh, high, low);
    } else {
        draw_dimension_block(&mut canvas, entry, high, low);
    }

    draw_corner_mark(&mut canvas, entry, accent);
    canvas.into_rgba()
}

fn draw_mesh_shaded(canvas: &mut Canvas, mesh: &MeshData, high: [u8; 4], low: [u8; 4]) {
    let projected = project_vertices(&mesh.vertices);
    if projected.is_empty() {
        return;
    }

    let points = fit_projected_points(canvas, &projected);
    let screen_points = points
        .iter()
        .map(|point| [point[0], point[1]])
        .collect::<Vec<_>>();
    let mut faces = mesh
        .faces
        .iter()
        .filter_map(|face| {
            let indices = [face[0] as usize, face[1] as usize, face[2] as usize];
            if indices
                .iter()
                .any(|index| *index >= points.len() || *index >= mesh.vertices.len())
            {
                return None;
            }
            let depth = indices.iter().map(|index| points[*index][2]).sum::<f32>() / 3.0;
            Some((depth, *face))
        })
        .collect::<Vec<_>>();

    faces.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let stride = (faces.len() / 5000).max(1);
    for (_, face) in faces.into_iter().step_by(stride) {
        let fill = shaded_face_color(mesh, face, high, low);
        draw_face_fill(canvas, &screen_points, face, fill);
    }

    draw_mesh_wireframe_points(canvas, mesh, &screen_points, high, low);
}

fn fit_projected_points(canvas: &Canvas, projected: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let (min_x, max_x, min_y, max_y) = bounds_2d(&projected);
    let span_x = (max_x - min_x).max(0.001);
    let span_y = (max_y - min_y).max(0.001);
    let scale = ((canvas.width as f32 * 0.82) / span_x).min((canvas.height as f32 * 0.70) / span_y);
    let center_x = canvas.width as f32 * 0.50;
    let center_y = canvas.height as f32 * 0.48;
    let offset_x = center_x - ((min_x + max_x) * 0.5 * scale);
    let offset_y = center_y - ((min_y + max_y) * 0.5 * scale);

    projected
        .iter()
        .map(|[x, y, z]| [x * scale + offset_x, y * scale + offset_y, *z])
        .collect::<Vec<_>>()
}

fn draw_mesh_wireframe_points(
    canvas: &mut Canvas,
    mesh: &MeshData,
    points: &[[f32; 2]],
    high: [u8; 4],
    low: [u8; 4],
) {
    let stride = (mesh.faces.len() / 1500).max(1);
    for (idx, face) in mesh.faces.iter().enumerate().step_by(stride) {
        let color = if idx % (stride * 3) == 0 { high } else { low };
        draw_face_edges(canvas, &points, *face, color);
    }
}

fn shaded_face_color(mesh: &MeshData, face: [u32; 3], high: [u8; 4], low: [u8; 4]) -> [u8; 4] {
    let a = mesh.vertices[face[0] as usize];
    let b = mesh.vertices[face[1] as usize];
    let c = mesh.vertices[face[2] as usize];
    let normal = cross(sub(b, a), sub(c, a));
    let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2])
        .sqrt()
        .max(0.001);
    let normal = [normal[0] / len, normal[1] / len, normal[2] / len];
    let light = normalize([-0.35, -0.45, 0.82]);
    let shade = (dot(normal, light).abs() * 0.70 + 0.30).clamp(0.0, 1.0);
    [
        lerp_u8(low[0], high[0], shade),
        lerp_u8(low[1], high[1], shade),
        lerp_u8(low[2], high[2], shade),
        150,
    ]
}

fn draw_face_fill(canvas: &mut Canvas, points: &[[f32; 2]], face: [u32; 3], color: [u8; 4]) {
    let indices = [face[0] as usize, face[1] as usize, face[2] as usize];
    if indices.iter().any(|index| *index >= points.len()) {
        return;
    }
    canvas.draw_triangle(
        points[indices[0]],
        points[indices[1]],
        points[indices[2]],
        color,
    );
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(0.001);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn draw_face_edges(canvas: &mut Canvas, points: &[[f32; 2]], face: [u32; 3], color: [u8; 4]) {
    let indices = [face[0] as usize, face[1] as usize, face[2] as usize];
    if indices.iter().any(|index| *index >= points.len()) {
        return;
    }
    for edge in [(0, 1), (1, 2), (2, 0)] {
        let a = points[indices[edge.0]];
        let b = points[indices[edge.1]];
        canvas.draw_line(a[0], a[1], b[0], b[1], color);
    }
}

fn draw_dimension_block(canvas: &mut Canvas, entry: &StlFileInfo, high: [u8; 4], low: [u8; 4]) {
    let dims = entry
        .dimensions
        .unwrap_or_else(|| fallback_dims(&entry.hash));
    let max_dim = dims.iter().copied().fold(0.0_f32, f32::max).max(1.0);
    let normalized = [dims[0] / max_dim, dims[1] / max_dim, dims[2] / max_dim];
    let w = 46.0 + normalized[0] * 58.0;
    let d = 36.0 + normalized[1] * 50.0;
    let h = 40.0 + normalized[2] * 70.0;

    let cx = canvas.width as f32 * 0.50;
    let cy = canvas.height as f32 * 0.58;
    let points = [
        [cx - w * 0.50, cy - d * 0.18],
        [cx, cy - d * 0.44],
        [cx + w * 0.50, cy - d * 0.18],
        [cx, cy + d * 0.10],
        [cx - w * 0.50, cy - d * 0.18 - h],
        [cx, cy - d * 0.44 - h],
        [cx + w * 0.50, cy - d * 0.18 - h],
        [cx, cy + d * 0.10 - h],
    ];

    for (a, b, color) in [
        (0, 1, low),
        (1, 2, high),
        (2, 3, low),
        (3, 0, low),
        (4, 5, high),
        (5, 6, high),
        (6, 7, high),
        (7, 4, high),
        (0, 4, low),
        (1, 5, high),
        (2, 6, high),
        (3, 7, low),
    ] {
        canvas.draw_line(
            points[a][0],
            points[a][1],
            points[b][0],
            points[b][1],
            color,
        );
    }

    let tick_color = [high[0], high[1], high[2], 90];
    for i in 1..4 {
        let t = i as f32 / 4.0;
        let a = lerp_point(points[4], points[5], t);
        let b = lerp_point(points[7], points[6], t);
        canvas.draw_line(a[0], a[1], b[0], b[1], tick_color);
    }
}

fn draw_corner_mark(canvas: &mut Canvas, entry: &StlFileInfo, accent: [u8; 3]) {
    let tris = entry.triangle_count.unwrap_or(0) as u32;
    let bars = 1 + ((tris.max(entry.size as u32) as usize + entry.filename.len()) % 4);
    for i in 0..bars {
        let x = 18 + i as i32 * 8;
        let y = canvas.height as i32 - 26 - i as i32 * 3;
        canvas.draw_rect(
            x,
            y,
            5,
            10 + i as i32 * 3,
            [accent[0], accent[1], accent[2], 110],
        );
    }
}

fn project_vertices(vertices: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let Some((min, max)) = bounds_3d(vertices) else {
        return Vec::new();
    };
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];

    let yaw = -0.62_f32;
    let pitch = -0.48_f32;
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let (sin_pitch, cos_pitch) = pitch.sin_cos();

    vertices
        .iter()
        .filter(|vertex| vertex.iter().all(|value| value.is_finite()))
        .map(|vertex| {
            let x = vertex[0] - center[0];
            let y = vertex[1] - center[1];
            let z = vertex[2] - center[2];
            let rx = x * cos_yaw - y * sin_yaw;
            let ry = x * sin_yaw + y * cos_yaw;
            let rz = z;
            let py = ry * cos_pitch - rz * sin_pitch;
            let pz = ry * sin_pitch + rz * cos_pitch;
            [rx, -py, pz]
        })
        .collect()
}

fn bounds_2d(points: &[[f32; 3]]) -> (f32, f32, f32, f32) {
    points.iter().fold(
        (f32::MAX, f32::MIN, f32::MAX, f32::MIN),
        |(min_x, max_x, min_y, max_y), [x, y, _]| {
            (min_x.min(*x), max_x.max(*x), min_y.min(*y), max_y.max(*y))
        },
    )
}

fn bounds_3d(vertices: &[[f32; 3]]) -> Option<([f32; 3], [f32; 3])> {
    let first = *vertices.first()?;
    let (mut min, mut max) = (first, first);
    for vertex in vertices {
        for axis in 0..3 {
            min[axis] = min[axis].min(vertex[axis]);
            max[axis] = max[axis].max(vertex[axis]);
        }
    }
    Some((min, max))
}

fn fallback_dims(hash: &[u8; 32]) -> [f32; 3] {
    [
        30.0 + hash[0] as f32,
        30.0 + hash[7] as f32,
        30.0 + hash[13] as f32,
    ]
}

fn accent_color(hash: &[u8; 32]) -> [u8; 3] {
    [
        74u8.saturating_add(hash[3] % 96),
        132u8.saturating_add(hash[11] % 90),
        172u8.saturating_add(hash[19] % 70),
    ]
}

fn lerp_point(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn hash_hex(hash: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in hash {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

struct Canvas {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; (width * height * 4) as usize],
        }
    }

    fn into_rgba(self) -> Vec<u8> {
        self.data
    }

    fn draw_soft_ellipse(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, color: [u8; 4]) {
        for y in (cy - ry)..=(cy + ry) {
            for x in (cx - rx)..=(cx + rx) {
                let dx = (x - cx) as f32 / rx.max(1) as f32;
                let dy = (y - cy) as f32 / ry.max(1) as f32;
                let dist = dx * dx + dy * dy;
                if dist <= 1.0 {
                    let mut c = color;
                    c[3] = ((1.0 - dist) * color[3] as f32) as u8;
                    self.blend_pixel(x, y, c);
                }
            }
        }
    }

    fn draw_rect(&mut self, x: i32, y: i32, width: i32, height: i32, color: [u8; 4]) {
        for yy in y..(y + height) {
            for xx in x..(x + width) {
                self.blend_pixel(xx, yy, color);
            }
        }
    }

    fn draw_triangle(&mut self, a: [f32; 2], b: [f32; 2], c: [f32; 2], color: [u8; 4]) {
        let min_x = a[0].min(b[0]).min(c[0]).floor().max(0.0) as i32;
        let max_x = a[0]
            .max(b[0])
            .max(c[0])
            .ceil()
            .min((self.width.saturating_sub(1)) as f32) as i32;
        let min_y = a[1].min(b[1]).min(c[1]).floor().max(0.0) as i32;
        let max_y = a[1]
            .max(b[1])
            .max(c[1])
            .ceil()
            .min((self.height.saturating_sub(1)) as f32) as i32;
        let area = edge_function(a, b, c);
        if area.abs() < 0.001 {
            return;
        }

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let w0 = edge_function(b, c, p);
                let w1 = edge_function(c, a, p);
                let w2 = edge_function(a, b, p);
                if (area > 0.0 && w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0)
                    || (area < 0.0 && w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0)
                {
                    self.blend_pixel(x, y, color);
                }
            }
        }
    }

    fn draw_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: [u8; 4]) {
        let mut x0 = x0.round() as i32;
        let mut y0 = y0.round() as i32;
        let x1 = x1.round() as i32;
        let y1 = y1.round() as i32;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            self.blend_pixel(x0, y0, color);
            self.blend_pixel(x0 + 1, y0, [color[0], color[1], color[2], color[3] / 3]);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn blend_pixel(&mut self, x: i32, y: i32, color: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 || color[3] == 0 {
            return;
        }
        let idx = ((y as u32 * self.width + x as u32) * 4) as usize;
        let alpha = color[3] as f32 / 255.0;
        let inv = 1.0 - alpha;
        for (channel, value) in color.iter().take(3).enumerate() {
            self.data[idx + channel] =
                ((*value as f32 * alpha) + (self.data[idx + channel] as f32 * inv)) as u8;
        }
        self.data[idx + 3] = (color[3] as f32 + self.data[idx + 3] as f32 * inv).min(255.0) as u8;
    }
}

fn edge_function(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (c[0] - a[0]) * (b[1] - a[1]) - (c[1] - a[1]) * (b[0] - a[0])
}

fn encode_rgba_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    debug_assert_eq!(rgba.len(), (width * height * 4) as usize);
    let mut scanlines = Vec::with_capacity((height * (width * 4 + 1)) as usize);
    for row in 0..height as usize {
        scanlines.push(0);
        let start = row * width as usize * 4;
        scanlines.extend_from_slice(&rgba[start..start + width as usize * 4]);
    }

    let mut png = Vec::new();
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_chunk(&mut png, b"IHDR", &ihdr);
    write_chunk(&mut png, b"IDAT", &zlib_store(&scanlines));
    write_chunk(&mut png, b"IEND", &[]);
    png
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 65_535 * 5 + 6);
    out.extend_from_slice(&[0x78, 0x01]);
    let mut remaining = data;
    while !remaining.is_empty() {
        let block_len = remaining.len().min(65_535);
        let final_block = block_len == remaining.len();
        out.push(if final_block { 0x01 } else { 0x00 });
        let len = block_len as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(&remaining[..block_len]);
        remaining = &remaining[block_len..];
    }
    if data.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xff, 0xff]);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn write_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(kind.len() + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    png.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + *byte as u32) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = if crc & 1 == 1 { 0xedb8_8320 } else { 0 };
            crc = (crc >> 1) ^ mask;
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::{SidecarMeta, StlType};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn entry(hash_byte: u8) -> StlFileInfo {
        StlFileInfo {
            path: PathBuf::from(format!("/tmp/model-{hash_byte}.stl")),
            filename: format!("model-{hash_byte}.stl"),
            size: 42,
            hash: [hash_byte; 32],
            stl_type: StlType::Binary,
            triangle_count: Some(12),
            dimensions: Some([20.0, 30.0, 10.0]),
            modified: None,
            thumbnail_path: None,
            meta: Some(SidecarMeta::default()),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("modelrack-thumb-{name}-{nanos}"))
    }

    #[test]
    fn thumbnail_paths_are_hash_addressed() {
        let root = temp_dir("path");
        let first = thumbnail_path_in(&entry(1), &root);
        let second = thumbnail_path_in(&entry(2), &root);

        assert_ne!(first, second);
        assert!(first.ends_with(format!("{}-{}.png", hash_hex(&[1; 32]), CACHE_VERSION)));
    }

    #[test]
    fn ensure_thumbnail_writes_png_and_reuses_cache_hit() {
        let root = temp_dir("reuse");
        let entry = entry(9);

        let first = ensure_thumbnail_in(&entry, None, &root).unwrap();
        let first_meta = fs::metadata(&first).unwrap().modified().unwrap();
        let data = fs::read(&first).unwrap();
        assert!(data.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(data.windows(4).any(|chunk| chunk == b"IHDR"));
        assert!(data.windows(4).any(|chunk| chunk == b"IDAT"));

        let second = ensure_thumbnail_in(&entry, None, &root).unwrap();
        let second_meta = fs::metadata(&second).unwrap().modified().unwrap();
        assert_eq!(first, second);
        assert_eq!(first_meta, second_meta);

        let _ = fs::remove_dir_all(root);
    }
}
