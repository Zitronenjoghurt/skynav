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

/// Decode the bundled Blue Marble texture and upload it to the GPU.
fn load_earth_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    let image = image::load_from_memory(include_bytes!("../../assets/earth.jpg"))
        .expect("decode earth texture")
        .to_rgba8();
    let (width, height) = image.dimensions();
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("earth_texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
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

    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

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
