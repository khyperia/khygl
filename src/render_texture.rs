use crate::{
    check_gl, create_vert_frag_program,
    texture::{Texture, TextureType},
    Rect,
};
use failure::Error;
use gl::{self, types::*};
use std::marker::PhantomData;

// https://rauwendaal.net/2014/06/14/rendering-a-screen-covering-triangle-in-opengl/

pub trait TextureRendererKind {
    fn shader() -> &'static str;
}

pub struct TextureRendererKindU8 {}

impl TextureRendererKind for TextureRendererKindU8 {
    fn shader() -> &'static str {
        FRAGMENT_SHADER_U8
    }
}

pub struct TextureRendererKindF32 {}

impl TextureRendererKind for TextureRendererKindF32 {
    fn shader() -> &'static str {
        FRAGMENT_SHADER_F32
    }
}

pub struct TextureRenderer<T: TextureRendererKind> {
    program: GLuint,
    src_pos_size_location: GLint,
    dst_pos_size_location: GLint,
    _t: PhantomData<T>,
}

impl<T: TextureRendererKind> TextureRenderer<T> {
    pub fn new() -> Result<Self, Error> {
        check_gl()?;
        let frag = T::shader();
        let program = unsafe { create_vert_frag_program(&[VERTEX_SHADER], &[frag])? };
        let src_pos_size_location =
            unsafe { gl::GetUniformLocation(program, b"src_pos_size\0".as_ptr() as *const GLchar) };
        check_gl()?;
        if src_pos_size_location == -1 {
            panic!("src_pos_size_location not found");
        }
        let dst_pos_size_location =
            unsafe { gl::GetUniformLocation(program, b"dst_pos_size\0".as_ptr() as *const GLchar) };
        check_gl()?;
        if dst_pos_size_location == -1 {
            panic!("dst_pos_size_location not found");
        }
        unsafe {
            gl::Enable(gl::BLEND);
            gl::Enable(gl::TEXTURE_2D);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        }
        check_gl()?;
        Ok(Self {
            program,
            src_pos_size_location,
            dst_pos_size_location,
            _t: PhantomData,
        })
    }

    pub fn render<Ty: TextureType>(
        &self,
        texture: &Texture<Ty>,
        src: impl Into<Option<Rect<f32>>>,
        dst: impl Into<Option<Rect<f32>>>,
        screen_size: (f32, f32),
    ) -> Result<(), Error> {
        let src = src
            .into()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, texture.size.0 as _, texture.size.1 as _));
        let dst = dst
            .into()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, screen_size.0 as _, screen_size.1 as _));
        unsafe {
            gl::UseProgram(self.program);
            gl::Uniform4f(
                self.src_pos_size_location,
                src.x / texture.size.0 as f32,
                src.y / texture.size.1 as f32,
                src.width / texture.size.0 as f32,
                src.height / texture.size.1 as f32,
            );
            gl::Uniform4f(
                self.dst_pos_size_location,
                dst.x / screen_size.0,
                dst.y / screen_size.1,
                dst.width / screen_size.0,
                dst.height / screen_size.1,
            );
            gl::BindTexture(gl::TEXTURE_2D, texture.id);
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::UseProgram(0);
            check_gl()?;
        }
        Ok(())
    }
}

impl<T: TextureRendererKind> Drop for TextureRenderer<T> {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.program) }
    }
}

const VERTEX_SHADER: &str = "
#version 130

uniform vec4 src_pos_size;
uniform vec4 dst_pos_size;
out vec2 texCoord;

void main()
{
    float x = (gl_VertexID & 1);
    float y = (gl_VertexID & 2) >> 1;
    float src_x = src_pos_size.x + src_pos_size.z * x;
    float src_y = src_pos_size.y + src_pos_size.w * y;
    texCoord.x = src_x;
    texCoord.y = src_y;
    float dst_x = dst_pos_size.x + dst_pos_size.z * x;
    float dst_y = dst_pos_size.y + dst_pos_size.w * y;
    gl_Position = vec4(dst_x*2-1, dst_y*2-1, 0, 1);
}
";

const FRAGMENT_SHADER_F32: &str = "
#version 130

uniform sampler2D tex;
in vec2 texCoord;

void main()
{
    vec4 color1 = texture(tex, texCoord);
    gl_FragColor = color1;
}
";

const FRAGMENT_SHADER_U8: &str = "
#version 130

uniform usampler2D tex;
in vec2 texCoord;

void main()
{
    vec4 color1 = texture(tex, texCoord);
    color1.w = 1.0;
    gl_FragColor = color1 / 255.0;
}
";
