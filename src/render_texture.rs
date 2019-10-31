use crate::{
    check_gl, create_vert_frag_program,
    texture::{Texture, TextureType},
    Rect,
};
use failure::Error;
use gl::{self, types::*};

// https://rauwendaal.net/2014/06/14/rendering-a-screen-covering-triangle-in-opengl/

struct TextureRendererBase {
    program: GLuint,
    src_pos_size_location: GLint,
    dst_pos_size_location: GLint,
    tint_location: GLint,
}

fn map_n1(x: GLint) -> Result<GLint, Error> {
    if x == -1 {
        Err(failure::err_msg("uniform not found"))
    } else {
        Ok(x)
    }
}

impl TextureRendererBase {
    fn new(frag: &'static str) -> Result<Self, Error> {
        check_gl()?;
        let program = unsafe { create_vert_frag_program(&[VERTEX_SHADER], &[frag])? };
        let src_pos_size_location = unsafe {
            map_n1(gl::GetUniformLocation(
                program,
                b"src_pos_size\0".as_ptr() as *const GLchar,
            ))
        };
        check_gl()?;
        let src_pos_size_location = src_pos_size_location?;
        let dst_pos_size_location = unsafe {
            map_n1(gl::GetUniformLocation(
                program,
                b"dst_pos_size\0".as_ptr() as *const GLchar,
            ))
        };
        check_gl()?;
        let dst_pos_size_location = dst_pos_size_location?;
        let tint_location = unsafe {
            map_n1(gl::GetUniformLocation(
                program,
                b"tint\0".as_ptr() as *const GLchar,
            ))
        };
        check_gl()?;
        let tint_location = tint_location?;
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
            tint_location,
        })
    }

    fn render<Ty: TextureType>(
        &self,
        texture: &Texture<Ty>,
        src: Option<Rect<f32>>,
        dst: Option<Rect<f32>>,
        tint: Option<[f32; 4]>,
        screen_size: (f32, f32),
    ) -> Result<(), Error> {
        let src =
            src.unwrap_or_else(|| Rect::new(0.0, 0.0, texture.size.0 as _, texture.size.1 as _));
        let dst =
            dst.unwrap_or_else(|| Rect::new(0.0, 0.0, screen_size.0 as _, screen_size.1 as _));
        let tint = tint.unwrap_or_else(|| [1.0, 1.0, 1.0, 1.0]);
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
            gl::Uniform4f(self.tint_location, tint[0], tint[1], tint[2], tint[3]);
            gl::BindTexture(gl::TEXTURE_2D, texture.id);
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::UseProgram(0);
            check_gl()?;
        }
        Ok(())
    }
}

impl Drop for TextureRendererBase {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.program) }
    }
}

pub struct TextureRendererF32 {
    base: TextureRendererBase,
}

impl TextureRendererF32 {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            base: TextureRendererBase::new(FRAGMENT_SHADER_F32)?,
        })
    }

    pub fn render<Ty: TextureType>(
        &self,
        texture: &Texture<Ty>,
        src: impl Into<Option<Rect<f32>>>,
        dst: impl Into<Option<Rect<f32>>>,
        tint: impl Into<Option<[f32; 4]>>,
        screen_size: (f32, f32),
    ) -> Result<(), Error> {
        self.base
            .render(texture, src.into(), dst.into(), tint.into(), screen_size)
    }
}

pub struct TextureRendererU8 {
    base: TextureRendererBase,
}

impl TextureRendererU8 {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            base: TextureRendererBase::new(FRAGMENT_SHADER_U8)?,
        })
    }

    pub fn render<Ty: TextureType>(
        &self,
        texture: &Texture<Ty>,
        src: impl Into<Option<Rect<f32>>>,
        dst: impl Into<Option<Rect<f32>>>,
        tint: impl Into<Option<[f32; 4]>>,
        screen_size: (f32, f32),
    ) -> Result<(), Error> {
        self.base
            .render(texture, src.into(), dst.into(), tint.into(), screen_size)
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
    // flip coordinate space
    gl_Position = vec4(dst_x*2-1, dst_y*-2+1, 0, 1);
}
";

const FRAGMENT_SHADER_F32: &str = "
#version 130

uniform vec4 tint;
uniform sampler2D tex;
in vec2 texCoord;

void main()
{
    vec4 color1 = texture(tex, texCoord);
    gl_FragColor = color1 * tint;
}
";

const FRAGMENT_SHADER_U8: &str = "
#version 130

uniform vec4 tint;
uniform usampler2D tex;
in vec2 texCoord;

void main()
{
    vec4 color1 = texture(tex, texCoord);
    color1.w = 255.0;
    gl_FragColor = color1 / 255.0 * tint;
}
";
