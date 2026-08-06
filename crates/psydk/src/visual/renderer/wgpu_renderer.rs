use std::sync::Arc;

use palette::bool_mask::BoolMask;
use wgpu::{
    util::DeviceExt, BindGroup, Buffer, Device, Instance, Queue, RenderPipeline, Surface, Texture, TextureFormat,
};
use winit::{dpi::PhysicalSize, window::Window};

use super::color_formats::ColorFormat;

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct GammaParams {
    correction: u32,
    texture_width: u32,
    texture_height: u32,
}

pub struct WgpuRenderer {
    surface_format: TextureFormat,
    render_pipeline: RenderPipeline,
    texture: Texture,
    texture_format: TextureFormat,
    lut_texture_array: Texture,
    linear_blending: bool,
    gamma_buffer: Buffer,
    bind_group: BindGroup,
    size: PhysicalSize<u32>,
}

impl WgpuRenderer {
    pub async fn new(
        window: Arc<Box<dyn Window>>,
        _instance: &Instance,
        device: &Device,
        queue: &Queue,
        surface_format: TextureFormat,
        internal_color_format: ColorFormat,
        linear_blending: bool,
        lut: Option<(&[f32], &[f32], &[f32])>,
    ) -> Self {
        let size = window.surface_size();
        let (width, height) = (size.width, size.height);

        // chose an internal texture format based on the provided internal color format
        let internal_texture_format = match internal_color_format {
            ColorFormat::Rgba8 => TextureFormat::Rgba8Unorm,
            ColorFormat::Rgba10 => TextureFormat::Rgb10a2Unorm,
            ColorFormat::RgbaF16 => TextureFormat::Rgba16Float,
        };

        log::debug!(
            "Creating WGPU renderer with surface format: {:?} and internal texture format: {:?}",
            surface_format,
            internal_texture_format
        );

        // create a render pipeline
        let render_pipeline = Self::create_render_pipelie(&device, surface_format);
        let texture = Self::create_texture(&device, width, height, internal_texture_format);
        let lut_texture_array = Self::create_lut_texture(&device, 16384, 1);

        // if a LUT is provided, create a texture array and upload the LUT data
        if let Some((r, g, b)) = lut {
            let mut lut_texture_data = Vec::with_capacity(16384 * 4); // 4 channels (RGBA) per pixel
            for i in 0..16384 {
                lut_texture_data.push(r[i]);
                lut_texture_data.push(g[i]);
                lut_texture_data.push(b[i]);
                lut_texture_data.push(1.0); // alpha channel
            }

            // convert the LUT data to bytes
            let lut_texture_data = bytemuck::cast_slice(&lut_texture_data);
            queue.write_texture(
                // Tells wgpu where to copy the pixel data
                wgpu::TexelCopyTextureInfo {
                    texture: &lut_texture_array,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                // The actual pixel data
                &lut_texture_data,
                // The layout of the texture
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: None, // since this is a 1D texture, we can set bytes_per_row to None
                    rows_per_image: None,
                },
                // The size of the texture
                wgpu::Extent3d {
                    width: 16384,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }

        let gamma_buffer = Self::create_uniform_buffer(&device);
        let bind_group = Self::create_bind_group(&device, &texture, &lut_texture_array, false);

        Self {
            surface_format,
            render_pipeline,
            texture,
            lut_texture_array,
            linear_blending,
            gamma_buffer,
            bind_group,
            size,
            texture_format: internal_texture_format,
        }
    }

    pub fn width(&self) -> u32 {
        self.size.width
    }

    pub fn height(&self) -> u32 {
        self.size.height
    }

    pub fn texture(&self) -> &Texture {
        &self.texture
    }

    pub fn lut_texture_array(&self) -> &Texture {
        &self.lut_texture_array
    }

    pub fn surface_format(&self) -> TextureFormat {
        self.surface_format
    }

    pub fn configure_surface(&self, surface: &Surface, device: &Device) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            // Request compatibility with the sRGB-format texture view we‘re going to create later.
            view_formats: vec![self.surface_format],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            width: self.size.width,
            height: self.size.height,
            desired_maximum_frame_latency: 1,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(device, &surface_config);
    }

    /// Re-size the texture
    pub fn resize(&mut self, width: u32, height: u32, surface: &Surface, device: &Device) {
        self.size = winit::dpi::PhysicalSize::new(width, height);
        self.texture = Self::create_texture(device, width, height, self.texture_format);
        self.bind_group = Self::create_bind_group(device, &self.texture, &self.lut_texture_array, self.linear_blending);
        self.configure_surface(surface, device);
    }

    fn create_texture(device: &wgpu::Device, width: u32, height: u32, texture_format: TextureFormat) -> wgpu::Texture {
        log::debug!(
            "Creating texture with size: {}x{} and format: {:?}",
            width,
            height,
            texture_format
        );
        device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: texture_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST,
            label: Some("Internal Texture"),
            view_formats: &[texture_format],
        })
    }

    fn create_lut_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D1,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("LUT Texture"),
            view_formats: &[wgpu::TextureFormat::Rgba32Float],
        })
    }

    fn create_uniform_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Gamma Buffer"),
            size: std::mem::size_of::<GammaParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn create_bind_group(
        device: &wgpu::Device,
        texture: &wgpu::Texture,
        lut_texture_array: &wgpu::Texture,
        linear_blending: bool,
    ) -> wgpu::BindGroup {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Render Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D1,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Gamma Buffer"),
                            contents: bytemuck::cast_slice(&[GammaParams {
                                correction: {
                                    if linear_blending {
                                        1
                                    } else {
                                        0
                                    }
                                },
                                texture_width: 1,
                                texture_height: 16384,
                            }]),
                            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        }),
                        offset: 0,
                        size: None,
                    }),
                },
                // the LUT texture array
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&lut_texture_array.create_view(
                        &wgpu::TextureViewDescriptor {
                            dimension: Some(wgpu::TextureViewDimension::D1),
                            ..Default::default()
                        },
                    )),
                },
                // a sampler for the LUT texture array
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&device.create_sampler(&wgpu::SamplerDescriptor {
                        label: Some("LUT Sampler"),
                        address_mode_u: wgpu::AddressMode::ClampToEdge,
                        address_mode_v: wgpu::AddressMode::ClampToEdge,
                        address_mode_w: wgpu::AddressMode::ClampToEdge,
                        mag_filter: wgpu::FilterMode::Linear,
                        min_filter: wgpu::FilterMode::Linear,
                        mipmap_filter: wgpu::FilterMode::Nearest,
                        ..Default::default()
                    })),
                },
            ],
        })
    }

    fn create_render_pipelie(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Render Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("./assets/shaders/render.wgsl").into()),
        });

        // create a bind group layout for texture and sampler
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Render Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D1,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some(&"vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(&"fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            cache: None,
        });

        log::debug!("Created render pipeline with surface format: {:?}", format);

        render_pipeline
    }

    pub fn render_to_surface_and_present(&mut self, device: &Device, queue: &Queue, surface: &Surface) {
        // create a new surface texture
        let surface_texture = surface.get_current_texture().unwrap();

        let surface_texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.render_to_texture(device, queue, &surface_texture_view);

        // present the surface
        surface_texture.present();
    }

    pub fn render_to_texture(&mut self, device: &Device, queue: &Queue, texture_view: &wgpu::TextureView) {
        // create a new render pass
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            // bind the render pass
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // bind the render pipeline
            render_pass.set_pipeline(&self.render_pipeline);
            // bind the bind group
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            // draw the quad
            render_pass.draw(0..6, 0..1);
        }

        // submit the render pass
        queue.submit(Some(encoder.finish()));
    }
}

// standard srgb inverse eotf
fn srgb_inverse_eotf(c: f32) -> f32 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

// gamma 2.2 inverse eotf
// this is a simplified version of the gamma 2.2 inverse eotf
// without the precise handling of the 0.04045 threshold
fn gamma22_inverse_eotf(c: f32) -> f32 {
    c.powf(1.0 / 2.2)
}
