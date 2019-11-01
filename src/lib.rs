pub mod display;
pub mod render_text;
pub mod render_texture;
pub mod texture;

use failure::Error;
use gl::types::*;
use std::{
    ffi::{c_void, CStr, CString},
    ops::Add,
    ptr::{null, null_mut},
    slice, str,
};

fn check_gl() -> Result<(), Error> {
    // This should technically loop.
    let er = unsafe { gl::GetError() };
    if er == gl::NO_ERROR {
        return Ok(());
    }
    Err(failure::err_msg(format!("OGL error: {}", er)))
}

fn gl_register_debug() -> Result<(), Error> {
    unsafe {
        gl::DebugMessageCallback(Some(debug_callback), null());
    }
    check_gl()?;
    Ok(())
}

extern "system" fn debug_callback(
    source: GLenum,
    type_: GLenum,
    id: GLuint,
    severity: GLenum,
    length: GLsizei,
    message: *const GLchar,
    _: *mut c_void,
) {
    let msg =
        str::from_utf8(unsafe { slice::from_raw_parts(message as *const u8, length as usize) });
    println!(
        "GL debug callback: source:{} type:{} id:{} severity:{} {:?}",
        source, type_, id, severity, msg
    );
}

#[derive(Clone, Debug)]
pub struct Rect<T> {
    pub x: T,
    pub y: T,
    pub width: T,
    pub height: T,
}

impl<T> Rect<T> {
    pub fn new(x: T, y: T, width: T, height: T) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> <T as Add>::Output
    where
        T: Add<T> + Copy,
    {
        self.x + self.width
    }

    pub fn bottom(&self) -> <T as Add>::Output
    where
        T: Add<T> + Copy,
    {
        self.y + self.height
    }
}

fn get_uniform_location(kernel: GLuint, key: &str) -> GLint {
    let key = CString::new(key).expect("Failed to convert uniform name to null-terminated string");
    unsafe { gl::GetUniformLocation(kernel, key.as_ptr() as *const GLchar) }
}

pub fn set_arg_f32(kernel: GLuint, key: &str, value: f32) -> Result<(), Error> {
    let location = get_uniform_location(kernel, key);
    if location != -1 {
        unsafe {
            gl::UseProgram(kernel);
            gl::Uniform1f(location, value);
            gl::UseProgram(0);
        }
    }
    check_gl()?;
    Ok(())
}

pub fn set_arg_u32(kernel: GLuint, key: &str, value: u32) -> Result<(), Error> {
    let location = get_uniform_location(kernel, key);
    if location != -1 {
        unsafe {
            gl::UseProgram(kernel);
            gl::Uniform1ui(location, value);
            gl::UseProgram(0);
        }
    }
    check_gl()?;
    Ok(())
}

pub fn create_compute_program(sources: &[&str]) -> Result<GLuint, Error> {
    let shader = create_shader(sources, gl::COMPUTE_SHADER)?;
    create_program(&[shader])
}

pub fn create_vert_frag_program(vertex: &[&str], fragment: &[&str]) -> Result<GLuint, Error> {
    let vertex = create_shader(vertex, gl::VERTEX_SHADER)?;
    let fragment = create_shader(fragment, gl::FRAGMENT_SHADER)?;
    create_program(&[vertex, fragment])
}

pub fn create_program(shaders: &[GLuint]) -> Result<GLuint, Error> {
    unsafe {
        let program = gl::CreateProgram();
        for &shader in shaders {
            gl::AttachShader(program, shader);
        }
        gl::LinkProgram(program);

        let mut success = 0;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut success);
        if success == 0 {
            let mut info_log: [GLchar; 512] = [0; 512];
            let ptr = info_log.as_mut_ptr();
            gl::GetProgramInfoLog(program, 512, null_mut(), ptr);
            let log = CStr::from_ptr(ptr)
                .to_str()
                .expect("Invalid OpenGL error message");
            panic!("Failed to compile OpenGL program:\n{}", log);
        }
        check_gl()?;

        for &shader in shaders {
            gl::DeleteShader(shader);
        }

        check_gl()?;

        Ok(program)
    }
}

pub fn create_shader(sources: &[&str], shader_type: GLenum) -> Result<GLuint, Error> {
    unsafe {
        let shader = gl::CreateShader(shader_type);
        check_gl()?;
        let vec_sources = sources
            .iter()
            .map(|source| source.as_ptr() as *const GLchar)
            .collect::<Vec<_>>();
        let lengths = sources
            .iter()
            .map(|source| source.len() as GLint)
            .collect::<Vec<_>>();
        gl::ShaderSource(
            shader,
            vec_sources.len() as GLsizei,
            vec_sources.as_ptr() as *const *const GLchar,
            lengths.as_ptr(),
        );
        check_gl()?;
        gl::CompileShader(shader);
        check_gl()?;
        let mut success = 0;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);
        if success == 0 {
            let mut info_log: [GLchar; 512] = [0; 512];
            let ptr = info_log.as_mut_ptr();
            gl::GetShaderInfoLog(shader, 512, null_mut(), ptr);
            let log = CStr::from_ptr(ptr)
                .to_str()
                .expect("Invalid OpenGL error message");
            panic!("Failed to compile OpenGL shader:\n{}", log);
        }
        check_gl()?;
        Ok(shader)
    }
}
