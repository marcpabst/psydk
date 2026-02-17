use std::sync::{Arc, Mutex};

use skia_safe::{
    textlayout::{FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, TextStyle, TypefaceFontProvider},
    Canvas, FontMgr, Paint, Point, Typeface,
};

#[derive(Debug)]
pub struct Text {
    pub text: String,
    pub skia_style: TextStyle,
    pub skia_paragraph: Option<Paragraph>,
    pub layout_width: Option<f32>,
}

impl Text {
    pub fn new(text: &str, font_family: Option<String>, font_size: Option<f32>, layout_width: Option<f32>) -> Self {
        let mut text_style = TextStyle::default();

        // set default font size to 16, we can change this later
        text_style.set_font_size(font_size.unwrap_or(16.0));

        if let Some(font_family) = &font_family {
            text_style.set_font_families(&[font_family]);
        }

        Self {
            text: text.to_string(),
            skia_style: text_style,
            skia_paragraph: None,
            layout_width,
        }
    }

    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.skia_style.set_font_size(font_size);
    }

    pub fn set_typeface(&mut self, typeface: Typeface) {
        self.skia_style.set_typeface(typeface.clone());
    }

    pub fn set_paint(&mut self, paint: &Paint) {
        let mut binding = paint.clone();
        let paint = binding.set_anti_alias(true);
        self.skia_style.set_foreground_paint(&paint);
    }

    pub fn build(&mut self, font_collection: &FontCollection) {
        let mut paragraph_style = ParagraphStyle::default();
        // paragraph_style.set_text_align(skia_safe::textlayout::TextAlign::Left);
        // paragraph_style.set_max_lines(1);
        //

        let mut paragraph_builder = ParagraphBuilder::new(&paragraph_style, font_collection.clone());

        paragraph_builder.push_style(&self.skia_style);
        paragraph_builder.add_text(&self.text);
        let mut skia_paragraph = paragraph_builder.build();
        skia_paragraph.layout(self.layout_width.unwrap_or(f32::INFINITY));
        self.skia_paragraph = Some(skia_paragraph);
    }

    pub fn measure(&self) -> Option<(f32, f32)> {
        if let Some(paragraph) = &self.skia_paragraph {
            Some((paragraph.max_intrinsic_width(), paragraph.height()))
        } else {
            None
        }
    }

    pub fn draw(&mut self, canvas: &Canvas, x: f32, y: f32) {
        if let Some(paragraph) = &self.skia_paragraph {
            paragraph.paint(canvas, Point::new(x, y));
        }
    }
}
