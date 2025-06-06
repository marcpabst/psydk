use std::{any::Any, cell::RefCell, sync::Arc};

use cosmic_text::fontdb::FaceInfo;
use foreign_types_shared::ForeignType;

#[cfg(target_os = "windows")]
use skia_safe::gpu::{d3d, d3d::BackendContext, Protected};
#[cfg(any(target_os = "macos", target_os = "ios"))]
use skia_safe::gpu::{mtl, mtl::BackendContext};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC, DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
};

use skia_safe::{
    gpu::{self, backend_formats, DirectContext, SurfaceOrigin},
    gradient_shader::{
        linear as sk_linear, radial as sk_radial, sweep as sk_sweep, GradientShaderColors as SkGradientShaderColors,
    },
    image::Image as SkImage,
    images::raster_from_data as sk_raster_from_data,
    scalar, AlphaType as SkAlphaType, ColorSpace, ColorType, Font as SkFont, Matrix, PictureRecorder, SamplingOptions,
    Typeface as SkTypeface,
};
use wgpu::{Adapter, Device, Queue, Texture};

use crate::{
    affine::Affine,
    bitmaps::{Bitmap, DynamicBitmap},
    brushes::{Brush, Extend, Gradient, GradientKind, ImageSampling},
    colors::RGBA,
    font::{DynamicFontFace, Glyph, Typeface},
    renderer::{Renderer, SharedRendererState},
    scenes::Scene,
    shapes::{Point, Shape},
    styles::{BlendMode, ImageFitMode, StrokeStyle},
};

#[derive(Debug)]
pub struct SkiaScene {
    pub picture_recorder: PictureRecorder,
    // pub canvas: skia_safe::Canvas,
    pub width: u32,
    pub height: u32,
    pub bg_color: RGBA,
}

pub struct SkiaRenderer {
    shared_state: SkiaSharedRendererState,
}

#[derive(Debug)]
/// A Bitmap that is backed by a Skia image.
pub struct SkiaBitmap {
    image: SkImage,
    data: Box<[u8]>,
}

#[derive(Debug)]
/// A Bitmap that is backed by a WGPU texture.
pub struct SkiaTexture {
    image: SkImage,
    texture: wgpu::Texture,
}

impl Typeface for SkTypeface {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn cloned(&self) -> Box<dyn Typeface> {
        Box::new(self.clone())
    }
}

impl SkiaScene {
    pub fn new(width: u32, height: u32) -> Self {
        let mut picture_recorder = PictureRecorder::new();
        let bounds = skia_safe::Rect::from_wh(width as f32, height as f32);
        picture_recorder.begin_recording(bounds, None);

        // clear the canvas
        let canvas = picture_recorder.recording_canvas().unwrap();
        canvas.clear(skia_safe::Color4f::new(1.0, 1.0, 1.0, 1.0));

        Self {
            picture_recorder,
            width,
            height,
            bg_color: RGBA::WHITE,
        }
    }

    fn draw_shape(skia_canvas: &skia_safe::Canvas, skia_paint: skia_safe::Paint, shape: Shape, affine: Option<Affine>) {
        // apply the affine transformation
        if let Some(affine) = affine {
            skia_canvas.save();
            skia_canvas.concat(&affine.into());
        }

        match shape {
            Shape::Rectangle { a, w, h } => {
                let rect = skia_safe::Rect::from_xywh(a.x as f32, a.y as f32, w as f32, h as f32);
                skia_canvas.draw_rect(rect, &skia_paint);
            }
            Shape::Circle { center, radius } => {
                skia_canvas.draw_circle(center, radius as f32, &skia_paint);
            }
            Shape::Line { start, end } => {
                skia_canvas.draw_line(start, end, &skia_paint);
            }
            Shape::Ellipse {
                center,
                radius_x,
                radius_y,
                rotation,
            } => {
                // create bounds for the ellipse
                let width = radius_x as f32;
                let height = radius_y as f32;

                let bounds = skia_safe::Rect::from_xywh(
                    center.x as f32 - width,
                    center.y as f32 - height,
                    width * 2.0,
                    height * 2.0,
                );

                // rotate the canvas
                skia_canvas.save();
                skia_canvas.rotate(rotation as f32, Some(center.into()));
                skia_canvas.draw_oval(bounds, &skia_paint);
                skia_canvas.restore();
            }
            Shape::RoundedRectangle { a, b, radius } => {
                let rect = skia_safe::Rect::from_xywh(a.x as f32, a.y as f32, b.x as f32, b.y as f32);
                skia_canvas.draw_round_rect(rect, radius as f32, radius as f32, &skia_paint);
            }
            Shape::Polygon { points } => {
                let mut path = skia_safe::path::Path::new();
                if points.len() == 0 {
                    return;
                }
                path.move_to(points[0]);
                for point in points.iter().skip(1) {
                    path.line_to(*point);
                }
                path.close();
                skia_canvas.draw_path(&path, &skia_paint);
            }
            Shape::Path { points } => {
                let mut path = skia_safe::path::Path::new();
                if points.len() == 0 {
                    return;
                }
                path.move_to(points[0]);
                for point in points.iter().skip(1) {
                    path.line_to(*point);
                }
                skia_canvas.draw_path(&path, &skia_paint);
            }
        }
        // restore the canvas
        if let Some(_) = affine {
            skia_canvas.restore();
        }
    }

    fn clip_shape(skia_canvas: &skia_safe::Canvas, skia_paint: skia_safe::Paint, shape: Shape, affine: Option<Affine>) {
        // apply the affine transformation
        if let Some(affine) = affine {
            skia_canvas.save();
            skia_canvas.concat(&affine.into());
        }

        match shape {
            Shape::Rectangle { a, w, h } => {
                let rect = skia_safe::Rect::from_xywh(a.x as f32, a.y as f32, w as f32, h as f32);
                skia_canvas.clip_rect(rect, skia_safe::ClipOp::Intersect, true);
            }
            Shape::Circle { center, radius } => {
                let circle = skia_safe::path::Path::circle(center, radius as f32, skia_safe::path::Direction::CCW);
                skia_canvas.clip_path(&circle, skia_safe::ClipOp::Intersect, true);
            }
            _ => {
                todo!()
            }
        }

        // restore the canvas
        if let Some(_) = affine {
            skia_canvas.restore();
        }
    }
}

impl Scene for SkiaScene {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn set_width(&mut self, width: u32) {
        todo!()
    }

    fn set_height(&mut self, height: u32) {
        todo!()
    }

    fn background_color(&self) -> RGBA {
        todo!()
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn start_layer(
        &mut self,
        composite_mode: BlendMode,
        clip: Shape,
        clip_transform: Option<Affine>,
        layer_transform: Option<Affine>,
        alpha: f32,
    ) {
        let mut canvas = self.picture_recorder.recording_canvas().unwrap();
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

    fn end_layer(&mut self) {
        self.picture_recorder.recording_canvas().unwrap().restore();
    }

    fn draw_shape_fill(
        &mut self,
        shape: Shape,
        brush: Brush,
        transform: Option<Affine>,
        blend_mode: Option<BlendMode>,
    ) {
        let mut canvas = self.picture_recorder.recording_canvas().unwrap();
        let mut paint: skia_safe::Paint = brush.into();

        paint.set_anti_alias(false);

        if let Some(blend_mode) = blend_mode {
            paint.set_blend_mode(blend_mode.into());
        }

        Self::draw_shape(&mut canvas, paint, shape, transform);
    }

    fn draw_shape_stroke(
        &mut self,
        shape: Shape,
        brush: Brush,
        style: StrokeStyle,
        transform: Option<Affine>,
        blend_mode: Option<BlendMode>,
    ) {
        let mut canvas = self.picture_recorder.recording_canvas().unwrap();
        let mut paint: skia_safe::Paint = brush.into();
        paint.set_stroke(true);
        paint.set_anti_alias(false);

        if let Some(blend_mode) = blend_mode {
            paint.set_blend_mode(blend_mode.into());
        }

        // set the stroke width
        paint.set_stroke_width(style.width as scalar);

        Self::draw_shape(&mut canvas, paint, shape, transform);
    }

    fn draw_glyphs(
        &mut self,
        position: Point,
        glyphs: &[Glyph],
        font_face: &DynamicFontFace,
        font_size: f32,
        brush: Brush,
        alpha: Option<f32>,
        transform: Option<Affine>,
        blend_mode: Option<BlendMode>,
    ) {
        // cast the font face to a skia font face
        let skia_typeface = font_face.try_as::<SkTypeface>().unwrap();

        // create a new skia font
        let skia_font = SkFont::from_typeface(skia_typeface, font_size);

        // create a new paint
        let mut paint: skia_safe::Paint = brush.into();

        // set the alpha if it's not none
        if let Some(alpha) = alpha {
            paint.set_alpha_f(alpha);
        }

        // the origin of the text
        let origin: skia_safe::Point = position.into();

        // draw the glyphs
        let canvas = self.picture_recorder.recording_canvas().unwrap();
        let glyph_ids = glyphs.iter().map(|glyph| glyph.id).collect::<Vec<u16>>();
        let glyph_positions: Vec<skia_safe::Point> = glyphs.into_iter().map(|glyph| glyph.position.into()).collect();
        let glyph_positions = skia_safe::canvas::GlyphPositions::Points(&glyph_positions);
        // let glyph_cluster_size: Vec<u32> = glyphs.into_iter().map(|glyph| glyph.end - glyph.start).collect();
        // canvas.draw_glyphs_at(&glyph_ids, glyph_positions, origin, &skia_font, &paint);
        canvas.draw_glyphs_at(&glyph_ids, glyph_positions, origin, &skia_font, &paint);
    }

    fn set_bg_color(&mut self, color: RGBA) {
        self.bg_color = color;
        let bg_color: skia_safe::Color4f = color.into();
        self.picture_recorder.recording_canvas().unwrap().clear(bg_color);
    }

    fn bg_color(&self) -> RGBA {
        self.bg_color
    }
}

impl Renderer for SkiaRenderer {
    fn render_to_texture(
        &self,
        device: &Device,
        _queue: &Queue,
        texture: &Texture,
        width: u32,
        height: u32,
        scene: &mut dyn Scene,
    ) {
        let mut skia_context = self
            .shared_state
            .context
            .try_borrow_mut()
            .expect("Failed to borrow skia context");

        // create a new surface
        #[cfg(target_os = "windows")]
        let mut surface = Self::create_surface_dx12(
            device,
            width,
            height,
            texture,
            &self.shared_state.backend.borrow(),
            &mut skia_context,
        );

        #[cfg(any(target_os = "macos", target_os = "ios"))]
        let mut surface = Self::create_surface_metal(
            device,
            width,
            height,
            texture,
            &self.shared_state.backend.borrow(),
            &mut skia_context,
        );

        let canvas = surface.canvas();

        // move origin to the center
        canvas.translate((width as scalar / 2.0, height as scalar / 2.0));

        // try to downcast the scene to a SkiaScene
        let skia_scene = scene.as_any_mut().downcast_mut::<SkiaScene>().unwrap();

        let picture = skia_scene.picture_recorder.finish_recording_as_picture(None).unwrap();

        // draw the picture to the canvas
        canvas.draw_picture(&picture, None, None);

        // flush the surface
        skia_context.flush_and_submit();
    }

    fn create_scene(&self, width: u32, heigth: u32) -> Box<dyn Scene> {
        Box::new(SkiaScene::new(width, heigth))
    }

    fn load_font_face(&mut self, face_info: &FaceInfo, font_data: &[u8], index: usize) -> DynamicFontFace {
        // load the font face using skia
        let typeface = self
            .shared_state
            .font_manager
            .new_from_data(font_data, index)
            .expect("Failed to load font face");
        // let typeface = self.font_manager.n
        return DynamicFontFace(Box::new(typeface));
    }

    fn create_bitmap_u8(&self, rgba: image::RgbaImage, color_space: crate::renderer::ColorSpace) -> DynamicBitmap {
        skia_create_bitmap_u8(rgba, color_space)
    }

    fn create_bitmap_f32(
        &self,
        rgba: image::ImageBuffer<image::Rgba<f32>, Vec<f32>>,
        color_space: crate::renderer::ColorSpace,
    ) -> DynamicBitmap {
        skia_create_bitmap_f32(rgba, color_space)
    }

    fn create_bitmap_from_wgpu_texture(
        &self,
        texture: wgpu::Texture,
        color_space: crate::renderer::ColorSpace,
    ) -> DynamicBitmap {
        create_bitmap_from_wgpu_texture(&mut self.shared_state.context.borrow_mut(), texture, color_space)
    }
}

impl SkiaRenderer {
    // #[cfg(any(target_os = "macos", target_os = "ios"))]
    // fn try_create_backend_metal(device: &Device, queue: &Queue) -> Option<(mtl::BackendContext, gpu::DirectContext)> {
    //     let backend = create_backend_context(device, queue);

    //     let context = gpu::DirectContext::new_metal(&backend, None).unwrap();

    //     Some((backend, context))
    // }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn create_surface_metal(
        device: &Device,
        width: u32,
        height: u32,
        texture: &Texture,
        backend: &BackendContext,
        context: &mut gpu::DirectContext,
    ) -> skia_safe::Surface {
        let raw_texture_ptr = unsafe {
            texture
                .as_hal::<wgpu::hal::api::Metal, _, _>(|texture| texture.unwrap().raw_handle().as_ptr() as mtl::Handle)
        };

        let texture_info = unsafe { mtl::TextureInfo::new(raw_texture_ptr) };

        let backend_render_target = skia_safe::gpu::backend_render_targets::make_mtl(
            (texture.width() as i32, texture.height() as i32),
            &texture_info,
        );

        unsafe {
            gpu::surfaces::wrap_backend_render_target(
                &mut *context,
                &backend_render_target,
                SurfaceOrigin::TopLeft,
                ColorType::RGBAF16,
                ColorSpace::new_srgb_linear(),
                None,
            )
            .unwrap()
        }
    }

    #[cfg(target_os = "windows")]
    fn try_create_backend_dx12(
        adapter: &Adapter,
        device: &Device,
        queue: &Queue,
    ) -> Option<(d3d::BackendContext, gpu::DirectContext)> {
        let command_queue =
            unsafe { queue.as_hal::<wgpu::hal::api::Dx12, _, _>(|queue| queue.map(|s| s.as_raw().clone())) };

        if let Some(command_queue) = command_queue {
            let raw_adapter = unsafe {
                adapter.as_hal::<wgpu::hal::api::Dx12, _, _>(|adapter| adapter.map(|s| (**s.raw_adapter()).clone()))
            }
            .unwrap();

            let raw_device =
                unsafe { device.as_hal::<wgpu::hal::api::Dx12, _, _>(|device| device.map(|s| s.raw_device().clone())) }
                    .unwrap();

            let backend = unsafe {
                d3d::BackendContext {
                    adapter: raw_adapter.into(),
                    device: raw_device,
                    queue: command_queue.clone(),
                    memory_allocator: None,
                    protected_context: Protected::No,
                }
            };

            let context = unsafe { gpu::DirectContext::new_d3d(&backend, None).unwrap() };

            Some((backend, context))
        } else {
            None
        }
    }

    #[cfg(target_os = "windows")]
    fn create_surface_dx12(
        _device: &Device,
        width: u32,
        height: u32,
        texture: &Texture,
        _backend: &d3d::BackendContext,
        context: &mut gpu::DirectContext,
    ) -> skia_safe::Surface {
        use windows::Win32::Graphics::{
            Direct3D12::D3D12_RESOURCE_STATE_RENDER_TARGET,
            Dxgi::Common::{DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_R16G16B16A16_UNORM},
        };

        let raw_texture = unsafe {
            texture.as_hal::<wgpu::hal::api::Dx12, _, _>(|texture| texture.map(|s| s.raw_resource().clone()))
        }
        .expect("Failed to get raw texture from WGPU texture");

        let backend_render_target = skia_safe::gpu::BackendRenderTarget::new_d3d(
            (width as i32, height as i32),
            &d3d::TextureResourceInfo {
                resource: raw_texture,
                alloc: None,
                resource_state: D3D12_RESOURCE_STATE_RENDER_TARGET,
                format: DXGI_FORMAT_R16G16B16A16_UNORM,
                sample_count: 1,
                level_count: 0,
                sample_quality_pattern: DXGI_STANDARD_MULTISAMPLE_QUALITY_PATTERN,
                protected: Protected::No,
            },
        );

        println!(
            "Cchecking backend render target validity: {}",
            backend_render_target.is_valid()
        );

        gpu::surfaces::wrap_backend_render_target(
            &mut *context,
            &backend_render_target,
            SurfaceOrigin::TopLeft,
            ColorType::R16G16B16A16UNorm,
            ColorSpace::new_srgb_linear(),
            None,
        )
        .expect("Failed to create Skia surface from DX12 texture")
    }
}

impl Bitmap for SkiaBitmap {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Bitmap for SkiaTexture {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
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
        // let c = color.as_srgba();
        skia_safe::Color4f::new(c.r, c.g, c.b, c.a)
    }
}

// convert a brush to a skia paint
impl From<&Brush<'_>> for skia_safe::Paint {
    fn from(brush: &Brush) -> Self {
        let mut paint = skia_safe::Paint::default();
        match brush {
            Brush::Solid(color) => {
                let skia_color: skia_safe::Color4f = color.into();
                paint.set_color4f(skia_color, &skia_safe::ColorSpace::new_srgb_linear());
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
                let skia_image = &image
                    .try_as::<SkiaBitmap>()
                    .map(|bitmap| &bitmap.image)
                    .or_else(|| image.try_as::<SkiaTexture>().map(|texture| &texture.image))
                    .expect("You're trying to use a non-skia image with a skia renderer");

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
impl From<crate::shapes::Point> for skia_safe::Point {
    fn from(point: crate::shapes::Point) -> Self {
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
impl From<&Shape> for skia_safe::Path {
    fn from(shape: &Shape) -> Self {
        let mut path = skia_safe::Path::new();
        match shape {
            Shape::Rectangle { a, w, h } => {
                path.add_rect(
                    skia_safe::Rect::from_xywh(a.x as scalar, a.y as scalar, *w as scalar, *h as scalar),
                    None,
                );
            }
            Shape::Circle { center, radius } => {
                path.add_circle(*center, *radius as scalar, None);
            }
            Shape::Line { start, end } => {
                path.move_to(*start);
                path.line_to(*end);
            }
            Shape::Ellipse {
                center,
                radius_x,
                radius_y,
                rotation,
            } => {
                path.add_oval(
                    skia_safe::Rect::from_xywh(
                        center.x as scalar - *radius_x as scalar,
                        center.y as scalar - *radius_y as scalar,
                        *radius_x as scalar * 2.0,
                        *radius_y as scalar * 2.0,
                    ),
                    None,
                );
            }
            Shape::RoundedRectangle { a, b, radius } => {
                path.add_round_rect(
                    skia_safe::Rect::from_xywh(a.x as scalar, a.y as scalar, b.x as scalar, b.y as scalar),
                    (*radius as scalar, *radius as scalar),
                    None,
                );
            }
            Shape::Polygon { points } => {
                if points.len() == 0 {
                    return path;
                }
                path.move_to(points[0]);
                for point in points.iter().skip(1) {
                    path.line_to(*point);
                }
                path.close();
            }
            Shape::Path { points } => {
                if points.len() == 0 {
                    return path;
                }
                path.move_to(points[0]);
                for point in points.iter().skip(1) {
                    path.line_to(*point);
                }
            }
        }
        path
    }
}

impl From<Shape> for skia_safe::Path {
    fn from(value: Shape) -> Self {
        (&value).into()
    }
}

impl From<Brush<'_>> for skia_safe::Paint {
    fn from(value: Brush) -> Self {
        (&value).into()
    }
}

#[derive(Clone, Debug)]
pub struct SkiaSharedRendererState {
    context: RefCell<gpu::DirectContext>,
    backend: Arc<RefCell<BackendContext>>,
    font_manager: skia_safe::FontMgr,
}

unsafe impl Send for SkiaSharedRendererState {}
unsafe impl Sync for SkiaSharedRendererState {}

impl SkiaSharedRendererState {
    pub fn new(adapter: &Adapter, device: &Device, queue: &Queue) -> Self {
        let backend_context = create_backend_context(adapter, device, queue);
        let skia_context = create_context(&backend_context);

        // create a font manager
        let font_manager = skia_safe::FontMgr::new();

        Self {
            context: RefCell::new(skia_context),
            backend: Arc::new(RefCell::new(backend_context)),
            font_manager,
        }
    }
}

impl SharedRendererState for SkiaSharedRendererState {
    fn create_renderer(
        &self,
        _surface_format: wgpu::TextureFormat,
        _width: u32,
        _height: u32,
    ) -> crate::DynamicRenderer {
        let renderer = SkiaRenderer {
            shared_state: self.clone(),
        };
        let backend_render = Box::new(renderer) as Box<dyn Renderer>;
        crate::DynamicRenderer::new(backend_render)
    }

    fn cloned(&self) -> Box<dyn SharedRendererState> {
        Box::new(SkiaSharedRendererState {
            context: RefCell::new(self.context.borrow().clone()),
            backend: self.backend.clone(),
            font_manager: self.font_manager.clone(),
        })
    }

    fn create_font_face(&self, font_data: &[u8], index: u32) -> DynamicFontFace {
        let font_manager = skia_safe::FontMgr::new();
        let typeface = font_manager
            .new_from_data(font_data, index as usize)
            .expect("Failed to load font face");
        // let typeface = self.font_manager.n
        return DynamicFontFace(Box::new(typeface));
    }

    fn create_bitmap_u8(&self, data: image::RgbaImage, color_space: crate::renderer::ColorSpace) -> DynamicBitmap {
        skia_create_bitmap_u8(data, color_space)
    }

    fn create_bitmap_f32(
        &self,
        data: image::ImageBuffer<image::Rgba<f32>, Vec<f32>>,
        color_space: crate::renderer::ColorSpace,
    ) -> DynamicBitmap {
        skia_create_bitmap_f32(data, color_space)
    }

    fn create_bitmap_from_wgpu_texture(
        &self,
        texture: wgpu::Texture,
        color_space: crate::renderer::ColorSpace,
    ) -> DynamicBitmap {
        create_bitmap_from_wgpu_texture(&mut self.context.borrow_mut(), texture, color_space)
    }

    fn render_resources(&self) -> Option<crate::renderer::DynamicRenderResources> {
        todo!()
    }

    fn as_any(&self) -> &dyn Any {
        todo!()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        todo!()
    }
}

fn skia_create_bitmap_u8(rgba: image::RgbaImage, color_space: crate::renderer::ColorSpace) -> DynamicBitmap {
    let (width, height) = rgba.dimensions();
    let buffer = rgba.into_raw();
    let boxed_buffer = buffer.into_boxed_slice();

    // create a new skia image
    let image = sk_raster_from_data(
        &skia_safe::ImageInfo::new(
            (width as i32, height as i32),
            ColorType::RGBA8888,
            SkAlphaType::Unpremul,
            Some(color_space.into()),
        ),
        &unsafe { skia_safe::Data::new_bytes(&boxed_buffer) },
        width as usize * 4,
    )
    .unwrap();

    DynamicBitmap(Box::new(SkiaBitmap {
        image,
        data: boxed_buffer,
    }))
}

fn skia_create_bitmap_f32(
    rgba: image::ImageBuffer<image::Rgba<f32>, Vec<f32>>,
    color_space: crate::renderer::ColorSpace,
) -> DynamicBitmap {
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
            Some(color_space.into()),
        ),
        &unsafe { skia_safe::Data::new_bytes(&boxed_buffer) },
        width as usize * 4 * 4,
    )
    .expect("Failed to create skia image for f32 bitmap");

    DynamicBitmap(Box::new(SkiaBitmap {
        image,
        data: boxed_buffer,
    }))
}

// allow a colorpace to be converted to a skia color space
impl From<crate::renderer::ColorSpace> for skia_safe::ColorSpace {
    fn from(value: crate::renderer::ColorSpace) -> Self {
        match value {
            crate::renderer::ColorSpace::Srgb => skia_safe::ColorSpace::new_srgb(),
            crate::renderer::ColorSpace::LinearSrgb => skia_safe::ColorSpace::new_srgb_linear(),
        }
    }
}

// Helper functions

/// Create a Skia backend texture from a WGPU texture. Currently only supports Windows with Direct3D 12 and Metal on macOS/iOS.
fn create_backend_texture(texture: &wgpu::Texture) -> skia_safe::gpu::BackendTexture {
    // windows/dx12 implementation
    #[cfg(target_os = "windows")]
    {
        let raw_texture_ptr = unsafe {
            texture.as_hal::<wgpu::hal::api::Dx12, _, _>(|texture| texture.map(|s| s.raw_resource().clone()))
        }
        .unwrap();

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

        println!(
            "Creating Skia backend texture for D3D with size: {}x{}",
            texture.width(),
            texture.height()
        );

        backend_texture
    }
    // macos/metal implementation
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        let raw_texture_ptr = unsafe {
            texture
                .as_hal::<wgpu::hal::api::Metal, _, _>(|texture| texture.unwrap().raw_handle().as_ptr() as mtl::Handle)
        };

        let texture_info = unsafe { mtl::TextureInfo::new(raw_texture_ptr) };

        println!(
            "Creating Skia backend texture for Metal with size: {}x{}",
            texture.width(),
            texture.height()
        );

        unsafe {
            skia_safe::gpu::backend_textures::make_mtl(
                (texture.width() as i32, texture.height() as i32),
                skia_safe::gpu::Mipmapped::No,
                &texture_info,
                "default",
            )
        }
    }
    // other platforms can be added here
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
    {
        panic!("Skia backend texture creation is not supported on this platform");
    }
}

fn create_bitmap_from_wgpu_texture(
    context: &mut DirectContext,
    texture: wgpu::Texture,
    color_space: crate::renderer::ColorSpace,
) -> DynamicBitmap {
    // create a skia backend context

    // create a backend texture from the wgpu texture
    let backend_texture = create_backend_texture(&texture);

    // create a skia image from the backend texture (using adopt_backend_texture)
    let skia_image = skia_safe::gpu::images::borrow_texture_from(
        context,
        &backend_texture,
        SurfaceOrigin::TopLeft,
        ColorType::RGBA8888,
        SkAlphaType::Unpremul,
        Some(color_space.into()),
    )
    .unwrap();

    println!("Skia image: {:?}", skia_image);

    context.flush_and_submit();
    context.reset(None);

    // create a bitmap from the skia image
    let skia_texture = SkiaTexture {
        image: skia_image,
        texture,
    };

    DynamicBitmap(Box::new(skia_texture))
}

fn create_backend_context(adapter: &Adapter, device: &Device, queue: &Queue) -> BackendContext {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        let command_queue_ptr =
            unsafe { queue.as_hal::<wgpu::hal::api::Metal, _, _>(|queue| queue.map(|s| s.as_raw().lock().as_ptr())) };

        if let Some(command_queue_ptr) = command_queue_ptr {
            let raw_device_ptr = unsafe {
                device.as_hal::<wgpu::hal::api::Metal, _, _>(|device| {
                    device.map(|s| s.raw_device().lock().as_ptr() as mtl::Handle)
                })
            };

            let backend =
                unsafe { mtl::BackendContext::new(raw_device_ptr.unwrap(), command_queue_ptr as mtl::Handle) };

            backend
        } else {
            panic!("Failed to create Skia backend context: command queue pointer is None");
        }
    }
    #[cfg(target_os = "windows")]
    {
        let command_queue =
            unsafe { queue.as_hal::<wgpu::hal::api::Dx12, _, _>(|queue| queue.map(|s| s.as_raw().clone())) };

        if let Some(command_queue) = command_queue {
            let raw_adapter = unsafe {
                adapter.as_hal::<wgpu::hal::api::Dx12, _, _>(|adapter| adapter.map(|s| (**s.raw_adapter()).clone()))
            }
            .unwrap();

            let raw_device =
                unsafe { device.as_hal::<wgpu::hal::api::Dx12, _, _>(|device| device.map(|s| s.raw_device().clone())) }
                    .unwrap();

            d3d::BackendContext {
                adapter: raw_adapter.into(),
                device: raw_device,
                queue: command_queue.clone(),
                memory_allocator: None,
                protected_context: Protected::No,
            }
        } else {
            panic!("Failed to create Skia backend context: command queue is None");
        }
    }
}

fn create_context(backend: &BackendContext) -> gpu::DirectContext {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        gpu::direct_contexts::make_metal(backend, None).expect("Failed to create Skia DirectContext")
    }
    #[cfg(target_os = "windows")]
    {
        unsafe { gpu::DirectContext::new_d3d(backend, None).expect("Failed to create Skia DirectContext") }
    }
}
