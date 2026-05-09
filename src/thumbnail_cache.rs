use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::scanner::{MeshData, StlFileInfo};

const CACHE_VERSION: &str = "v9";
const THUMB_SIZE: u32 = 224;
const MAX_SHADED_RENDER_FACES: usize = 160_000;

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
    render_preview_rgba(entry, mesh, width, height, -0.62, -0.48)
}

pub(crate) fn render_preview_rgba(
    entry: &StlFileInfo,
    mesh: Option<&MeshData>,
    width: u32,
    height: u32,
    yaw: f32,
    pitch: f32,
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
        let stats = draw_mesh_shaded(&mut canvas, mesh, high, low, yaw, pitch);
        if !stats.drew_geometry() {
            draw_dimension_block(&mut canvas, entry, high, low);
        }
    } else {
        draw_dimension_block(&mut canvas, entry, high, low);
    }

    draw_corner_mark(&mut canvas, entry, accent);
    canvas.into_rgba()
}

#[derive(Default)]
struct MeshDrawStats {
    submitted_faces: usize,
    filled_faces: usize,
}

impl MeshDrawStats {
    fn drew_geometry(&self) -> bool {
        self.filled_faces > 0
    }
}

fn draw_mesh_shaded(
    canvas: &mut Canvas,
    mesh: &MeshData,
    high: [u8; 4],
    low: [u8; 4],
    yaw: f32,
    pitch: f32,
) -> MeshDrawStats {
    let Some(mesh) = mesh.compacted() else {
        return MeshDrawStats::default();
    };

    let projected = project_vertices(&mesh.vertices, yaw, pitch);
    if projected.is_empty() {
        return MeshDrawStats::default();
    }

    let points = fit_projected_points(canvas, &projected);
    let vertex_colors = smooth_vertex_colors(&mesh, high, low);
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
    let stride = (faces.len() / MAX_SHADED_RENDER_FACES).max(1);
    let mut stats = MeshDrawStats::default();
    for (_, face) in faces.into_iter().step_by(stride) {
        stats.submitted_faces += 1;
        if draw_face_fill(canvas, &points, &vertex_colors, face) {
            stats.filled_faces += 1;
        }
    }

    stats
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

fn smooth_vertex_colors(mesh: &MeshData, high: [u8; 4], low: [u8; 4]) -> Vec<[u8; 4]> {
    let face_normals = mesh
        .faces
        .iter()
        .map(|face| face_normal(mesh, *face))
        .collect::<Vec<_>>();
    let Some((min, max)) = bounds_3d(&mesh.vertices) else {
        return vec![high; mesh.vertices.len()];
    };
    let epsilon = (distance(min, max).max(1.0) * 0.00001).max(0.0001);
    let normal_buckets = normal_buckets(mesh, &face_normals, epsilon);
    let reference_normals = reference_normals(mesh, &face_normals);
    let light = normalize([-0.35, -0.45, 0.82]);

    mesh.vertices
        .iter()
        .enumerate()
        .map(|(vertex_index, vertex)| {
            let normal = smoothed_vertex_normal(
                *vertex,
                reference_normals[vertex_index],
                epsilon,
                &normal_buckets,
            );
            shade_color(normal, light, high, low)
        })
        .collect()
}

fn face_normal(mesh: &MeshData, face: [u32; 3]) -> [f32; 3] {
    let a = mesh.vertices[face[0] as usize];
    let b = mesh.vertices[face[1] as usize];
    let c = mesh.vertices[face[2] as usize];
    normalize(cross(sub(b, a), sub(c, a)))
}

fn normal_buckets(
    mesh: &MeshData,
    face_normals: &[[f32; 3]],
    epsilon: f32,
) -> HashMap<[i64; 3], Vec<[f32; 3]>> {
    let mut buckets = HashMap::<[i64; 3], Vec<[f32; 3]>>::new();

    for (face_index, face) in mesh.faces.iter().enumerate() {
        let normal = face_normals[face_index];
        for vertex_index in face {
            let vertex = mesh.vertices[*vertex_index as usize];
            buckets
                .entry(quantized_vertex_key(vertex, epsilon))
                .or_default()
                .push(normal);
        }
    }

    buckets
}

fn smoothed_vertex_normal(
    vertex: [f32; 3],
    reference: [f32; 3],
    epsilon: f32,
    normal_buckets: &HashMap<[i64; 3], Vec<[f32; 3]>>,
) -> [f32; 3] {
    let key = quantized_vertex_key(vertex, epsilon);
    let Some(candidates) = normal_buckets.get(&key) else {
        return reference;
    };
    let mut sum = [0.0, 0.0, 0.0];

    for candidate in candidates {
        let alignment = dot(reference, *candidate);
        if alignment.abs() < 0.35 {
            continue;
        }
        let aligned = if alignment < 0.0 {
            [-candidate[0], -candidate[1], -candidate[2]]
        } else {
            *candidate
        };
        sum = add(sum, aligned);
    }

    let normal = normalize(sum);
    if normal.iter().all(|value| value.is_finite()) {
        normal
    } else {
        reference
    }
}

fn reference_normals(mesh: &MeshData, face_normals: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0, 0.0, 1.0]; mesh.vertices.len()];
    for (face_index, face) in mesh.faces.iter().enumerate() {
        for vertex_index in face {
            normals[*vertex_index as usize] = face_normals[face_index];
        }
    }
    normals
}

fn quantized_vertex_key(vertex: [f32; 3], epsilon: f32) -> [i64; 3] {
    [
        (vertex[0] / epsilon).round() as i64,
        (vertex[1] / epsilon).round() as i64,
        (vertex[2] / epsilon).round() as i64,
    ]
}

fn shade_color(normal: [f32; 3], light: [f32; 3], high: [u8; 4], low: [u8; 4]) -> [u8; 4] {
    let shade = (dot(normal, light).abs() * 0.64 + 0.36).clamp(0.0, 1.0);
    [
        lerp_u8(low[0], high[0], shade),
        lerp_u8(low[1], high[1], shade),
        lerp_u8(low[2], high[2], shade),
        245,
    ]
}

fn draw_face_fill(
    canvas: &mut Canvas,
    points: &[[f32; 3]],
    colors: &[[u8; 4]],
    face: [u32; 3],
) -> bool {
    let indices = [face[0] as usize, face[1] as usize, face[2] as usize];
    if indices
        .iter()
        .any(|index| *index >= points.len() || *index >= colors.len())
    {
        return false;
    }
    canvas.draw_triangle_shaded_depth(
        points[indices[0]],
        points[indices[1]],
        points[indices[2]],
        colors[indices[0]],
        colors[indices[1]],
        colors[indices[2]],
    )
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let delta = sub(a, b);
    (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt()
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

fn interpolate_color(a: [u8; 4], b: [u8; 4], c: [u8; 4], wa: f32, wb: f32, wc: f32) -> [u8; 4] {
    let channel = |idx: usize| {
        (a[idx] as f32 * wa + b[idx] as f32 * wb + c[idx] as f32 * wc)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [channel(0), channel(1), channel(2), channel(3)]
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

fn project_vertices(vertices: &[[f32; 3]], yaw: f32, pitch: f32) -> Vec<[f32; 3]> {
    let Some((min, max)) = bounds_3d(vertices) else {
        return Vec::new();
    };
    let center = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];

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
    depth: Vec<f32>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; (width * height * 4) as usize],
            depth: vec![f32::NEG_INFINITY; (width * height) as usize],
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

    fn draw_triangle_shaded_depth(
        &mut self,
        a: [f32; 3],
        b: [f32; 3],
        c: [f32; 3],
        color_a: [u8; 4],
        color_b: [u8; 4],
        color_c: [u8; 4],
    ) -> bool {
        let a2 = [a[0], a[1]];
        let b2 = [b[0], b[1]];
        let c2 = [c[0], c[1]];
        let min_x = a2[0].min(b2[0]).min(c2[0]).floor().max(0.0) as i32;
        let max_x = a2[0]
            .max(b2[0])
            .max(c2[0])
            .ceil()
            .min((self.width.saturating_sub(1)) as f32) as i32;
        let min_y = a2[1].min(b2[1]).min(c2[1]).floor().max(0.0) as i32;
        let max_y = a2[1]
            .max(b2[1])
            .max(c2[1])
            .ceil()
            .min((self.height.saturating_sub(1)) as f32) as i32;
        let area = edge_function(a2, b2, c2);
        if area.abs() < 0.001 {
            return false;
        }

        let mut drew = false;
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let w0 = edge_function(b2, c2, p);
                let w1 = edge_function(c2, a2, p);
                let w2 = edge_function(a2, b2, p);
                if (area > 0.0 && w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0)
                    || (area < 0.0 && w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0)
                {
                    let alpha0 = w0 / area;
                    let alpha1 = w1 / area;
                    let alpha2 = w2 / area;
                    let depth = alpha0 * a[2] + alpha1 * b[2] + alpha2 * c[2];
                    let color =
                        interpolate_color(color_a, color_b, color_c, alpha0, alpha1, alpha2);
                    drew |= self.blend_pixel_if_front(x, y, depth, color);
                }
            }
        }
        drew
    }

    fn draw_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: [u8; 4]) -> bool {
        let mut x0 = x0.round() as i32;
        let mut y0 = y0.round() as i32;
        let x1 = x1.round() as i32;
        let y1 = y1.round() as i32;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut drew = false;

        loop {
            drew |= self.blend_pixel(x0, y0, color);
            drew |= self.blend_pixel(x0 + 1, y0, [color[0], color[1], color[2], color[3] / 3]);
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
        drew
    }

    fn blend_pixel_if_front(&mut self, x: i32, y: i32, depth: f32, color: [u8; 4]) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 || !depth.is_finite()
        {
            return false;
        }
        let depth_idx = (y as u32 * self.width + x as u32) as usize;
        if depth + 0.0001 < self.depth[depth_idx] {
            return false;
        }
        self.depth[depth_idx] = depth;
        self.blend_pixel(x, y, color)
    }

    fn blend_pixel(&mut self, x: i32, y: i32, color: [u8; 4]) -> bool {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 || color[3] == 0 {
            return false;
        }
        let idx = ((y as u32 * self.width + x as u32) * 4) as usize;
        let alpha = color[3] as f32 / 255.0;
        let inv = 1.0 - alpha;
        for (channel, value) in color.iter().take(3).enumerate() {
            self.data[idx + channel] =
                ((*value as f32 * alpha) + (self.data[idx + channel] as f32 * inv)) as u8;
        }
        self.data[idx + 3] = (color[3] as f32 + self.data[idx + 3] as f32 * inv).min(255.0) as u8;
        true
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
            three_mf_plate_count: None,
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

    #[test]
    fn render_thumbnail_skips_invalid_vertices_without_losing_valid_faces() {
        let mesh = MeshData {
            vertices: vec![
                [0.0, 0.0, 0.0],
                [f32::NAN, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            faces: vec![[0, 2, 3], [0, 1, 3]],
        };

        let pixels = render_thumbnail(&entry(17), Some(&mesh), THUMB_SIZE, THUMB_SIZE);
        let fallback_pixels = render_thumbnail(&entry(17), None, THUMB_SIZE, THUMB_SIZE);
        let different_pixels = pixels
            .chunks_exact(4)
            .zip(fallback_pixels.chunks_exact(4))
            .filter(|(with_mesh, fallback)| with_mesh != fallback)
            .count();

        let mut canvas = Canvas::new(THUMB_SIZE, THUMB_SIZE);
        let stats = draw_mesh_shaded(
            &mut canvas,
            &mesh,
            [220, 230, 240, 235],
            [80, 90, 100, 150],
            -0.62,
            -0.48,
        );

        assert!(different_pixels > 1_000);
        assert!(stats.filled_faces > 0);
    }

    #[test]
    fn shaded_renderer_does_not_decimate_common_high_poly_meshes() {
        let face_count = 12_900usize;
        let mut vertices = Vec::with_capacity(face_count * 3);
        let mut faces = Vec::with_capacity(face_count);
        for idx in 0..face_count {
            let angle = idx as f32 * 0.013;
            let radius = 20.0 + (idx % 37) as f32 * 0.03;
            let base = vertices.len() as u32;
            vertices.push([angle.cos() * radius, angle.sin() * radius, 0.0]);
            vertices.push([
                (angle + 0.01).cos() * (radius + 0.2),
                (angle + 0.01).sin() * (radius + 0.2),
                0.6,
            ]);
            vertices.push([
                (angle + 0.02).cos() * radius,
                (angle + 0.02).sin() * radius,
                0.0,
            ]);
            faces.push([base, base + 1, base + 2]);
        }
        let mesh = MeshData { vertices, faces };
        let mut canvas = Canvas::new(THUMB_SIZE, THUMB_SIZE);

        let stats = draw_mesh_shaded(
            &mut canvas,
            &mesh,
            [220, 230, 240, 235],
            [80, 90, 100, 150],
            -0.62,
            -0.48,
        );

        assert_eq!(stats.submitted_faces, face_count);
        assert!(stats.filled_faces > 5_000);
    }

    #[test]
    fn render_thumbnail_falls_back_when_mesh_has_no_drawable_faces() {
        let mesh = MeshData {
            vertices: vec![[f32::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            faces: vec![[0, 1, 2]],
        };

        let with_non_drawable_mesh =
            render_thumbnail(&entry(23), Some(&mesh), THUMB_SIZE, THUMB_SIZE);
        let fallback = render_thumbnail(&entry(23), None, THUMB_SIZE, THUMB_SIZE);

        assert_eq!(with_non_drawable_mesh, fallback);
    }

    #[test]
    fn thumbnail_cache_version_reflects_renderer_contract() {
        assert_eq!(CACHE_VERSION, "v9");
    }
}
