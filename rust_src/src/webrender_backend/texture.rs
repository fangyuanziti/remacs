use std::{cell::RefCell, collections::HashMap, rc::Rc};

use gleam::gl::{self, Gl};
use webrender::{self, api::units::*, api::*};

type TextureTable = HashMap<gl::GLuint, (FramebufferIntSize, bool)>;

pub struct TextureResourceManager {
    textures: Rc<RefCell<TextureTable>>,
}

impl TextureResourceManager {
    pub fn new() -> TextureResourceManager {
        Self {
            textures: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn new_texture(
        &mut self,
        gl: Rc<dyn Gl>,
        size: FramebufferIntSize,
        need_flip: bool,
    ) -> gl::GLuint {
        let texture_id = gl.gen_textures(1)[0];

        gl.bind_texture(gl::TEXTURE_2D, texture_id);

        gl.tex_parameter_i(
            gl::TEXTURE_2D,
            gl::TEXTURE_MAG_FILTER,
            gl::LINEAR as gl::GLint,
        );

        gl.tex_parameter_i(
            gl::TEXTURE_2D,
            gl::TEXTURE_MIN_FILTER,
            gl::LINEAR as gl::GLint,
        );

        gl.tex_parameter_i(
            gl::TEXTURE_2D,
            gl::TEXTURE_WRAP_S,
            gl::CLAMP_TO_EDGE as gl::GLint,
        );

        gl.tex_parameter_i(
            gl::TEXTURE_2D,
            gl::TEXTURE_WRAP_T,
            gl::CLAMP_TO_EDGE as gl::GLint,
        );

        gl.tex_image_2d(
            gl::TEXTURE_2D,
            0,
            gl::RGBA as gl::GLint,
            size.width,
            size.height,
            0,
            gl::BGRA,
            gl::UNSIGNED_BYTE,
            None,
        );

        self.insert(texture_id, size, need_flip);

        gl.bind_texture(gl::TEXTURE_2D, 0);

        texture_id
    }

    pub fn insert(&mut self, texture_id: gl::GLuint, size: FramebufferIntSize, need_flip: bool) {
        self.textures
            .borrow_mut()
            .insert(texture_id, (size, need_flip));
    }

    pub fn new_external_image_handler(&self) -> Box<dyn ExternalImageHandler> {
        let handler = ExternalHandler {
            texture: self.textures.clone(),
        };

        Box::new(handler)
    }
}

struct ExternalHandler {
    texture: Rc<RefCell<TextureTable>>,
}

impl ExternalImageHandler for ExternalHandler {
    fn lock(
        &mut self,
        key: ExternalImageId,
        _channel_index: u8,
        _rendering: ImageRendering,
    ) -> ExternalImage {
        let texture_id = key.0;

        let textures = self.texture.borrow();
        let (size, need_filp) = textures.get(&(texture_id as u32)).unwrap();

        let uv = if *need_filp {
            TexelRect::new(0.0, size.height as f32, size.width as f32, 0.0)
        } else {
            TexelRect::new(0.0, 0.0, size.width as f32, size.height as f32)
        };

        ExternalImage {
            uv,
            source: ExternalImageSource::NativeTexture(texture_id as u32),
        }
    }
    fn unlock(&mut self, _key: ExternalImageId, _channel_index: u8) {}
}
