/// Note that we only use skia's colour management for converting betwen
/// srgb and linear srgb. We do not use it for any other colour space conversions.
use std::{any::Any, cell::RefCell, sync::Arc};

use cosmic_text::fontdb::FaceInfo;
use foreign_types_shared::ForeignType;
use std::sync::Mutex;
use windows_core::Interface;

pub use skia_safe;

#[cfg(target_os = "windows")]
use skia_safe::gpu::{d3d, d3d::BackendContext, Protected};
#[cfg(any(target_os = "macos", target_os = "ios"))]
use skia_safe::graphite::{self, mtl, mtl::BackendContext};
use skia_safe::{svg::Dom, PathDirection, PathVerb};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC, DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
};

use skia_safe::{
    gpu::{self, backend_formats, DirectContext, SurfaceOrigin},
    gradient_shader::{
        linear as sk_linear, radial as sk_radial, sweep as sk_sweep, GradientShaderColors as SkGradientShaderColors,
    },
    image::Image as SkImage,
    images::raster_from_data as sk_raster_from_data,
    scalar, AlphaType as SkAlphaType, ColorType, Font as SkFont, Matrix, PictureRecorder, SamplingOptions,
    Typeface as SkTypeface,
};
// use wgpu::{Adapter, Device, Queue, Texture};

#[cfg(target_os = "windows")]
use crate::color_formats;

use super::{
    affine::Affine,
    brushes::{Brush, Extend, Gradient, GradientKind, ImageSampling},
    color_formats::{ColorEncoding, ColorFormat},
    colors::RGBA,
    styles::{BlendMode, ImageFitMode, StrokeStyle},
};

#[derive(Debug, Clone)]
pub struct Scene {
    pub picture_recorder: Arc<Mutex<PictureRecorder>>,
    pub width: u32,
    pub height: u32,
    pub bg_color: RGBA,
    pub font_collection: Arc<Mutex<skia_safe::textlayout::FontCollection>>,
}

unsafe impl Send for Scene {}
unsafe impl Sync for Scene {}

#[derive(Debug, Clone)]
pub struct Renderer {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    context: RefCell<graphite::Context>,
    #[cfg(target_os = "windows")]
    context: RefCell<gpu::DirectContext>,
    backend: Arc<RefCell<BackendContext>>,
    // todo remove font manager
    font_manager: skia_safe::FontMgr,
    pub font_collection: Arc<Mutex<skia_safe::textlayout::FontCollection>>,
    internal_color_encoding: ColorEncoding,
    internal_color_format: ColorFormat,
}

// TODO: make Renderer Send and Sync properly
unsafe impl Send for Renderer {}
unsafe impl Sync for Renderer {}

#[derive(Debug, Clone)]
/// A Bitmap that is backed by a Skia image.
pub struct Bitmap {
    image: SkImage,
}

#[derive(Debug)]
enum BitmapData {
    Blob(Box<[u8]>),
    Texture(wgpu::Texture),
}

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl From<(f32, f32)> for Point {
    fn from(value: (f32, f32)) -> Self {
        Self { x: value.0, y: value.1 }
    }
}

#[derive(Debug)]
/// An SVG.
pub struct SVG {
    pub dom: skia_safe::svg::Dom,
}

#[derive(Debug)]
pub struct Typeface {
    pub typeface: SkTypeface,
}

/// A Glyph.
#[derive(Debug, Clone)]
pub struct Glyph {
    pub id: u16,
    pub position: Point,
}

impl Scene {
    pub fn new(width: u32, height: u32, font_collection: Arc<Mutex<skia_safe::textlayout::FontCollection>>) -> Self {
        let mut picture_recorder = PictureRecorder::new();
        let bounds = skia_safe::Rect::from_wh(width as f32, height as f32);
        picture_recorder.begin_recording(bounds, false);

        // clear the canvas
        let canvas = picture_recorder.recording_canvas().unwrap();
        canvas.clear(skia_safe::Color4f::new(1.0, 1.0, 1.0, 1.0));

        Self {
            picture_recorder: Arc::new(Mutex::new(picture_recorder)),
            width,
            height,
            bg_color: RGBA::WHITE,
            font_collection,
        }
    }

    pub fn draw_rectangle(
        skia_canvas: &skia_safe::Canvas,
        skia_paint: skia_safe::Paint,
        a: Point,
        w: f32,
        h: f32,
        affine: Option<Affine>,
    ) {
        if let Some(affine) = affine {
            skia_canvas.save();
            skia_canvas.concat(&affine.into());
        }

        let rect = skia_safe::Rect::from_xywh(a.x as f32, a.y as f32, w as f32, h as f32);
        skia_canvas.draw_rect(rect, &skia_paint);

        if let Some(_) = affine {
            skia_canvas.restore();
        }
    }

    pub fn draw_circle(
        skia_canvas: &skia_safe::Canvas,
        skia_paint: skia_safe::Paint,
        center: Point,
        radius: f32,
        affine: Option<Affine>,
    ) {
        if let Some(affine) = affine {
            skia_canvas.save();
            skia_canvas.concat(&affine.into());
        }

        skia_canvas.draw_circle(center, radius as f32, &skia_paint);

        if let Some(_) = affine {
            skia_canvas.restore();
        }
    }

    pub fn draw_line(
        skia_canvas: &skia_safe::Canvas,
        skia_paint: skia_safe::Paint,
        start: Point,
        end: Point,
        affine: Option<Affine>,
    ) {
        if let Some(affine) = affine {
            skia_canvas.save();
            skia_canvas.concat(&affine.into());
        }

        skia_canvas.draw_line(start, end, &skia_paint);

        if let Some(_) = affine {
            skia_canvas.restore();
        }
    }

    pub fn draw_ellipse(
        skia_canvas: &skia_safe::Canvas,
        skia_paint: skia_safe::Paint,
        center: Point,
        radius_x: f32,
        radius_y: f32,
        rotation: f32,
        affine: Option<Affine>,
    ) {
        if let Some(affine) = affine {
            skia_canvas.save();
            skia_canvas.concat(&affine.into());
        }

        let width = radius_x as f32;
        let height = radius_y as f32;

        let bounds = skia_safe::Rect::from_xywh(
            center.x as f32 - width,
            center.y as f32 - height,
            width * 2.0,
            height * 2.0,
        );

        skia_canvas.save();
        skia_canvas.rotate(rotation as f32, Some(center.into()));
        skia_canvas.draw_oval(bounds, &skia_paint);
        skia_canvas.restore();

        if let Some(_) = affine {
            skia_canvas.restore();
        }
    }

    pub fn draw_rounded_rectangle(
        skia_canvas: &skia_safe::Canvas,
        skia_paint: skia_safe::Paint,
        a: Point,
        b: Point,
        radius: f32,
        affine: Option<Affine>,
    ) {
        if let Some(affine) = affine {
            skia_canvas.save();
            skia_canvas.concat(&affine.into());
        }

        let rect = skia_safe::Rect::from_xywh(a.x as f32, a.y as f32, b.x as f32, b.y as f32);
        skia_canvas.draw_round_rect(rect, radius as f32, radius as f32, &skia_paint);

        if let Some(_) = affine {
            skia_canvas.restore();
        }
    }

    pub fn draw_polygon(
        skia_canvas: &skia_safe::Canvas,
        skia_paint: skia_safe::Paint,
        points: Vec<Point>,
        affine: Option<Affine>,
    ) {
        if let Some(affine) = affine {
            skia_canvas.save();
            skia_canvas.concat(&affine.into());
        }

        let mut path = skia_safe::path::Path::polygon(
            points
                .iter()
                .map(|point| (*point).into())
                .collect::<Vec<skia_safe::Point>>()
                .as_slice(),
            true,
            None,
            false,
        );

        skia_canvas.draw_path(&path, &skia_paint);

        if let Some(_) = affine {
            skia_canvas.restore();
        }
    }

    pub fn draw_triangle(
        skia_canvas: &skia_safe::Canvas,
        skia_paint: skia_safe::Paint,
        a: Point,
        b: Point,
        c: Point,
        affine: Option<Affine>,
    ) {
        if let Some(affine) = affine {
            skia_canvas.save();
            skia_canvas.concat(&affine.into());
        }

        let mut path = skia_safe::path::Path::polygon([a.into(), b.into(), c.into()].as_slice(), true, None, false);
        skia_canvas.draw_path(&path, &skia_paint);

        if let Some(_) = affine {
            skia_canvas.restore();
        }
    }

    pub fn draw_path(
        skia_canvas: &skia_safe::Canvas,
        skia_paint: skia_safe::Paint,
        points: Vec<Point>,
        affine: Option<Affine>,
    ) {
        if let Some(affine) = affine {
            skia_canvas.save();
            skia_canvas.concat(&affine.into());
        }

        let mut path = skia_safe::path::Path::polygon(
            points
                .iter()
                .map(|point| (*point).into())
                .collect::<Vec<skia_safe::Point>>()
                .as_slice(),
            false,
            None,
            false,
        );
        skia_canvas.draw_path(&path, &skia_paint);

        if let Some(_) = affine {
            skia_canvas.restore();
        }
    }

    // pub fn clip_shape(
    //     skia_canvas: &skia_safe::Canvas,
    //     skia_paint: skia_safe::Paint,
    //     shape: Shape,
    //     affine: Option<Affine>,
    // ) {
    //     // apply the affine transformation
    //     if let Some(affine) = affine {
    //         skia_canvas.save();
    //         skia_canvas.concat(&affine.into());
    //     }

    //     match shape {
    //         Shape::Rectangle { a, w, h } => {
    //             let rect = skia_safe::Rect::from_xywh(a.x as f32, a.y as f32, w as f32, h as f32);
    //             skia_canvas.clip_rect(rect, skia_safe::ClipOp::Intersect, true);
    //         }
    //         Shape::Circle { center, radius } => {
    //             let circle = skia_safe::path::Path::circle(center, radius as f32, None);
    //             skia_canvas.clip_path(&circle, skia_safe::ClipOp::Intersect, true);
    //         }
    //         _ => {
    //             todo!()
    //         }
    //     }

    //     // restore the canvas
    //     if let Some(_) = affine {
    //         skia_canvas.restore();
    //     }
    // }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn start_layer(
        &mut self,
        composite_mode: BlendMode,
        clip_transform: Option<Affine>,
        layer_transform: Option<Affine>,
        alpha: f32,
    ) {
        let mut binding = self.picture_recorder.lock().unwrap();
        let mut canvas = binding.recording_canvas().unwrap();
        // let mut layer_paint = skia_safe::Paint::default();
        // layer_paint.set_alpha_f(alpha);
        // // layer_paint.set_blend_mode(composite_mode.into());
        // let save_layer_rec = skia_safe::canvas::SaveLayerRec::default();
        // let save_layer_rec = save_layer_rec.paint(&layer_paint);

        canvas.save_layer_alpha_f(None, alpha);
        // Self::clip_shape(&mut canvas, skia_safe::Paint::default(), clip, clip_transform);

        // update the current blend mode
        // self.current_blend_mode = composite_mode.into();
    }

    pub fn end_layer(&mut self) {
        let mut binding = self.picture_recorder.lock().unwrap();
        binding.recording_canvas().unwrap().restore();
    }

    pub fn draw_text(canvas: &mut skia_safe::Canvas, text: &mut super::text::Text, x: f32, y: f32) {
        text.draw(canvas, x, y);
    }

    pub fn draw_glyphs(
        &mut self,
        position: Point,
        glyphs: &[Glyph],
        font_face: &Typeface,
        font_size: f32,
        brush: Brush,
        alpha: Option<f32>,
        transform: Option<Affine>,
        blend_mode: Option<BlendMode>,
    ) {
        // cast the font face to a skia font face

        // create a new skia font
        let skia_font = SkFont::from_typeface(font_face.typeface.clone(), font_size);

        // create a new paint
        let mut paint: skia_safe::Paint = brush.into();

        // set the alpha if it's not none
        if let Some(alpha) = alpha {
            paint.set_alpha_f(alpha);
        }

        // the origin of the text
        let origin: skia_safe::Point = position.into();

        // draw the glyphs
        let mut binding = self.picture_recorder.lock().unwrap();
        let canvas = binding.recording_canvas().unwrap();
        let glyph_ids = glyphs.iter().map(|glyph| glyph.id).collect::<Vec<u16>>();
        let glyph_positions: Vec<skia_safe::Point> = glyphs.into_iter().map(|glyph| glyph.position.into()).collect();
        let glyph_positions = skia_safe::canvas::GlyphPositions::Points(&glyph_positions);
        // let glyph_cluster_size: Vec<u32> = glyphs.into_iter().map(|glyph| glyph.end - glyph.start).collect();
        // canvas.draw_glyphs_at(&glyph_ids, glyph_positions, origin, &skia_font, &paint);
        canvas.draw_glyphs_at(&glyph_ids, glyph_positions, origin, &skia_font, &paint);
    }

    pub fn set_bg_color(&mut self, color: RGBA) {
        self.bg_color = color;
        let bg_color: skia_safe::Color4f = color.into();
        let mut binding = self.picture_recorder.lock().unwrap();
        binding.recording_canvas().unwrap().clear(bg_color);
    }

    pub fn draw_svg(&mut self, svg: &SVG, position: Point, width: f32, height: f32, blend_mode: Option<BlendMode>) {
        // get the dom
        let dom = &svg.dom;
        let mut root = dom.root();
        let mut binding = self.picture_recorder.lock().unwrap();
        let canvas = binding.recording_canvas().unwrap();
        canvas.save();
        canvas.translate((position.x as scalar, position.y as scalar));

        root.set_width(skia_safe::svg::Length::new(
            width.into(),
            skia_safe::svg::LengthUnit::PX,
        ));
        root.set_height(skia_safe::svg::Length::new(
            height.into(),
            skia_safe::svg::LengthUnit::PX,
        ));

        dom.render(canvas);

        canvas.restore();
    }

    pub fn draw_lottie_animation(
        skia_canvas: &skia_safe::Canvas,
        animation: &mut super::lottie::LottieAnimation,
        rect: Option<(f32, f32, f32, f32)>,
    ) {
        animation.update();

        let anim = animation.animation();

        if let Some((x, y, width, height)) = rect {
            let dest = skia_safe::Rect::from_xywh(x as f32, y as f32, width as f32, height as f32);
            anim.render(skia_canvas, Some(dest));
        } else {
            anim.render(skia_canvas, None);
        }
    }

    pub fn bg_color(&self) -> RGBA {
        self.bg_color
    }
}

impl Renderer {
    pub fn new(
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        internal_color_encoding: ColorEncoding,
        internal_color_format: ColorFormat,
    ) -> Self {
        let backend_context = create_backend_context(adapter, device, queue);
        let skia_context = create_context(&backend_context);

        // create a font manager
        let font_manager = skia_safe::FontMgr::new();

        // create font collection
        let mut font_collection = skia_safe::textlayout::FontCollection::new();
        font_collection.set_default_font_manager(font_manager.clone(), None);

        Self {
            context: RefCell::new(skia_context),
            backend: Arc::new(RefCell::new(backend_context)),
            font_manager,
            font_collection: Arc::new(Mutex::new(font_collection)),
            internal_color_encoding: internal_color_encoding,
            internal_color_format: internal_color_format,
        }
    }

    pub fn render_to_texture(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        width: u32,
        height: u32,
        scene: &mut Scene,
    ) {
        let mut skia_context = self.context.try_borrow_mut().expect("Failed to borrow skia context");

        // create a new surface
        #[cfg(target_os = "windows")]
        let mut surface = Self::create_surface_dx12(
            device,
            width,
            height,
            texture,
            self.internal_color_encoding,
            self.internal_color_format,
            &self.backend.borrow(),
            &mut skia_context,
        );

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        let mut recorder = skia_context
            .make_recorder(Some(&graphite::RecorderOptions::default()))
            .expect("Failed to create recorder");

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        let mut surface = Self::create_surface_metal(
            device,
            width,
            height,
            texture,
            self.internal_color_encoding,
            self.internal_color_format,
            &self.backend.borrow(),
            &mut recorder,
        );

        let canvas = surface.canvas();

        // move origin to the center
        canvas.translate((width as scalar / 2.0, height as scalar / 2.0));
        let mut binding = scene.picture_recorder.lock().unwrap();
        let picture = binding.finish_recording_as_picture(None).unwrap();

        // draw the picture to the canvas
        canvas.draw_picture(&picture, None, None);

        #[cfg(target_os = "windows")]
        skia_context.flush_and_submit();

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            let recording = recorder.snap().expect("Failed to snap recording");

            let insert_info = graphite::InsertRecordingInfo::new(&recording);
            skia_context.insert_recording(&insert_info);
            skia_context.submit(None);
        }
    }

    pub fn create_font_face_from_data(&self, face_info: &FaceInfo, font_data: &[u8], index: usize) -> Option<Typeface> {
        self.font_manager
            .new_from_data(font_data, index)
            .map(|tf| Typeface { typeface: tf })
    }

    pub fn create_bitmap_from_image_u8(
        &self,
        rgba: image::RgbaImage,
        color_encoding: ColorEncoding,
    ) -> Result<Bitmap, String> {
        Ok(skia_create_bitmap_u8(rgba, color_encoding))
    }

    pub fn create_bitmap_from_image_f32(
        &self,
        rgba: image::ImageBuffer<image::Rgba<f32>, Vec<f32>>,
        color_encoding: ColorEncoding,
    ) -> Result<Bitmap, String> {
        Ok(skia_create_bitmap_f32(rgba, color_encoding))
    }

    pub fn create_texture_from_wgpu_texture(
        &self,
        texture: wgpu::Texture,
        color_encoding: ColorEncoding,
    ) -> Result<Bitmap, String> {
        Ok(create_texture_from_wgpu_texture(
            &mut self.context.borrow_mut(),
            texture,
            color_encoding,
        ))
    }

    pub fn create_svg_from_str(&self, svg_data: &str) -> Result<SVG, String> {
        let dom = Dom::from_str(svg_data, self.font_manager.clone())
            .map_err(|e| format!("Failed to parse SVG data: {:?}", e))?;
        Ok(SVG { dom })
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn create_surface_metal(
        _device: &wgpu::Device,
        width: u32,
        height: u32,
        texture: &wgpu::Texture,
        color_encoding: ColorEncoding,
        color_format: ColorFormat,
        _backend: &mtl::BackendContext,
        mut recorder: &mut graphite::Recorder,
    ) -> skia_safe::Surface {
        let raw_texture_ptr =
            unsafe { texture.as_hal::<wgpu::hal::api::Metal>().unwrap().raw_handle().as_ptr() as mtl::Handle };

        // let texture_info = unsafe { mtl::TextureInfo::new(raw_texture_ptr) };

        // let backend_render_target = skia_safe::gpu::backend_render_targets::make_mtl(
        //     (texture.width() as i32, texture.height() as i32),
        //     &texture_info,
        // );
        //

        // Create backend texture from Metal drawable
        let backend_texture =
            unsafe { mtl::make_backend_texture((width as i32, height as i32), raw_texture_ptr as mtl::Handle) };

        // unsafe {
        //     gpu::surfaces::wrap_backend_render_target(
        //         &mut *context,
        //         &backend_render_target,
        //         SurfaceOrigin::TopLeft,
        //         color_format.into(),
        //         Some(color_encoding.into()),
        //         None,
        //     )
        //     .unwrap()
        // }
        //

        let mut surface = graphite::surfaces::wrap_backend_texture(
            &mut recorder,
            &backend_texture,
            color_format.into(),
            Some(color_encoding.into()),
            None,
        )
        .expect("Failed to create surface");

        surface
    }

    #[cfg(target_os = "windows")]
    fn try_create_backend_dx12(
        adapter: &Adapter,
        device: &Device,
        queue: &Queue,
    ) -> Option<(d3d::BackendContext, gpu::DirectContext)> {
        let command_queue = unsafe { queue.as_hal::<wgpu::hal::api::Dx12>().unwrap().as_raw().clone() };

        let raw_adapter = unsafe { adapter.as_hal::<wgpu::hal::api::Dx12>().unwrap().raw_adapter().clone() };

        let raw_device = unsafe { device.as_hal::<wgpu::hal::api::Dx12>().unwrap().raw_device().clone() };

        let backend = unsafe {
            use windows::core::Interface;

            d3d::BackendContext {
                adapter: raw_adapter.cast().unwrap(),
                device: raw_device,
                queue: command_queue.clone(),
                memory_allocator: None,
                protected_context: Protected::No,
            }
        };

        let context = unsafe { gpu::DirectContext::new_d3d(&backend, None).unwrap() };

        Some((backend, context))
    }

    #[cfg(target_os = "windows")]
    fn create_surface_dx12(
        _device: &Device,
        width: u32,
        height: u32,
        texture: &Texture,
        color_encoding: ColorEncoding,
        color_format: ColorFormat,
        _backend: &d3d::BackendContext,
        context: &mut gpu::DirectContext,
    ) -> skia_safe::Surface {
        use windows::Win32::Graphics::Direct3D12::D3D12_RESOURCE_STATE_RENDER_TARGET;

        let raw_texture = unsafe { texture.as_hal::<wgpu::hal::api::Dx12>().unwrap().raw_resource().clone() };

        let backend_render_target = skia_safe::gpu::BackendRenderTarget::new_d3d(
            (width as i32, height as i32),
            &d3d::TextureResourceInfo {
                resource: raw_texture,
                alloc: None,
                resource_state: D3D12_RESOURCE_STATE_RENDER_TARGET,
                format: color_format.into(),
                sample_count: 1,
                level_count: 0,
                sample_quality_pattern: DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
                protected: Protected::No,
            },
        );

        gpu::surfaces::wrap_backend_render_target(
            &mut *context,
            &backend_render_target,
            SurfaceOrigin::TopLeft,
            color_format.into(),
            Some(color_encoding.into()),
            None,
        )
        .expect(&format!(
            "Failed to create Skia surface from DX12 texture with color encoding {:?} and color format {:?}",
            color_encoding, color_format
        ))
    }

    pub fn create_scene(&self, width: u32, heigth: u32) -> Scene {
        Scene::new(width, heigth, self.font_collection.clone())
    }
}

// convert a color to a skia color
impl From<RGBA> for skia_safe::Color4f {
    fn from(color: RGBA) -> Self {
        skia_safe::Color4f::new(color.r, color.g, color.b, color.a)
    }
}

impl From<&RGBA> for skia_safe::Color4f {
    fn from(c: &RGBA) -> Self {
        skia_safe::Color4f::new(c.r, c.g, c.b, c.a)
    }
}

// convert a brush to a skia paint
impl From<&Brush> for skia_safe::Paint {
    fn from(brush: &Brush) -> Self {
        let mut paint = skia_safe::Paint::default();
        match brush {
            Brush::Solid(color) => {
                let skia_color: skia_safe::Color4f = color.into();
                let skia_color_space = skia_safe::ColorSpace::new_srgb_linear();

                // let shader = skia_safe::shaders::color_in_space(skia_color, &skia_color_space);
                // paint.set_shader(shader);
                paint.set_color4f(&skia_color, &skia_color_space);

                paint.set_blend_mode(skia_safe::BlendMode::Src);
                paint
            }
            Brush::Gradient(Gradient { extend, kind, stops }) => {
                let gradient_colors: Vec<skia_safe::Color4f> = stops.iter().map(|stop| stop.color.into()).collect();
                let gradient_colors = SkGradientShaderColors::from(gradient_colors.as_slice());
                let stops: Vec<skia_safe::scalar> = stops.iter().map(|stop| stop.offset).collect();

                // gradients need to be handled through a shader
                let shader = match kind {
                    GradientKind::Linear { start, end } => sk_linear(
                        (*start, *end),
                        gradient_colors,
                        stops.as_slice(),
                        (*extend).into(),
                        None,
                        None,
                    )
                    .unwrap(),
                    GradientKind::Radial { center, radius } => sk_radial(
                        *center,
                        *radius,
                        gradient_colors,
                        stops.as_slice(),
                        (*extend).into(),
                        None,
                        None,
                    )
                    .unwrap(),
                    GradientKind::Sweep {
                        center,
                        start_angle,
                        end_angle,
                    } => sk_sweep(
                        *center,
                        gradient_colors,
                        stops.as_slice(),
                        (*extend).into(),
                        (*start_angle, *end_angle),
                        None,
                        None,
                    )
                    .unwrap(),
                };

                paint.set_shader(shader);
                paint
            }
            Brush::Image {
                image,
                start,
                fit_mode,
                edge_mode,
                sampling,
                transform,
                alpha,
            } => {
                // downcast the image to a skia image
                let skia_image = &image.image;

                let mut local_matrix = match fit_mode {
                    ImageFitMode::Original => Matrix::new_identity(),
                    ImageFitMode::Exact { width, height } => {
                        let scale_x = width / skia_image.width() as f32;
                        let scale_y = height / skia_image.height() as f32;
                        let p: skia_safe::Vector = (*start).into();
                        let mut mat = Matrix::translate((start.x as scalar, start.y as scalar));
                        mat.post_scale((scale_x as scalar, scale_y as scalar), p);
                        mat
                    }
                };

                // multiply the local matrix with the transform matrix
                if let Some(transform) = transform {
                    local_matrix.post_concat(&(*transform).into());
                    // println!("local matrix: {:?}", local_matrix);
                }

                let sampling_options = match sampling {
                    ImageSampling::Nearest => {
                        SamplingOptions::new(skia_safe::FilterMode::Nearest, skia_safe::MipmapMode::None)
                    }
                    ImageSampling::Linear => {
                        SamplingOptions::new(skia_safe::FilterMode::Linear, skia_safe::MipmapMode::None)
                    }
                };

                // create a shader from the image
                let shader = skia_image.to_shader(
                    Some((edge_mode.0.into(), edge_mode.1.into())),
                    sampling_options,
                    &local_matrix,
                );

                // paint.set_color(skia_safe::Color::WHITE);
                paint.set_shader(shader);

                // set the alpha
                if let Some(alpha) = alpha {
                    paint.set_alpha_f(*alpha);
                }

                paint
            }
        }
    }
}

// convert Point to skia point
impl From<Point> for skia_safe::Point {
    fn from(point: Point) -> Self {
        skia_safe::Point::new(point.x as scalar, point.y as scalar)
    }
}

// convert Affine to skia matrix
impl From<Affine> for skia_safe::Matrix {
    fn from(affine: Affine) -> Self {
        let mut sk_matrix = skia_safe::Matrix::default();
        let nalgebra_matrix = affine.as_matrix();
        // skia expects the matrix in column major order
        let scalar_array: [scalar; 6] = [
            nalgebra_matrix[(0, 0)] as scalar,
            nalgebra_matrix[(1, 0)] as scalar,
            nalgebra_matrix[(0, 1)] as scalar,
            nalgebra_matrix[(1, 1)] as scalar,
            nalgebra_matrix[(0, 2)] as scalar,
            nalgebra_matrix[(1, 2)] as scalar,
        ];
        sk_matrix.set_affine(&scalar_array);
        sk_matrix
    }
}

// convert Extend to skia tile mode
impl From<Extend> for skia_safe::TileMode {
    fn from(extend: Extend) -> Self {
        match extend {
            Extend::Pad => skia_safe::TileMode::Clamp,
            Extend::Repeat => skia_safe::TileMode::Repeat,
            Extend::Reflect => skia_safe::TileMode::Mirror,
        }
    }
}

// convert CompositeMode to skia blend mode
impl From<BlendMode> for skia_safe::BlendMode {
    fn from(composite_mode: BlendMode) -> Self {
        match composite_mode {
            BlendMode::SourceAtop => skia_safe::BlendMode::SrcATop,
            BlendMode::SourceIn => skia_safe::BlendMode::SrcIn,
            BlendMode::SourceOut => skia_safe::BlendMode::SrcOut,
            BlendMode::SourceOver => skia_safe::BlendMode::SrcOver,
            BlendMode::DestinationAtop => skia_safe::BlendMode::DstATop,
            BlendMode::DestinationIn => skia_safe::BlendMode::DstIn,
            BlendMode::DestinationOut => skia_safe::BlendMode::DstOut,
            BlendMode::DestinationOver => skia_safe::BlendMode::DstOver,
            BlendMode::Lighter => skia_safe::BlendMode::Lighten,
            BlendMode::Copy => skia_safe::BlendMode::Src,
            BlendMode::Xor => skia_safe::BlendMode::Xor,
            BlendMode::Multiply => skia_safe::BlendMode::Multiply,
            BlendMode::Modulate => skia_safe::BlendMode::Modulate,
        }
    }
}

// convert Shape to Path
// impl From<&Shape> for skia_safe::Path {
//     fn from(shape: &Shape) -> Self {
//         let mut path = skia_safe::Path::new();
//         match shape {
//             Shape::Rectangle { a, w, h } => {
//                 path.add_rect(
//                     skia_safe::Rect::from_xywh(a.x as scalar, a.y as scalar, *w as scalar, *h as scalar),
//                     None,
//                 );
//             }
//             Shape::Circle { center, radius } => {
//                 path.add_circle(*center, *radius as scalar, None);
//             }
//             Shape::Line { start, end } => {
//                 path.move_to(*start);
//                 path.line_to(*end);
//             }
//             Shape::Ellipse {
//                 center,
//                 radius_x,
//                 radius_y,
//                 rotation,
//             } => {
//                 path.add_oval(
//                     skia_safe::Rect::from_xywh(
//                         center.x as scalar - *radius_x as scalar,
//                         center.y as scalar - *radius_y as scalar,
//                         *radius_x as scalar * 2.0,
//                         *radius_y as scalar * 2.0,
//                     ),
//                     None,
//                 );
//             }
//             Shape::RoundedRectangle { a, b, radius } => {
//                 path.add_round_rect(
//                     skia_safe::Rect::from_xywh(a.x as scalar, a.y as scalar, b.x as scalar, b.y as scalar),
//                     (*radius as scalar, *radius as scalar),
//                     None,
//                 );
//             }
//             Shape::Polygon { points } => {
//                 if points.len() == 0 {
//                     return path;
//                 }
//                 path.move_to(points[0]);
//                 for point in points.iter().skip(1) {
//                     path.line_to(*point);
//                 }
//                 path.close();
//             }
//             Shape::Triangle { a, b, c } => {
//                 path.move_to(*a);
//                 path.line_to(*b);
//                 path.line_to(*c);
//                 path.close();
//             }
//             Shape::Path { points } => {
//                 if points.len() == 0 {
//                     return path;
//                 }
//                 path.move_to(points[0]);
//                 for point in points.iter().skip(1) {
//                     path.line_to(*point);
//                 }
//             }
//         }
//         path
//     }
// }

// impl From<Shape> for skia_safe::Path {
//     fn from(value: Shape) -> Self {
//         (&value).into()
//     }
// }

impl From<Brush> for skia_safe::Paint {
    fn from(value: Brush) -> Self {
        (&value).into()
    }
}

fn skia_create_bitmap_u8(rgba: image::RgbaImage, color_encoding: ColorEncoding) -> Bitmap {
    let (width, height) = rgba.dimensions();
    let buffer = rgba.into_raw();
    let boxed_buffer = buffer.into_boxed_slice();

    // create a new skia image
    let image = sk_raster_from_data(
        &skia_safe::ImageInfo::new(
            (width as i32, height as i32),
            ColorType::RGBA8888,
            SkAlphaType::Unpremul,
            Some(color_encoding.into()),
        ),
        &unsafe { skia_safe::Data::new_bytes(&boxed_buffer) },
        width as usize * 4,
    )
    .unwrap();

    Bitmap { image }
}

fn skia_create_bitmap_f32(
    rgba: image::ImageBuffer<image::Rgba<f32>, Vec<f32>>,
    color_encoding: ColorEncoding,
) -> Bitmap {
    let (width, height) = rgba.dimensions();
    let buffer = rgba.into_raw();
    // convert the buffer to bytes using bytemuck
    let buffer = bytemuck::cast_slice::<f32, u8>(&buffer).to_vec();

    let boxed_buffer = buffer.into_boxed_slice();

    // create a new skia image
    let image = sk_raster_from_data(
        &skia_safe::ImageInfo::new(
            (width as i32, height as i32),
            ColorType::RGBAF32,
            SkAlphaType::Unpremul,
            Some(color_encoding.into()),
        ),
        &unsafe { skia_safe::Data::new_bytes(&boxed_buffer) },
        width as usize * 4 * 4,
    )
    .expect("Failed to create skia image for f32 bitmap");

    Bitmap { image }
}

// allow a colorpace to be converted to a skia color space
impl From<ColorEncoding> for skia_safe::ColorSpace {
    fn from(value: ColorEncoding) -> Self {
        match value {
            ColorEncoding::Srgb => skia_safe::ColorSpace::new_srgb(),
            ColorEncoding::Linear => skia_safe::ColorSpace::new_srgb_linear(),
        }
    }
}

impl From<ColorFormat> for skia_safe::ColorType {
    fn from(value: ColorFormat) -> Self {
        match value {
            ColorFormat::Rgba8 => skia_safe::ColorType::RGBA8888,
            ColorFormat::RgbaF16 => skia_safe::ColorType::RGBAF16,
            ColorFormat::Rgba10 => skia_safe::ColorType::RGBA1010102,
            _ => panic!("Unsupported color format for Skia renderer"),
        }
    }
}

#[cfg(target_os = "windows")]
impl From<ColorFormat> for DXGI_FORMAT {
    fn from(value: ColorFormat) -> Self {
        match value {
            ColorFormat::Rgba8 => windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM,
            ColorFormat::RgbaF16 => windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R16G16B16A16_FLOAT,
            ColorFormat::Rgba10 => windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R10G10B10A2_UNORM,
            _ => panic!("Unsupported color format for Skia renderer"),
        }
    }
}

// Helper functions

/// Create a Skia backend texture from a WGPU texture. Currently only supports Windows with Direct3D 12 and Metal on macOS/iOS.
fn create_backend_texture(texture: &wgpu::Texture) -> skia_safe::gpu::BackendTexture {
    // windows/dx12 implementation
    #[cfg(target_os = "windows")]
    {
        let raw_texture_ptr = unsafe { texture.as_hal::<wgpu::hal::api::Dx12>().unwrap().raw_resource().clone() };

        let backend_texture = skia_safe::gpu::BackendTexture::new_d3d(
            (texture.width() as i32, texture.height() as i32),
            &d3d::TextureResourceInfo {
                resource: raw_texture_ptr,
                alloc: None,
                resource_state: windows::Win32::Graphics::Direct3D12::D3D12_RESOURCE_STATE_COMMON,
                format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM,
                sample_count: 1,
                level_count: 0,
                sample_quality_pattern:
                    windows::Win32::Graphics::Dxgi::Common::DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
                protected: Protected::No,
            },
        );

        backend_texture
    }
    // macos/metal implementation
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        // let raw_texture_ptr =
        //     unsafe { texture.as_hal::<wgpu::hal::api::Metal>().unwrap().raw_handle().as_ptr() as mtl::Handle };

        // let texture_info = unsafe { mtl::TextureInfo::new(raw_texture_ptr) };

        // log::debug!(
        //     "Creating Skia backend texture for Metal with size: {}x{}",
        //     texture.width(),
        //     texture.height()
        // );

        // unsafe {
        //     skia_safe::gpu::backend_textures::make_mtl(
        //         (texture.width() as i32, texture.height() as i32),
        //         skia_safe::gpu::Mipmapped::No,
        //         &texture_info,
        //         "default",
        //     )
        // }
        todo!("Skia backend texture creation for Metal is not yet implemented")
    }
    // other platforms can be added here
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
    {
        panic!("Skia backend texture creation is not supported on this platform");
    }
}

fn create_texture_from_wgpu_texture(
    context: &mut graphite::Context,
    texture: wgpu::Texture,
    color_encoding: ColorEncoding,
) -> Bitmap {
    todo!()
    // // create a skia backend context

    // // create a backend texture from the wgpu texture
    // let backend_texture = create_backend_texture(&texture);

    // // create a skia image from the backend texture (using adopt_backend_texture)
    // let skia_image = skia_safe::gpu::images::borrow_texture_from(
    //     context,
    //     &backend_texture,
    //     SurfaceOrigin::TopLeft,
    //     ColorType::RGBA8888,
    //     SkAlphaType::Unpremul,
    //     Some(color_encoding.into()),
    // )
    // .expect("Failed to create Skia image from WGPU texture");

    // println!("Skia image: {:?}", skia_image);

    // context.flush_and_submit();
    // context.reset(None);

    // // create a bitmap from the skia image
    // Bitmap {
    //     image: skia_image,
    //     data: BitmapData::Texture(texture),
    // }
}

#[allow(rustc::unused_variables)]
fn create_backend_context(adapter: &wgpu::Adapter, device: &wgpu::Device, queue: &wgpu::Queue) -> BackendContext {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        let command_queue_ptr = unsafe {
            queue
                .as_hal::<wgpu::hal::api::Metal>()
                .unwrap()
                .as_raw()
                .lock()
                .as_ptr()
        };

        let raw_device_ptr = unsafe {
            device
                .as_hal::<wgpu::hal::api::Metal>()
                .unwrap()
                .raw_device()
                .lock()
                .as_ptr() as mtl::Handle
        };

        // let backend = unsafe { mtl::BackendContext::new(raw_device_ptr, command_queue_ptr as mtl::Handle) };

        let backend = unsafe { BackendContext::new(raw_device_ptr as mtl::Handle, command_queue_ptr as mtl::Handle) };

        backend
    }
    #[cfg(target_os = "windows")]
    {
        let command_queue = unsafe { queue.as_hal::<wgpu::hal::api::Dx12>().unwrap().as_raw().clone() };

        let raw_adapter = unsafe {
            let a = adapter.as_hal::<wgpu::hal::api::Dx12>().unwrap();
            a.raw_adapter().clone()
        };

        let raw_device = unsafe { device.as_hal::<wgpu::hal::api::Dx12>().unwrap().raw_device().clone() };

        d3d::BackendContext {
            adapter: raw_adapter.cast().expect("Failed to cast raw adapter"),
            device: raw_device,
            queue: command_queue.clone(),
            memory_allocator: None,
            protected_context: Protected::No,
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn create_context(backend: &BackendContext) -> graphite::Context {
    let context_options = graphite::ContextOptions::default();
    mtl::make_context(&backend, Some(&context_options)).expect("Failed to create Graphite context")
}

#[cfg(target_os = "windows")]
fn create_context(backend: &BackendContext) -> gpu::DirectContext {
    unsafe { gpu::DirectContext::new_d3d(backend, None).expect("Failed to create Skia DirectContext") }
}
