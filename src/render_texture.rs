use crate::{
    check_gl, create_vert_frag_program,
    texture::{CpuTexture, Texture, TextureType},
    Rect,
};
use failure::Error;
use gl::{self, types::*};
use std::sync::Once;

// https://rauwendaal.net/2014/06/14/rendering-a-screen-covering-triangle-in-opengl/

pub struct TextureRenderer {
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

impl TextureRenderer {
    pub fn new() -> Result<Self, Error> {
        check_gl()?;
        let program = create_vert_frag_program(&[VERTEX_SHADER], &[FRAGMENT_SHADER])?;
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

    pub fn render<Ty: TextureType>(
        &self,
        texture: &Texture<Ty>,
        src: impl Into<Option<Rect<f32>>>,
        dst: impl Into<Option<Rect<f32>>>,
        tint: impl Into<Option<[f32; 4]>>,
        screen_size: (f32, f32),
    ) -> Result<(), Error> {
        let src = src
            .into()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, texture.size.0 as _, texture.size.1 as _));
        let dst = dst
            .into()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, screen_size.0 as _, screen_size.1 as _));
        let tint = tint.into().unwrap_or_else(|| [1.0, 1.0, 1.0, 1.0]);
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

    pub fn line_x(
        &self,
        x_start: usize,
        x_end: usize,
        y: usize,
        color: [f32; 4],
        screen_size: (f32, f32),
    ) -> Result<(), Error> {
        self.render(
            texture1x1(),
            None,
            Rect::new(x_start as f32, y as f32, (x_end - x_start) as f32, 1.0),
            color,
            screen_size,
        )
    }

    pub fn line_y(
        &self,
        x: usize,
        y_start: usize,
        y_end: usize,
        color: [f32; 4],
        screen_size: (f32, f32),
    ) -> Result<(), Error> {
        self.render(
            texture1x1(),
            None,
            Rect::new(x as f32, y_start as f32, 1.0, (y_end - y_start) as f32),
            color,
            screen_size,
        )
    }

    pub fn rect(
        &self,
        rect: Rect<usize>,
        color: [f32; 4],
        screen_size: (f32, f32),
    ) -> Result<(), Error> {
        self.line_x(rect.x, rect.right(), rect.y, color, screen_size)?;
        self.line_x(rect.x, rect.right(), rect.bottom(), color, screen_size)?;
        self.line_y(rect.x, rect.y, rect.bottom(), color, screen_size)?;
        self.line_y(rect.right(), rect.y, rect.bottom(), color, screen_size)?;
        Ok(())
    }
}

impl Drop for TextureRenderer {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.program) }
    }
}

fn texture1x1() -> &'static Texture<[u8; 4]> {
    static TEXTURE1X1_ONCE: Once = Once::new();
    static mut TEXTURE1X1_VAL: Option<Texture<[u8; 4]>> = None;
    TEXTURE1X1_ONCE.call_once(|| {
        let mut texture1x1 = Texture::new((1, 1)).expect("Failed to create 1x1 texture");
        texture1x1
            .upload(&CpuTexture::new(vec![[255, 255, 255, 255]], (1, 1)))
            .expect("Failed to upload to 1x1 texture");
        unsafe {
            TEXTURE1X1_VAL = Some(texture1x1);
        }
    });
    unsafe { TEXTURE1X1_VAL.as_ref().expect("std::sync::Once didn't run") }
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

const FRAGMENT_SHADER: &str = "
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
