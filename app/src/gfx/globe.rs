use eframe::egui_wgpu::{self, CallbackResources, CallbackTrait, RenderState, ScreenDescriptor};
use eframe::wgpu::{self, util::DeviceExt};
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    view_proj: [f32; 16],
    model: [f32; 16],
    sun_dir: [f32; 4],
    base_color: [f32; 4],
}

impl Uniforms {
    fn new(view_proj: Mat4, model: Mat4, sun_dir: Vec3, base_color: [f32; 3]) -> Self {
        let s = sun_dir.normalize_or_zero();
        Self {
            view_proj: view_proj.to_cols_array(),
            model: model.to_cols_array(),
            sun_dir: [s.x, s.y, s.z, 0.0],
            base_color: [base_color[0], base_color[1], base_color[2], 1.0],
        }
    }
}

/// GPU resources for the globe, built once on eframe's wgpu device and stored
/// in the renderer's callback resources.
pub struct GlobeRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    needs_gamma: bool,
}

impl GlobeRenderer {
    pub fn new(rs: &RenderState) -> Self {
        let device = &rs.device;
        let needs_gamma = !rs.target_format.is_srgb();

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("globe_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("globe.wgsl").into()),
        });

        let (vertices, indices) = unit_sphere(64, 96);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globe_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("globe_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globe_uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let texture_view = load_earth_texture(&rs.device, &rs.queue);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("globe_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            // Sharp obliquely-viewed coastlines without shimmer at distance.
            anisotropy_clamp: 16,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globe_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globe_bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("globe_pl"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        const ATTRS: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("globe_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &ATTRS,
                }],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: rs.target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            uniform_buffer,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            needs_gamma,
        }
    }
}

/// Per-frame data submitted with the paint callback.
struct GlobeCallback {
    uniforms: Uniforms,
}

impl CallbackTrait for GlobeCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(renderer) = resources.get::<GlobeRenderer>() {
            let mut uniforms = self.uniforms;
            uniforms.base_color[3] = if renderer.needs_gamma { 1.0 } else { 0.0 };
            queue.write_buffer(&renderer.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        if let Some(r) = resources.get::<GlobeRenderer>() {
            render_pass.set_pipeline(&r.pipeline);
            render_pass.set_bind_group(0, &r.bind_group, &[]);
            render_pass.set_vertex_buffer(0, r.vertex_buffer.slice(..));
            render_pass.set_index_buffer(r.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..r.index_count, 0, 0..1);
        }
    }
}

/// Enqueue a lit globe into `rect`. Model rotation orients the surface;
/// `sun_dir` is the direction to the Sun in the render frame.
pub fn show(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    view_proj: Mat4,
    model: Mat4,
    sun_dir: Vec3,
    base_color: [f32; 3],
) {
    let uniforms = Uniforms::new(view_proj, model, sun_dir, base_color);
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        GlobeCallback { uniforms },
    ));
}

/// Bundled Blue Marble texture: a 16k-wide image on native (full resolution on
/// capable GPUs), and the lighter 8k image on the web to keep the WASM payload
/// reasonable.
#[cfg(not(target_arch = "wasm32"))]
const EARTH_JPG: &[u8] = include_bytes!("../../assets/earth_16k.jpg");
#[cfg(target_arch = "wasm32")]
const EARTH_JPG: &[u8] = include_bytes!("../../assets/earth.jpg");

/// Decode the bundled Blue Marble texture, build a full mip chain and upload it.
/// The image is downscaled to fit the device's `max_texture_dimension_2d` so the
/// high-resolution asset still loads on GPUs (and WebGPU) that cap below 16k.
fn load_earth_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let mut image = image::load_from_memory(EARTH_JPG)
        .expect("decode earth texture")
        .to_rgba8();

    let max_dim = device.limits().max_texture_dimension_2d;
    if image.width() > max_dim || image.height() > max_dim {
        let scale = max_dim as f32 / image.width().max(image.height()) as f32;
        let w = (image.width() as f32 * scale) as u32;
        let h = (image.height() as f32 * scale) as u32;
        image = image::imageops::resize(&image, w, h, image::imageops::FilterType::Triangle);
    }

    let (width, height) = image.dimensions();
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let mip_level_count = (width.max(height).max(1)).ilog2() + 1;

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("earth_texture"),
        size,
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &image,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        size,
    );

    generate_mipmaps(device, queue, &texture, mip_level_count);
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Fill mip levels 1.. by successively half-resolution blits. Sampling/storing
/// through the sRGB format keeps the box filter in linear light.
fn generate_mipmaps(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    levels: u32,
) {
    if levels < 2 {
        return;
    }
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mip_blit"),
        source: wgpu::ShaderSource::Wgsl(MIP_BLIT_WGSL.into()),
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("mip_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mip_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mip_pl"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mip_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        multiview_mask: None,
        cache: None,
    });

    let views: Vec<wgpu::TextureView> = (0..levels)
        .map(|level| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: level,
                mip_level_count: Some(1),
                ..Default::default()
            })
        })
        .collect();

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mip_encoder"),
    });
    for target in 1..levels as usize {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mip_bg"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&views[target - 1]),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mip_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &views[target],
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
    queue.submit([encoder.finish()]);
}

/// Fullscreen-triangle blit that samples the previous mip level.
const MIP_BLIT_WGSL: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
    var out: VsOut;
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    out.uv = uv;
    out.pos = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.uv.y = 1.0 - out.uv.y;
    return out;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(src, samp, in.uv);
}
"#;

fn unit_sphere(stacks: u32, slices: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for i in 0..=stacks {
        let v = i as f32 / stacks as f32;
        let phi = v * std::f32::consts::PI;
        let (sp, cp) = phi.sin_cos();
        for j in 0..=slices {
            let s = j as f32 / slices as f32;
            let theta = s * std::f32::consts::TAU;
            let (st, ct) = theta.sin_cos();
            // z = up (north pole), matching the equatorial render frame.
            let pos = [sp * ct, sp * st, cp];
            // Equirectangular UV: prime meridian (theta=0) at the texture centre,
            // north pole at v=0.
            vertices.push(Vertex {
                position: pos,
                normal: pos,
                uv: [s + 0.5, v],
            });
        }
    }

    let row = slices + 1;
    for i in 0..stacks {
        for j in 0..slices {
            let a = i * row + j;
            let b = a + row;
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }

    (vertices, indices)
}
