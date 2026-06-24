use eframe::egui_wgpu::{self, CallbackResources, CallbackTrait, RenderState, ScreenDescriptor};
use eframe::wgpu::{self, util::DeviceExt};
use glam::Mat4;

const INSTANCE_CAPACITY: usize = 16_384;
const LINE_CAPACITY: usize = 65_536;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkyInstance {
    pub position: [f32; 3],
    pub size: f32,
    pub color: [f32; 3],
    pub brightness: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    view: [f32; 16],
    proj: [f32; 16],
    gamma: [f32; 4],
}

const QUAD: [[f32; 2]; 6] = [
    [-1.0, -1.0],
    [1.0, -1.0],
    [1.0, 1.0],
    [-1.0, -1.0],
    [1.0, 1.0],
    [-1.0, 1.0],
];

/// GPU resources for the sky: star/body billboards plus constellation and
/// horizon lines, built once on eframe's device.
pub struct SkyRenderer {
    billboards: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    quad_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    line_buffer: wgpu::Buffer,
    needs_gamma: bool,
}

impl SkyRenderer {
    pub fn new(rs: &RenderState) -> Self {
        let device = &rs.device;
        let needs_gamma = !rs.target_format.is_srgb();

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sky.wgsl").into()),
        });

        let quad_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sky_quad"),
            contents: bytemuck::cast_slice(&QUAD),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky_instances"),
            size: (INSTANCE_CAPACITY * std::mem::size_of::<SkyInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let line_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky_lines"),
            size: (LINE_CAPACITY * std::mem::size_of::<LineVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky_uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky_bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky_pl"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let blend = wgpu::BlendState {
            color: ADD,
            alpha: ADD,
        };
        let target = wgpu::ColorTargetState {
            format: rs.target_format,
            blend: Some(blend),
            write_mask: wgpu::ColorWrites::ALL,
        };

        const QUAD_ATTRS: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x2];
        const INSTANCE_ATTRS: [wgpu::VertexAttribute; 4] =
            wgpu::vertex_attr_array![1 => Float32x3, 2 => Float32, 3 => Float32x3, 4 => Float32];
        const LINE_ATTRS: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

        let billboards = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky_billboards"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<[f32; 2]>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &QUAD_ATTRS,
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<SkyInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &INSTANCE_ATTRS,
                    },
                ],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(target.clone())],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        let lines = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky_lines"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_line"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<LineVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &LINE_ATTRS,
                }],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_line"),
                targets: &[Some(target)],
                compilation_options: Default::default(),
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            billboards,
            lines,
            bind_group,
            uniform_buffer,
            quad_buffer,
            instance_buffer,
            line_buffer,
            needs_gamma,
        }
    }
}

const ADD: wgpu::BlendComponent = wgpu::BlendComponent {
    src_factor: wgpu::BlendFactor::One,
    dst_factor: wgpu::BlendFactor::One,
    operation: wgpu::BlendOperation::Add,
};

struct SkyCallback {
    uniforms: Uniforms,
    instances: Vec<SkyInstance>,
    lines: Vec<LineVertex>,
}

impl CallbackTrait for SkyCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(r) = resources.get::<SkyRenderer>() {
            let mut uniforms = self.uniforms;
            uniforms.gamma[0] = if r.needs_gamma { 1.0 } else { 0.0 };
            queue.write_buffer(&r.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
            let instances = self.instances.len().min(INSTANCE_CAPACITY);
            queue.write_buffer(
                &r.instance_buffer,
                0,
                bytemuck::cast_slice(&self.instances[..instances]),
            );
            let lines = self.lines.len().min(LINE_CAPACITY);
            queue.write_buffer(
                &r.line_buffer,
                0,
                bytemuck::cast_slice(&self.lines[..lines]),
            );
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        if let Some(r) = resources.get::<SkyRenderer>() {
            let line_count = self.lines.len().min(LINE_CAPACITY) as u32;
            if line_count > 0 {
                render_pass.set_pipeline(&r.lines);
                render_pass.set_bind_group(0, &r.bind_group, &[]);
                render_pass.set_vertex_buffer(0, r.line_buffer.slice(..));
                render_pass.draw(0..line_count, 0..1);
            }

            let instance_count = self.instances.len().min(INSTANCE_CAPACITY) as u32;
            if instance_count > 0 {
                render_pass.set_pipeline(&r.billboards);
                render_pass.set_bind_group(0, &r.bind_group, &[]);
                render_pass.set_vertex_buffer(0, r.quad_buffer.slice(..));
                render_pass.set_vertex_buffer(1, r.instance_buffer.slice(..));
                render_pass.draw(0..6, 0..instance_count);
            }
        }
    }
}

/// Render the sky (lines beneath billboards) into `rect`.
pub fn show(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    view: Mat4,
    proj: Mat4,
    instances: Vec<SkyInstance>,
    lines: Vec<LineVertex>,
) {
    let uniforms = Uniforms {
        view: view.to_cols_array(),
        proj: proj.to_cols_array(),
        gamma: [0.0; 4],
    };
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        SkyCallback {
            uniforms,
            instances,
            lines,
        },
    ));
}
