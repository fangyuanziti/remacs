use std::mem::ManuallyDrop;
use std::ptr;

use font_kit::{
    family_name::FamilyName,
    loaders::default::Font,
    metrics::Metrics,
    properties::{Style, Weight},
    source::{Source, SystemSource},
};

use webrender::api::units::*;
use webrender::api::*;

use super::{glyph::GlyphStringRef, output::OutputRef};

use crate::{
    fonts::LispFontRef,
    frame::LispFrameRef,
    lisp::{ExternalPtr, LispObject},
    multibyte::LispStringRef,
    remacs_sys::{
        font, font_driver, font_make_entity, font_make_object, font_metrics, font_property_index,
        font_style_to_value, frame, glyph_string, Fassoc, Fcdr, Fcons, Fmake_symbol, Fnreverse,
        Qbold, Qextra_bold, Qextra_light, Qitalic, Qlight, Qnil, Qnormal, Qoblique, Qsemi_bold,
        Qultra_bold, Qwr, FONT_INVALID_CODE,
    },
    symbols::LispSymbolRef,
    util::HandyDandyRectBuilder,
};

fn pixel_to_color(pixel: u64) -> ColorF {
    let pixel_array: [u16; 4] = unsafe { std::mem::transmute(pixel) };

    ColorF::new(
        pixel_array[0] as f32 / 255.0, // red
        pixel_array[1] as f32 / 255.0, // green
        pixel_array[2] as f32 / 255.0, // blue
        1.0,
    )
}

pub type FontRef = ExternalPtr<font>;
impl Default for FontRef {
    fn default() -> Self {
        FontRef::new(ptr::null_mut())
    }
}

type FontDriverRef = ExternalPtr<font_driver>;
unsafe impl Sync for FontDriverRef {}

lazy_static! {
    pub static ref FONT_DRIVER: FontDriverRef = {
        let mut font_driver = Box::new(font_driver::default());

        font_driver.type_ = Qwr;
        font_driver.case_sensitive = true;
        font_driver.get_cache = Some(get_cache);
        font_driver.list = Some(list);
        font_driver.match_ = Some(match_);
        font_driver.list_family = Some(list_family);
        font_driver.open = Some(open);
        font_driver.close = Some(close);
        font_driver.encode_char = Some(encode_char);
        font_driver.text_extents = Some(text_extents);
        font_driver.draw = Some(draw);

        FontDriverRef::new(Box::into_raw(font_driver))
    };
}

/// A newtype for objects we know are font_spec.
#[derive(Clone, Copy)]
pub struct LispFontLike(LispObject);

impl LispFontLike {
    fn aref(&self, index: font_property_index::Type) -> LispObject {
        let vl = self.0.as_vectorlike().unwrap();
        let v = unsafe { vl.as_vector_unchecked() };
        unsafe { v.get_unchecked(index as usize) }
    }

    fn get_family(&self) -> Option<FamilyName> {
        let tem = self.aref(font_property_index::FONT_FAMILY_INDEX);

        if tem.is_nil() {
            None
        } else {
            let symbol_or_string = tem.as_symbol_or_string();
            let string: LispStringRef = symbol_or_string.into();
            match string.to_string().as_ref() {
                "Serif" => Some(FamilyName::Serif),
                "Sans Serif" => Some(FamilyName::SansSerif),
                "Monospace" => Some(FamilyName::Monospace),
                "Cursive" => Some(FamilyName::Cursive),
                "Fantasy" => Some(FamilyName::Fantasy),
                f => Some(FamilyName::Title(f.to_string())),
            }
        }
    }

    fn aset(&self, index: font_property_index::Type, val: LispObject) {
        let vl = self.0.as_vectorlike().unwrap();
        let mut v = unsafe { vl.as_vector_unchecked() };
        unsafe { v.set_unchecked(index as usize, val) };
    }

    fn set_style(&self, index: font_property_index::Type, val: LispObject) {
        let value = unsafe { font_style_to_value(index, val, true) };

        self.aset(index, LispObject::from(value));
    }

    fn as_lisp_object(self) -> LispObject {
        self.0
    }
}

impl From<LispObject> for LispFontLike {
    fn from(v: LispObject) -> LispFontLike {
        LispFontLike(v)
    }
}

extern "C" fn get_cache(f: *mut frame) -> LispObject {
    let frame = LispFrameRef::new(f);
    let output: OutputRef = unsafe { frame.output_data.wr.into() };

    let dpyinfo = output.display_info;

    dpyinfo.name_list_element
}

extern "C" fn draw(
    s: *mut glyph_string,
    from: i32,
    to: i32,
    x: i32,
    y: i32,
    with_background: bool,
) -> i32 {
    let s: GlyphStringRef = s.into();

    let mut output: OutputRef = {
        let frame: LispFrameRef = s.f.into();
        unsafe { frame.output_data.wr.into() }
    };

    let font_key = WRFontRef::from(s.font).font_key;

    let from = from as usize;
    let to = to as usize;

    let pixel_size = unsafe { (*s.font).pixel_size };

    let font_instance_key = output.add_font_instance(font_key, pixel_size);

    output.display(|builder, api, space_and_clip| {
        let glyph_indices: Vec<u32> = s.get_chars()[from..to].iter().map(|c| *c as u32).collect();

        let glyph_dimensions = api.get_glyph_dimensions(font_instance_key, glyph_indices.clone());

        let mut glyph_instances: Vec<GlyphInstance> = vec![];

        for (i, index) in glyph_indices.into_iter().enumerate() {
            let previous_char_width = if i == 0 {
                0.0
            } else {
                let dimension = glyph_dimensions[i - 1];
                match dimension {
                    Some(d) => d.advance,
                    None => 0.0,
                }
            };

            let previous_char_start = if i == 0 {
                x as f32
            } else {
                glyph_instances[i - 1].point.x
            };

            let start = previous_char_start + previous_char_width;

            let glyph_instance = GlyphInstance {
                index,
                point: LayoutPoint::new(start, y as f32),
            };

            glyph_instances.push(glyph_instance);
        }

        let x = s.x;
        let y = s.y;

        let text_bounds = (x, y).by(s.width as i32, s.height as i32);
        let layout = CommonItemProperties::new(text_bounds, space_and_clip);

        let face = s.face;

        // draw background
        if with_background {
            let background_color = pixel_to_color(unsafe { (*face).background });
            builder.push_rect(&layout, background_color);
        }

        // draw foreground text
        let foreground_color = pixel_to_color(unsafe { (*face).foreground });
        builder.push_text(
            &layout,
            layout.clip_rect,
            &glyph_instances,
            font_instance_key,
            foreground_color,
            None,
        );
    });

    return 1;
}

extern "C" fn list(frame: *mut frame, font_spec: LispObject) -> LispObject {
    // FIXME: implment the real list in future
    match_(frame, font_spec)
}

extern "C" fn match_(_f: *mut frame, spec: LispObject) -> LispObject {
    let font_spec = LispFontLike(spec);
    let family = font_spec.get_family();

    let fonts = family
        .and_then(|f| SystemSource::new().select_family_by_generic_name(&f).ok())
        .map(|f| {
            f.fonts()
                .iter()
                .filter_map(|f| f.load().ok())
                .collect::<Vec<Font>>()
        });

    match fonts {
        Some(fonts) => {
            let mut list = Qnil;

            for f in fonts {
                let entity: LispFontLike = unsafe { font_make_entity() }.into();

                // set type
                entity.aset(font_property_index::FONT_TYPE_INDEX, Qwr);

                let family_name: &str = &f.family_name();
                // set family
                entity.aset(font_property_index::FONT_FAMILY_INDEX, unsafe {
                    Fmake_symbol(LispObject::from(family_name))
                });

                let weight = f.properties().weight;

                let weight = if weight <= Weight::EXTRA_LIGHT {
                    Qextra_light
                } else if weight <= Weight::LIGHT {
                    Qlight
                } else if weight <= Weight::NORMAL {
                    Qnormal
                } else if weight <= Weight::MEDIUM {
                    Qsemi_bold
                } else if weight <= Weight::SEMIBOLD {
                    Qsemi_bold
                } else if weight <= Weight::BOLD {
                    Qbold
                } else if weight <= Weight::EXTRA_BOLD {
                    Qextra_bold
                } else if weight <= Weight::BLACK {
                    Qultra_bold
                } else {
                    Qultra_bold
                };

                // set weight
                entity.set_style(font_property_index::FONT_WEIGHT_INDEX, weight);

                let slant = match f.properties().style {
                    Style::Normal => Qnormal,
                    Style::Italic => Qitalic,
                    Style::Oblique => Qoblique,
                };

                // set slant
                entity.set_style(font_property_index::FONT_SLANT_INDEX, slant);

                let postscript_name: &str = &f
                    .postscript_name()
                    .expect("Font must have a postcript_name!");

                // set name
                entity.aset(font_property_index::FONT_EXTRA_INDEX, unsafe {
                    Fcons(
                        Fcons(":postscript-name".into(), LispObject::from(postscript_name)),
                        Qnil,
                    )
                });
                list = unsafe { Fcons(entity.as_lisp_object(), list) }
            }

            unsafe { Fnreverse(list) }
        }
        None => Qnil,
    }
}

#[allow(unused_variables)]
extern "C" fn list_family(f: *mut frame) -> LispObject {
    unimplemented!();
}

#[repr(C)]
pub struct WRFont {
    // extend basic font
    pub font: font,
    // webrender font key
    pub font_key: FontKey,
    // font-kit font
    pub metrics: Metrics,

    pub font_backend: ManuallyDrop<Font>,
}

impl WRFont {
    pub fn glyph_for_char(&self, character: char) -> Option<u32> {
        self.font_backend.glyph_for_char(character)
    }
}

pub type WRFontRef = ExternalPtr<WRFont>;

impl From<*mut font> for WRFontRef {
    fn from(ptr: *mut font) -> Self {
        WRFontRef::new(ptr as *mut WRFont)
    }
}

impl LispFontRef {
    fn as_webrender_font(&mut self) -> WRFontRef {
        WRFontRef::new(self.as_font_mut() as *mut WRFont)
    }
}

extern "C" fn open(frame: *mut frame, font_entity: LispObject, pixel_size: i32) -> LispObject {
    let font_entity: LispFontLike = font_entity.into();

    let frame: LispFrameRef = frame.into();
    let output: OutputRef = unsafe { frame.output_data.wr.into() };

    let mut pixel_size = font_entity
        .aref(font_property_index::FONT_SIZE_INDEX)
        .as_fixnum()
        .unwrap_or(pixel_size as i64);

    if pixel_size == 0 {
        pixel_size = if !output.font.is_null() {
            output.font.pixel_size as i64
        } else {
            9
        };
    }

    let font_object: LispFontLike = unsafe {
        font_make_object(
            vecsize!(WRFont) as i32,
            font_entity.as_lisp_object(),
            pixel_size as i32,
        )
    }
    .into();

    // set type
    font_object.aset(font_property_index::FONT_TYPE_INDEX, Qwr);

    // set name
    font_object.aset(
        font_property_index::FONT_NAME_INDEX,
        LispSymbolRef::from(font_entity.aref(font_property_index::FONT_FAMILY_INDEX)).symbol_name(),
    );

    // Get postscript name form font_entity.
    let font_extra = font_entity.aref(font_property_index::FONT_EXTRA_INDEX);

    let val = unsafe { Fassoc(":postscript-name".into(), font_extra, Qnil) };

    if val.is_nil() {
        error!("postscript-name can not be found!");
    }

    let postscript_name = unsafe { Fcdr(val) }.as_string().unwrap().to_string();

    // load font by postscript_name.
    // The existing of font has been checked in `match` function.
    let font = SystemSource::new()
        .select_by_postscript_name(&postscript_name)
        .unwrap();

    // Create font key in webrender.
    let font_key = output.add_font(&font);

    let mut wr_font = font_object
        .as_lisp_object()
        .as_font()
        .unwrap()
        .as_webrender_font();

    wr_font.font_backend = ManuallyDrop::new(font.load().unwrap());

    let (font_metrics, font_advance) = {
        let font = font.load().unwrap();
        (font.metrics(), font.advance(33).unwrap())
    };

    let scale = pixel_size as f32 / font_metrics.units_per_em as f32;

    wr_font.font.pixel_size = pixel_size as i32;
    wr_font.font.average_width = (font_advance.x * scale) as i32;
    wr_font.font.ascent = (scale * font_metrics.ascent) as i32;
    wr_font.font.descent = (-scale * font_metrics.descent) as i32;
    wr_font.font.space_width = wr_font.font.average_width;

    wr_font.font.height =
        (scale * font_metrics.line_gap) as i32 + wr_font.font.ascent + wr_font.font.descent;

    wr_font.font.driver = FONT_DRIVER.clone().as_mut();

    wr_font.font_key = font_key;

    font_object.as_lisp_object()
}

extern "C" fn close(_font: *mut font) {}

extern "C" fn encode_char(font: *mut font, c: i32) -> u32 {
    let font: WRFontRef = font.into();

    std::char::from_u32(c as u32)
        .and_then(|c| font.glyph_for_char(c))
        .unwrap_or(FONT_INVALID_CODE)
}

#[allow(unused_variables)]
extern "C" fn text_extents(
    font: *mut font,
    code: *mut u32,
    nglyphs: i32,
    metrics: *mut font_metrics,
) {
    let font: WRFontRef = font.into();

    unsafe {
        (*metrics).lbearing = 10;
        (*metrics).rbearing = 10;
        (*metrics).width = font.font.average_width as i16;
        (*metrics).ascent = font.font.ascent as i16;
        (*metrics).descent = font.font.descent as i16;
    }
}
