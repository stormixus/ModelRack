use egui::{Color32, ColorImage};

use crate::scanner::StlType;

pub fn thumbnail_cache_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|home| {
            std::path::PathBuf::from(home)
                .join("Library")
                .join("Caches")
                .join("ModelRack")
                .join("Thumbnails")
        })
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(|base| {
            std::path::PathBuf::from(base)
                .join("ModelRack")
                .join("Thumbnails")
        })
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".cache"))
            })
            .map(|base| base.join("ModelRack").join("Thumbnails"))
    }
}

pub fn cache_path_for_hash(hash: &[u8; 32]) -> Option<std::path::PathBuf> {
    thumbnail_cache_dir().map(|dir| dir.join(format!("{}.png", hash_hex(hash))))
}

pub fn load_cached_thumbnail(hash: &[u8; 32]) -> Option<ColorImage> {
    let path = cache_path_for_hash(hash)?;
    let image = image::open(path).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = image
        .pixels()
        .map(|p| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
        .collect();
    Some(ColorImage { size, pixels })
}

pub fn save_cached_thumbnail(hash: &[u8; 32], image: &ColorImage) -> std::io::Result<()> {
    let Some(path) = cache_path_for_hash(hash) else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut rgba = image::RgbaImage::new(image.size[0] as u32, image.size[1] as u32);
    for (idx, pixel) in image.pixels.iter().enumerate() {
        let x = (idx % image.size[0]) as u32;
        let y = (idx / image.size[0]) as u32;
        rgba.put_pixel(x, y, image::Rgba(pixel.to_array()));
    }
    rgba.save(&path).map_err(std::io::Error::other)
}

pub fn hash_hex(hash: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in hash {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{:02x}", byte);
    }
    out
}

pub fn generate_error_placeholder(width: usize, height: usize) -> ColorImage {
    let size = [width, height];
    let pixels: Vec<Color32> = (0..(width * height))
        .map(|i| {
            let x = (i % width) as f32 / width as f32;
            let y = (i / width) as f32 / height as f32;
            let checker = ((x * 12.0) as usize ^ (y * 12.0) as usize) & 1;
            if checker == 0 {
                Color32::from_rgb(65, 40, 40)
            } else {
                Color32::from_rgb(50, 30, 30)
            }
        })
        .collect();

    ColorImage { size, pixels }
}

pub fn generate_placeholder(width: usize, height: usize) -> ColorImage {
    generate_tinted_placeholder(width, height, Color32::from_rgb(60, 60, 65), "mesh")
}

pub fn generate_file_placeholder(stl_type: StlType, width: usize, height: usize) -> ColorImage {
    let base = match stl_type {
        StlType::Binary | StlType::Ascii => Color32::from_rgb(35, 64, 72),
        StlType::ThreeMf => Color32::from_rgb(56, 71, 48),
        StlType::Obj => Color32::from_rgb(68, 57, 42),
        StlType::Step => Color32::from_rgb(54, 57, 78),
        StlType::LargeStl => Color32::from_rgb(72, 56, 49),
        StlType::Unknown => Color32::from_rgb(65, 40, 40),
    };
    generate_tinted_placeholder(width, height, base, "file")
}

fn generate_tinted_placeholder(
    width: usize,
    height: usize,
    base: Color32,
    _kind: &str,
) -> ColorImage {
    let size = [width, height];
    let pixels: Vec<Color32> = (0..(width * height))
        .map(|i| {
            let x = (i % width) as f32 / width as f32;
            let y = (i / width) as f32 / height as f32;
            let checker = ((x * 16.0) as usize ^ (y * 16.0) as usize) & 1;
            let shade = if checker == 0 { 1.0 } else { 0.82 };
            Color32::from_rgb(
                (base.r() as f32 * shade) as u8,
                (base.g() as f32 * shade) as u8,
                (base.b() as f32 * shade) as u8,
            )
        })
        .collect();

    ColorImage { size, pixels }
}

/// Generate a wireframe thumbnail from mesh data using an isometric projection
pub fn generate_wireframe(
    vertices: &[[f32; 3]],
    faces: &[[u32; 3]],
    width: usize,
    height: usize,
) -> Option<ColorImage> {
    if vertices.is_empty() || faces.is_empty() {
        return None;
    }

    // Compute 3D bounding box
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in vertices {
        for i in 0..3 {
            min[i] = min[i].min(v[i]);
            max[i] = max[i].max(v[i]);
        }
    }

    // Isometric-like projection
    let (sin_y, cos_y) = (0.6_f32.sin(), 0.6_f32.cos());
    let (sin_x, cos_x) = (0.4_f32.sin(), 0.4_f32.cos());

    let projected: Vec<(f32, f32)> = vertices
        .iter()
        .map(|v| {
            let cx = v[0] - (min[0] + max[0]) / 2.0;
            let cy = v[1] - (min[1] + max[1]) / 2.0;
            let cz = v[2] - (min[2] + max[2]) / 2.0;
            let rx = cx * cos_y + cz * sin_y;
            let rz = -cx * sin_y + cz * cos_y;
            let ry = cy * cos_x - rz * sin_x;
            (rx, ry)
        })
        .collect();

    // Compute 2D bounds
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    for &(x, y) in &projected {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    let padding = 10.0;
    let scale_x = (width as f32 - padding * 2.0) / (max_x - min_x).max(0.001);
    let scale_y = (height as f32 - padding * 2.0) / (max_y - min_y).max(0.001);
    let scale = scale_x.min(scale_y);

    let offset_x = width as f32 / 2.0 - (min_x + max_x) / 2.0 * scale;
    let offset_y = height as f32 / 2.0 - (min_y + max_y) / 2.0 * scale;

    let screen: Vec<(i32, i32)> = projected
        .iter()
        .map(|&(x, y)| ((x * scale + offset_x) as i32, (y * scale + offset_y) as i32))
        .collect();

    let bg = Color32::from_rgb(35, 35, 40);
    let line_color = Color32::from_rgb(180, 200, 220);
    let mut pixels = vec![bg; width * height];

    for face in faces {
        for &(a, b) in &[(0, 1), (1, 2), (2, 0)] {
            let ai = face[a as usize] as usize;
            let bi = face[b as usize] as usize;
            if ai < screen.len() && bi < screen.len() {
                draw_line(
                    &mut pixels,
                    width,
                    height,
                    screen[ai],
                    screen[bi],
                    line_color,
                );
            }
        }
    }

    Some(ColorImage {
        size: [width, height],
        pixels,
    })
}

fn draw_line(
    pixels: &mut [Color32],
    w: usize,
    h: usize,
    p0: (i32, i32),
    p1: (i32, i32),
    color: Color32,
) {
    let (mut x0, mut y0) = p0;
    let (x1, y1) = p1;
    let dx = (x1 - x0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 >= 0 && x0 < w as i32 && y0 >= 0 && y0 < h as i32 {
            pixels[(y0 as usize) * w + x0 as usize] = color;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn single_triangle() -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
        let verts = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let faces = vec![[0, 1, 2]];
        (verts, faces)
    }

    fn cube_mesh() -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
        let verts = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [2, 3, 7],
            [2, 7, 6],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
        ];
        (verts, faces)
    }

    #[test]
    fn wireframe_empty_verts_returns_none() {
        let result = generate_wireframe(&[], &[[0, 1, 2]], 160, 112);
        assert!(result.is_none());
    }

    #[test]
    fn wireframe_empty_faces_returns_none() {
        let result = generate_wireframe(&[[0.0, 0.0, 0.0]], &[], 160, 112);
        assert!(result.is_none());
    }

    #[test]
    fn wireframe_single_triangle_produces_correct_size() {
        let (verts, faces) = single_triangle();
        let result = generate_wireframe(&verts, &faces, 160, 112);
        assert!(result.is_some());
        let img = result.unwrap();
        assert_eq!(img.size, [160, 112]);
        assert_eq!(img.pixels.len(), 160 * 112);
    }

    #[test]
    fn wireframe_cube_does_not_panic() {
        let (verts, faces) = cube_mesh();
        let result = generate_wireframe(&verts, &faces, 160, 112);
        assert!(result.is_some());
    }

    #[test]
    fn wireframe_different_sizes_produce_correct_output() {
        let (verts, faces) = single_triangle();
        let result = generate_wireframe(&verts, &faces, 80, 56);
        assert!(result.is_some());
        let img = result.unwrap();
        assert_eq!(img.size, [80, 56]);
        assert_eq!(img.pixels.len(), 80 * 56);
    }

    #[test]
    fn error_placeholder_is_correct_size() {
        let img = generate_error_placeholder(160, 112);
        assert_eq!(img.size, [160, 112]);
        assert_eq!(img.pixels.len(), 160 * 112);
    }

    #[test]
    fn hash_hex_is_stable_lowercase() {
        let mut hash = [0u8; 32];
        hash[0] = 0xab;
        hash[31] = 0x7f;

        assert_eq!(
            hash_hex(&hash),
            "ab0000000000000000000000000000000000000000000000000000000000007f"
        );
    }
}
