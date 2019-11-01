use crate::check_gl;
use failure::Error;
use gl::types::*;
use std::{ffi::c_void, marker::PhantomData};

pub trait TextureType: Clone + Default {
    fn internalformat() -> GLuint;
    fn format() -> GLuint;
    fn type_() -> GLuint;
    fn size() -> usize {
        std::mem::size_of::<Self>()
    }
}

impl TextureType for [f32; 4] {
    fn internalformat() -> GLuint {
        gl::RGBA32F
    }
    fn format() -> GLuint {
        gl::RGBA
    }
    fn type_() -> GLuint {
        gl::FLOAT
    }
}

impl TextureType for [u8; 4] {
    fn internalformat() -> GLuint {
        gl::RGBA8UI
    }
    fn format() -> GLuint {
        gl::RGBA_INTEGER
    }
    fn type_() -> GLuint {
        gl::UNSIGNED_INT_8_8_8_8
    }
}

impl TextureType for [u32; 2] {
    fn internalformat() -> GLuint {
        gl::RG32UI
    }
    fn format() -> GLuint {
        gl::RG_INTEGER
    }
    fn type_() -> GLuint {
        gl::UNSIGNED_INT
    }
}

impl TextureType for u32 {
    fn internalformat() -> GLuint {
        gl::R32UI
    }
    fn format() -> GLuint {
        gl::RED_INTEGER
    }
    fn type_() -> GLuint {
        gl::UNSIGNED_INT
    }
}

pub struct Texture<T: TextureType> {
    pub id: GLuint,
    pub size: (usize, usize),
    _t: PhantomData<T>,
}

impl<T: TextureType> Texture<T> {
    pub fn new(size: (usize, usize)) -> Result<Self, Error> {
        let format = T::internalformat();
        let mut texture = 0;
        unsafe {
            gl::CreateTextures(gl::TEXTURE_2D, 1, &mut texture);
            check_gl()?;
            gl::TextureStorage2D(texture, 1, format, size.0 as _, size.1 as _);
            check_gl()?;
            gl::TextureParameteri(texture, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);
            check_gl()?;
            gl::TextureParameteri(texture, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
            check_gl()?;
        }
        Ok(Self {
            id: texture,
            size,
            _t: PhantomData,
        })
    }

    pub fn download(&mut self) -> Result<CpuTexture<T>, Error> {
        let mut pixels = vec![T::default(); self.size.0 * self.size.1];
        let buf_size = T::size() * pixels.len();
        unsafe {
            gl::GetTextureImage(
                self.id,
                0,
                T::format(),
                T::type_(),
                buf_size as i32,
                pixels.as_mut_ptr() as *mut _,
            );
            check_gl()?;
        }
        Ok(CpuTexture::new(pixels, self.size))
    }

    pub fn upload(&mut self, cpu_texture: &CpuTexture<T>) -> Result<(), Error> {
        assert_eq!(self.size, cpu_texture.size);
        unsafe {
            gl::TextureSubImage2D(
                self.id,
                0,
                0,
                0,
                self.size.0 as _,
                self.size.1 as _,
                T::format(),
                T::type_(),
                cpu_texture.data.as_ptr() as *const c_void,
            );
            check_gl()?;
        }
        Ok(())
    }

    pub fn bind(&self, unit: usize) -> Result<(), Error> {
        unsafe {
            gl::BindImageTexture(
                unit as GLuint,
                self.id,
                0,
                gl::FALSE,
                0,
                gl::READ_WRITE,
                T::internalformat(),
            );
            check_gl()
        }
    }
}

impl<T: TextureType> Drop for Texture<T> {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.id);
        }
        check_gl().expect("Failed to delete texture in drop impl");
    }
}

pub struct CpuTexture<T> {
    pub data: Vec<T>,
    pub size: (usize, usize),
}

impl<T> CpuTexture<T> {
    pub fn new(data: Vec<T>, size: (usize, usize)) -> Self {
        Self { data, size }
    }
}

pub struct VertexBuffer<T> {
    pub id: GLuint,
    _t: PhantomData<T>,
}

impl<T> VertexBuffer<T> {
    pub fn new() -> Result<Self, Error> {
        let mut id = 0;
        unsafe {
            gl::CreateBuffers(1, &mut id);
            check_gl()?;
        }
        Ok(Self {
            id,
            _t: PhantomData,
        })
    }

    pub fn set_data(&mut self, data: &[T], usage: GLenum) -> Result<(), Error> {
        // usage must be: GL_STREAM_DRAW, GL_STREAM_READ, GL_STREAM_COPY, GL_STATIC_DRAW, GL_STATIC_READ, GL_STATIC_COPY, GL_DYNAMIC_DRAW, GL_DYNAMIC_READ, or GL_DYNAMIC_COPY
        unsafe {
            gl::NamedBufferData(self.id, data.len() as isize, data.as_ptr() as _, usage);
            check_gl()?;
        }
        Ok(())
    }
}

impl<T> Drop for VertexBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.id);
            check_gl().expect("Failed to delete buffer in drop impl");
        }
    }
}

pub struct VertexArray {
    pub id: GLuint,
}

// Steps:
// 0) gl::BindAttribLocation on the shader to associate attrib_index to variable name
// 1) bind VertexBuffer to a bind_index using bind_buffer_to_bind_index
// 2) associate a attrib_index to a bind_index using associate_attrib_index_to_bind_index
// 3) specify the format of an attrib_index using attrib_format_*
// 4) gl::BindVertexArray to draw with gl::DrawArrays
impl VertexArray {
    pub fn new() -> Result<Self, Error> {
        let mut id = 0;
        unsafe {
            gl::CreateVertexArrays(1, &mut id);
            check_gl()?;
        }
        Ok(Self { id })
    }

    pub fn enable_attrib(&self, attrib_index: GLuint) -> Result<(), Error> {
        unsafe {
            gl::EnableVertexArrayAttrib(self.id, attrib_index);
            check_gl()?;
        }
        Ok(())
    }

    pub fn disable_attrib(&self, attrib_index: GLuint) -> Result<(), Error> {
        unsafe {
            gl::DisableVertexArrayAttrib(self.id, attrib_index);
            check_gl()?;
        }
        Ok(())
    }

    pub fn bind_buffer_to_bind_index<T>(
        &self,
        bind_index: GLuint,
        buffer: &VertexBuffer<T>,
        offset: GLintptr,
        stride: GLsizei,
    ) -> Result<(), Error> {
        unsafe {
            gl::VertexArrayVertexBuffer(self.id, bind_index, buffer.id, offset, stride);
            check_gl()?;
        }
        Ok(())
    }

    pub fn associate_attrib_index_to_bind_index(&self, attrib_index: GLuint, bind_index: GLuint) -> Result<(), Error> {
        unsafe {
            gl::VertexArrayAttribBinding(self.id, attrib_index, bind_index);
            check_gl()?;
        }
        Ok(())
    }

    // size: num elements per vertex
    // type: gl::FLOAT, etc.
    // "relativeoffset is the offset, measured in basic machine units of the first element relative to the start of the vertex buffer binding this attribute fetches from."
    pub fn attrib_format_float(
        &self,
        attrib_index: GLuint,
        size: GLint,
        type_: GLenum,
        normalized: bool,
        relative_offset: GLuint,
    ) -> Result<(), Error> {
        unsafe {
            let normalized = if normalized { gl::TRUE } else { gl::FALSE };
            gl::VertexArrayAttribFormat(self.id, attrib_index, size, type_, normalized, relative_offset);
            check_gl()?;
        }
        Ok(())
    }

    pub fn attrib_format_int(
        &self,
        attrib_index: GLuint,
        size: GLint,
        type_: GLenum,
        relative_offset: GLuint,
    ) -> Result<(), Error> {
        unsafe {
            gl::VertexArrayAttribIFormat(self.id, attrib_index, size, type_, relative_offset);
            check_gl()?;
        }
        Ok(())
    }

    pub fn bind(&self) -> Result<(), Error> {
        unsafe {
            gl::BindVertexArray(self.id);
            check_gl()?;
        }
        Ok(())
    }

    pub fn unbind(&self) -> Result<(), Error> {
        unsafe {
            gl::BindVertexArray(0);
            check_gl()?;
        }
        Ok(())
    }
}

impl Drop for VertexArray {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &self.id);
            check_gl().expect("Failed to delete vertex array in drop impl");
        }
    }
}
