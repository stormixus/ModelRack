use egui::ColorImage;
use wgpu::util::DeviceExt;

const RENDER_SIZE: u32 = 256;

pub struct OffscreenRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl OffscreenRenderer {
    pub fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or("No wgpu adapter found")?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("ModelRack thumbnail renderer"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                ..Default::default()
            },
            None,
        ))
        .map_err(|e| format!("Failed to create wgpu device: {}", e))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lambertian shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("uniforms bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lambertian pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 3 * 4,
                            shader_location: 1,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
        })
    }

    pub fn render(&self, vertices: &[[f32; 3]], faces: &[[u32; 3]]) -> Result<ColorImage, String> {
        if vertices.is_empty() || faces.is_empty() {
            return Err("Empty mesh data".to_string());
        }

        // Mesh normalization
        let (normalized, normals) = normalize_mesh(vertices, faces);

        // Build interleaved vertex data: [px, py, pz, nx, ny, nz]
        let vertex_data: Vec<f32> = normalized
            .iter()
            .zip(normals.iter())
            .flat_map(|(p, n)| [p[0], p[1], p[2], n[0], n[1], n[2]])
            .collect();

        let index_data: Vec<u32> = faces.iter().flat_map(|f| [f[0], f[1], f[2]]).collect();

        let vertex_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vertex buffer"),
                contents: bytemuck::cast_slice(&vertex_data),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("index buffer"),
                contents: bytemuck::cast_slice(&index_data),
                usage: wgpu::BufferUsages::INDEX,
            });

        // Uniforms: MVP matrix
        let uniforms = build_uniforms();
        let uniform_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("uniform buffer"),
                contents: bytemuck::cast_slice(&uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniforms bind group"),
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        // Render target texture
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render target"),
            size: wgpu::Extent3d {
                width: RENDER_SIZE,
                height: RENDER_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Readback buffer
        let buffer_bytes = RENDER_SIZE as u64 * RENDER_SIZE as u64 * 4;
        let readback_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback buffer"),
            size: buffer_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Render pass
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("thumbnail render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.14,
                            g: 0.14,
                            b: 0.16,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buf.slice(..));
            render_pass.set_index_buffer(index_buf.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..index_data.len() as u32, 0, 0..1);
        }

        // Copy texture to buffer
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(RENDER_SIZE * 4),
                    rows_per_image: Some(RENDER_SIZE),
                },
            },
            wgpu::Extent3d {
                width: RENDER_SIZE,
                height: RENDER_SIZE,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        // Read back
        let buf_slice = readback_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buf_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|_| "Readback channel closed".to_string())?
            .map_err(|e| format!("Buffer map failed: {}", e))?;

        let data = buf_slice.get_mapped_range();
        let rgba: Vec<u8> = data.to_vec();
        drop(data);
        readback_buf.unmap();

        let pixels: Vec<egui::Color32> = rgba
            .chunks_exact(4)
            .map(|c| egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
            .collect();

        Ok(ColorImage {
            size: [RENDER_SIZE as usize, RENDER_SIZE as usize],
            pixels,
        })
    }
}

/// Compute normals for each vertex by averaging face normals
fn compute_normals(vertices: &[[f32; 3]], faces: &[[u32; 3]]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0f32; 3]; vertices.len()];
    let mut counts = vec![0u32; vertices.len()];

    for face in faces {
        let a = vertices[face[0] as usize];
        let b = vertices[face[1] as usize];
        let c = vertices[face[2] as usize];

        let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 0.0 {
            let nn = [n[0] / len, n[1] / len, n[2] / len];
            for &idx in &[face[0], face[1], face[2]] {
                let i = idx as usize;
                normals[i][0] += nn[0];
                normals[i][1] += nn[1];
                normals[i][2] += nn[2];
                counts[i] += 1;
            }
        }
    }

    for i in 0..normals.len() {
        if counts[i] > 0 {
            let c = counts[i] as f32;
            normals[i][0] /= c;
            normals[i][1] /= c;
            normals[i][2] /= c;
            let len =
                (normals[i][0].powi(2) + normals[i][1].powi(2) + normals[i][2].powi(2)).sqrt();
            if len > 0.0 {
                normals[i][0] /= len;
                normals[i][1] /= len;
                normals[i][2] /= len;
            }
        }
    }

    normals
}

/// Normalize mesh: center at origin, uniform scale to fit viewport
fn normalize_mesh(vertices: &[[f32; 3]], faces: &[[u32; 3]]) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
    // Bounding sphere
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in vertices {
        for i in 0..3 {
            min[i] = min[i].min(v[i]);
            max[i] = max[i].max(v[i]);
        }
    }

    let center = [
        (min[0] + max[0]) / 2.0,
        (min[1] + max[1]) / 2.0,
        (min[2] + max[2]) / 2.0,
    ];

    let extent = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    let max_extent = extent[0].max(extent[1]).max(extent[2]).max(0.0001);

    // 10% padding
    let scale = 1.8 / max_extent;

    let normalized: Vec<[f32; 3]> = vertices
        .iter()
        .map(|v| {
            [
                (v[0] - center[0]) * scale,
                (v[1] - center[1]) * scale,
                (v[2] - center[2]) * scale,
            ]
        })
        .collect();

    let normals = compute_normals(vertices, faces);

    (normalized, normals)
}

/// Build MVP + lighting uniforms
fn build_uniforms() -> [f32; 20] {
    // Isometric camera: 35deg Y rotation + 23deg X rotation
    let angle_y = 35f32.to_radians();
    let angle_x = 23f32.to_radians();

    let (sin_y, cos_y) = (angle_y.sin(), angle_y.cos());
    let (sin_x, cos_x) = (angle_x.sin(), angle_x.cos());

    // View matrix: orbit around origin
    let eye = [3.0 * sin_y * cos_x, 3.0 * sin_x, 3.0 * cos_y * cos_x];
    let target = [0.0, 0.0, 0.0];
    let up = [0.0, 1.0, 0.0];

    let fwd = normalize(sub(target, eye));
    let right = normalize(cross(fwd, up));
    let up_ortho = cross(right, fwd);

    let view: [f32; 16] = [
        right[0],
        up_ortho[0],
        -fwd[0],
        0.0,
        right[1],
        up_ortho[1],
        -fwd[1],
        0.0,
        right[2],
        up_ortho[2],
        -fwd[2],
        0.0,
        -dot(right, eye),
        -dot(up_ortho, eye),
        dot(fwd, eye),
        1.0,
    ];

    // Orthographic projection
    let proj: [f32; 16] = {
        let r = 1.1;
        let l = -r;
        let t = 1.1;
        let b = -t;
        let n = 0.1;
        let f = 10.0;
        [
            2.0 / (r - l),
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 / (t - b),
            0.0,
            0.0,
            0.0,
            0.0,
            -2.0 / (f - n),
            0.0,
            -(r + l) / (r - l),
            -(t + b) / (t - b),
            -(f + n) / (f - n),
            1.0,
        ]
    };

    // MVP
    let mvp = mul_mat4(&proj, &view);

    // Light direction: upper-right-front
    let light_dir = normalize([1.0, 2.0, 1.5]);

    let mut uniforms = [0.0f32; 20];
    uniforms[..16].copy_from_slice(&mvp);
    uniforms[16] = light_dir[0];
    uniforms[17] = light_dir[1];
    uniforms[18] = light_dir[2];
    uniforms[19] = 0.25; // ambient

    uniforms
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 0.0 {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        [0.0, 0.0, 0.0]
    }
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

fn mul_mat4(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut m = [0.0; 16];
    for col in 0..4 {
        for row in 0..4 {
            m[col * 4 + row] = a[row] * b[col * 4]
                + a[4 + row] * b[col * 4 + 1]
                + a[8 + row] * b[col * 4 + 2]
                + a[12 + row] * b[col * 4 + 3];
        }
    }
    m
}

const SHADER_SOURCE: &str = r#"
struct Uniforms {
    mvp: mat4x4<f32>,
    light_dir: vec3<f32>,
    ambient: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = uniforms.mvp * vec4<f32>(in.position, 1.0);

    let normal = normalize(in.normal);
    let light = normalize(uniforms.light_dir);
    let n_dot_l = max(dot(normal, light), 0.0);
    let diffuse = uniforms.ambient + (1.0 - uniforms.ambient) * n_dot_l;

    out.color = vec3<f32>(diffuse, diffuse, diffuse);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_readback_benchmark() {
        let renderer = match OffscreenRenderer::new() {
            Ok(r) => r,
            Err(e) => {
                println!("GPU readback benchmark: SKIP — no GPU device ({})", e);
                return;
            }
        };

        // Simple tetrahedron mesh
        let vertices = vec![
            [0.0, 1.0, 0.0],
            [0.943, -0.333, 0.0],
            [-0.471, -0.333, 0.816],
            [-0.471, -0.333, -0.816],
        ];
        let faces = vec![[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]];

        const SAMPLES: usize = 10;
        let mut times_us: Vec<f64> = Vec::with_capacity(SAMPLES);

        for _ in 0..SAMPLES {
            let start = std::time::Instant::now();
            let image = renderer
                .render(&vertices, &faces)
                .expect("Benchmark render should succeed");
            let elapsed = start.elapsed().as_micros() as f64;
            times_us.push(elapsed);
            assert_eq!(image.size, [256, 256]);
        }

        times_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let min = times_us.first().unwrap();
        let max = times_us.last().unwrap();
        let mean = times_us.iter().sum::<f64>() / SAMPLES as f64;

        println!(
            "GPU readback benchmark: 256x256 RGBA via buffer copy — min={:.0}µs mean={:.0}µs max={:.0}µs ({} samples)",
            min, mean, max, SAMPLES
        );
    }
}
