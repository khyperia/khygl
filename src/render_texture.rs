use crate::{
    check_gl, create_vert_frag_program,
    texture::{CpuTexture, Texture, TextureType},
    Error, Rect,
};
use gl::{self, types::*};
use std::sync::Once;

// https://rauwendaal.net/2014/06/14/rendering-a-screen-covering-triangle-in-opengl/

pub struct TextureRenderer {
    program: GLuint,
    dummy_buffer: GLuint,
    src_pos_size_location: GLint,
    dst_pos_size_location: GLint,
    tint_location: GLint,
    scale_offset_location: GLint,
    img_size_location: Option<GLint>,
}

fn uniform(program: GLuint, var: &[u8]) -> Result<GLint, Error> {
    assert!(var[var.len() - 1] == 0);
    let location = unsafe { gl::GetUniformLocation(program, var.as_ptr() as *const GLchar) };
    check_gl()?;
    if location == -1 {
        Err("uniform not found".into())
    } else {
        Ok(location)
    }
}

impl TextureRenderer {
    fn impl_new(frag: &str) -> Result<Self, Error> {
        check_gl()?;
        let program = create_vert_frag_program(&[VERTEX_SHADER], &[frag])?;
        if !program.success {
            panic!("Failed to compile shader: {}", program.log);
        }
        let program = program.shader;
        let src_pos_size_location = uniform(program, b"src_pos_size\0")?;
        let dst_pos_size_location = uniform(program, b"dst_pos_size\0")?;
        let tint_location = uniform(program, b"tint\0")?;
        let scale_offset_location = uniform(program, b"scale_offset\0")?;
        let img_size_location = uniform(program, b"img_size\0").ok();
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        }
        let mut dummy_buffer = 0;
        unsafe {
            gl::GenVertexArrays(1, &mut dummy_buffer);
        }
        check_gl()?;
        Ok(Self {
            program,
            dummy_buffer,
            src_pos_size_location,
            dst_pos_size_location,
            tint_location,
            scale_offset_location,
            img_size_location,
        })
    }

    pub fn new() -> Result<Self, Error> {
        Self::impl_new(FRAGMENT_SHADER)
    }

    pub fn new_binning() -> Result<Self, Error> {
        Self::impl_new(FRAGMENT_SHADER_BINNING)
    }

    pub fn render<'a, 'b, Ty: TextureType>(
        &'a self,
        texture: &'b Texture<Ty>,
        screen_size: (f32, f32),
    ) -> RenderBuilder<'a, 'b, Ty> {
        RenderBuilder::new(self, texture, screen_size)
    }

    pub fn line_x(
        &self,
        x_start: usize,
        x_end: usize,
        y: usize,
        color: [f32; 4],
        screen_size: (f32, f32),
    ) -> Result<(), Error> {
        self.render(texture1x1(), screen_size)
            .dst(Rect::new(
                x_start as f32,
                y as f32,
                (x_end - x_start) as f32,
                1.0,
            ))
            .tint(color)
            .go()
    }

    pub fn line_y(
        &self,
        x: usize,
        y_start: usize,
        y_end: usize,
        color: [f32; 4],
        screen_size: (f32, f32),
    ) -> Result<(), Error> {
        self.render(texture1x1(), screen_size)
            .dst(Rect::new(
                x as f32,
                y_start as f32,
                1.0,
                (y_end - y_start) as f32,
            ))
            .tint(color)
            .go()
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
        unsafe {
            gl::DeleteVertexArrays(1, &self.dummy_buffer);
            gl::DeleteProgram(self.program);
        }
    }
}

#[must_use]
pub struct RenderBuilder<'renderer, 'texture, T: TextureType> {
    texture_renderer: &'renderer TextureRenderer,
    texture: &'texture Texture<T>,
    screen_size: (f32, f32),
    src: Option<Rect<f32>>,
    dst: Option<Rect<f32>>,
    tint: Option<[f32; 4]>,
    scale_offset: Option<(f32, f32)>,
}

impl<'renderer, 'texture, T: TextureType> RenderBuilder<'renderer, 'texture, T> {
    fn new(
        texture_renderer: &'renderer TextureRenderer,
        texture: &'texture Texture<T>,
        screen_size: (f32, f32),
    ) -> Self {
        Self {
            texture_renderer,
            texture,
            screen_size,
            src: None,
            dst: None,
            tint: None,
            scale_offset: None,
        }
    }

    pub fn src(mut self, src: Rect<f32>) -> Self {
        self.src = Some(src);
        self
    }

    pub fn dst(mut self, dst: Rect<f32>) -> Self {
        self.dst = Some(dst);
        self
    }

    pub fn tint(mut self, tint: [f32; 4]) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn scale_offset(mut self, scale_offset: (f32, f32)) -> Self {
        self.scale_offset = Some(scale_offset);
        self
    }

    pub fn go(mut self) -> Result<(), Error> {
        let src = self.src.take().unwrap_or_else(|| {
            Rect::new(0.0, 0.0, self.texture.size.0 as _, self.texture.size.1 as _)
        });
        let dst = self.dst.take().unwrap_or_else(|| {
            Rect::new(0.0, 0.0, self.screen_size.0 as _, self.screen_size.1 as _)
        });
        let tint = self.tint.take().unwrap_or_else(|| [1.0, 1.0, 1.0, 1.0]);
        let scale_offset = self.scale_offset.take().unwrap_or_else(|| (1.0, 0.0));
        unsafe {
            gl::UseProgram(self.texture_renderer.program);
            gl::Uniform4f(
                self.texture_renderer.src_pos_size_location,
                src.x / self.texture.size.0 as f32,
                src.y / self.texture.size.1 as f32,
                src.width / self.texture.size.0 as f32,
                src.height / self.texture.size.1 as f32,
            );
            gl::Uniform4f(
                self.texture_renderer.dst_pos_size_location,
                dst.x / self.screen_size.0,
                dst.y / self.screen_size.1,
                dst.width / self.screen_size.0,
                dst.height / self.screen_size.1,
            );
            gl::Uniform4f(
                self.texture_renderer.tint_location,
                tint[0],
                tint[1],
                tint[2],
                tint[3],
            );
            gl::Uniform2f(
                self.texture_renderer.scale_offset_location,
                scale_offset.0,
                scale_offset.1,
            );
            if let Some(img_size_location) = self.texture_renderer.img_size_location {
                gl::Uniform2i(
                    img_size_location,
                    self.texture.size.0 as GLint,
                    self.texture.size.1 as GLint,
                );
            }
            gl::BindTexture(gl::TEXTURE_2D, self.texture.id);
            gl::BindVertexArray(self.texture_renderer.dummy_buffer);
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
            gl::BindVertexArray(0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::UseProgram(0);
            check_gl()?;
        }
        Ok(())
    }
}

pub fn texture1x1() -> &'static Texture<[u8; 4]> {
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
#version 450

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
#version 450

uniform vec4 tint;
uniform vec2 scale_offset;
uniform sampler2D tex;
in vec2 texCoord;
layout(location = 0) out vec4 out_color;

void main()
{
    vec4 color1 = texture(tex, texCoord) * scale_offset.x + scale_offset.y;
    out_color = color1 * tint;
}
";

const FRAGMENT_SHADER_BINNING: &str = "
#version 450

uniform vec4 tint;
uniform vec2 scale_offset;
uniform sampler2D tex;
uniform ivec2 img_size;
in vec2 texCoord;
layout(location = 0) out vec4 out_color;

void main()
{
    vec2 delta_pos = vec2(dFdx(texCoord.x), dFdy(-texCoord.y));
    vec2 next_pos = texCoord + delta_pos;
    ivec2 img_coords = ivec2(texCoord * img_size);
    ivec2 next_coords = ivec2(next_pos * img_size);
    next_coords = clamp(next_coords, img_coords + ivec2(1, 1), img_coords + ivec2(10, 10));
    vec4 color1 = vec4(0.0f, 0.0f, 0.0f, 0.0f);
    int samples = 0;
    for (int y = img_coords.y; y < next_coords.y; y++) {
        for (int x = img_coords.x; x < next_coords.x; x++) {
            color1 += texelFetch(tex, ivec2(x, y), 0) * scale_offset.x + scale_offset.y;
            samples += 1;
        }
    }
    if (samples > 0) {
        out_color = color1 * tint / samples;
    } else {
        out_color = vec4(1.0, 0.0, 1.0, 1.0) * scale_offset.x + scale_offset.y;
    }
}
";
