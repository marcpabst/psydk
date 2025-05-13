use std::sync::Arc;

use wgpu::{
    util::DeviceExt, BindGroup, Buffer, Device, Instance, Queue, RenderPipeline, Surface, Texture, TextureFormat,
};
use winit::{dpi::PhysicalSize, window::Window};

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
    lut_texture_array: Texture,
    encode_gamma: bool,
    gamma_buffer: Buffer,
    bind_group: BindGroup,
    size: PhysicalSize<u32>,
}

impl WgpuRenderer {
    pub async fn new(
        window: Arc<Window>,
        _instance: &Instance,
        device: &Device,
        queue: &Queue,
        surface_format: TextureFormat,
        lut: Option<image::RgbImage>,
        encode_gamma: bool,
    ) -> Self {
        let size = window.inner_size();
        let (width, height) = (size.width, size.height);

        // create a render pipeline
        let render_pipeline = Self::create_render_pipelie(&device, surface_format);
        let texture = Self::create_texture(&device, width, height);
        let lut_texture_array = Self::create_lut_texture_array(&device, 256, 256);

        // if a LUT is provided, create a texture array and upload the LUT data
        let lut_texture_data = if let Some(lut) = lut {
            // make sure the LUT is 128x128
            assert_eq!(lut.width(), 256);
            assert_eq!(lut.height(), 256);
            // get u8 data from the LUT
            // the desired structure is 128x128 red, 128x128 green, 128x128 blue
            // the image however has rgb values interleaved
            let mut lut_texture_data = Vec::with_capacity(256 * 256 * 3);
            for c in 0..3 {
                for i in 0..(256 * 256) {
                    // get the pixel value
                    let pixel = lut.get_pixel(i % 256, i / 256);
                    // get the channel value
                    let channel_value = pixel[c];
                    // push the value to the texture data
                    lut_texture_data.push(channel_value);
                }
            }

            lut_texture_data
        } else {
            // create a default LUT based on the sRGB encoding function
            // the LUT is 256x256 red, 256x256 green, 256x256 blue
            let mut lut_texture_data = vec![0u8; 256 * 256 * 3];
            for i in 0..(256 * 256) {
                for c in 0..3 {
                    let x = i as f32 / (256.0 * 256.0);
                    let y = srgb_inverse_eotf(x);
                    let y = (y * 255.0).round() as u8;
                    lut_texture_data[c * (256 * 256) + i] = y;
                }
            }
            lut_texture_data
        };

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
                bytes_per_row: Some(256),
                rows_per_image: Some(256),
            },
            // The size of the texture
            wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 3,
            },
        );

        let gamma_buffer = Self::create_uniform_buffer(&device);
        let bind_group = Self::create_bind_group(&device, &texture, &lut_texture_array, encode_gamma);

        Self {
            surface_format,
            render_pipeline,
            texture,
            lut_texture_array,
            encode_gamma,
            gamma_buffer,
            bind_group,
            size,
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
        self.texture = Self::create_texture(device, width, height);
        self.bind_group = Self::create_bind_group(device, &self.texture, &self.lut_texture_array, self.encode_gamma);
        self.configure_surface(surface, device);
    }

    fn create_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            label: None,
            view_formats: &[wgpu::TextureFormat::Rgba16Float],
        })
    }

    fn create_lut_texture_array(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 3,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: None,
            view_formats: &[wgpu::TextureFormat::R8Unorm],
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
        encode_gamma: bool,
    ) -> wgpu::BindGroup {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Render Bind Group Layout"),
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
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
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
                                correction: if encode_gamma { 1 } else { 0 },
                                texture_width: 256,
                                texture_height: 256,
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
                            dimension: Some(wgpu::TextureViewDimension::D2Array),
                            ..Default::default()
                        },
                    )),
                },
            ],
        })
    }

    fn create_render_pipelie(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Render Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../assets/shaders/render.wgsl").into()),
        });

        // create a bind group layout for texture and sampler
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Render Bind Group Layout"),
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
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
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
