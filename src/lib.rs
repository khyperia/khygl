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

pub fn check_gl() -> Result<(), Error> {
    // This should technically loop.
    let er = unsafe { gl::GetError() };
    if er == gl::NO_ERROR {
        return Ok(());
    }
    Err(failure::err_msg(format!("OGL error: {}", er)))
}

pub fn gl_register_debug() -> Result<(), Error> {
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

pub fn set_arg_f32_3(kernel: GLuint, key: &str, x: f32, y: f32, z: f32) -> Result<(), Error> {
    let location = get_uniform_location(kernel, key);
    if location != -1 {
        unsafe {
            gl::UseProgram(kernel);
            gl::Uniform3f(location, x, y, z);
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

pub struct CompileResult {
    pub shader: GLuint,
    pub success: bool,
    pub log: String,
}

pub fn create_compute_program(sources: &[&str]) -> Result<CompileResult, Error> {
    let shader = create_shader(sources, gl::COMPUTE_SHADER)?;
    if !shader.success {
        return Ok(CompileResult {
            shader: 0,
            success: shader.success,
            log: shader.log,
        });
    }
    create_program(&[shader.shader])
}

pub fn create_vert_frag_program(
    vertex: &[&str],
    fragment: &[&str],
) -> Result<CompileResult, Error> {
    let vertex = create_shader(vertex, gl::VERTEX_SHADER)?;
    if !vertex.success {
        return Ok(CompileResult {
            shader: 0,
            success: vertex.success,
            log: vertex.log,
        });
    }
    let fragment = create_shader(fragment, gl::FRAGMENT_SHADER)?;
    if !fragment.success {
        return Ok(CompileResult {
            shader: 0,
            success: fragment.success,
            log: fragment.log,
        });
    }
    let mut result = create_program(&[vertex.shader, fragment.shader])?;
    if result.success {
        result.log = format!("{}{}{}", vertex.log, fragment.log, result.log);
    }
    Ok(result)
}

pub fn create_program(shaders: &[GLuint]) -> Result<CompileResult, Error> {
    unsafe {
        let program = gl::CreateProgram();
        for &shader in shaders {
            gl::AttachShader(program, shader);
        }
        gl::LinkProgram(program);

        let mut success = 0;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut success);
        check_gl()?;
        let mut info_log_length = 0;
        gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut info_log_length);
        check_gl()?;
        let log = if info_log_length == 0 {
            ""
        } else {
            let mut info_log = vec![0; info_log_length as usize];
            let ptr = info_log.as_mut_ptr();
            gl::GetProgramInfoLog(program, info_log_length, null_mut(), ptr);
            CStr::from_bytes_with_nul(std::slice::from_raw_parts(
                ptr as *const u8,
                info_log_length as usize,
            ))?
            .to_str()?
        };
        check_gl()?;

        for &shader in shaders {
            gl::DeleteShader(shader);
        }

        check_gl()?;

        Ok(CompileResult {
            shader: program,
            success: success == (gl::TRUE as _),
            log: log.to_string(),
        })
    }
}

pub fn create_shader(sources: &[&str], shader_type: GLenum) -> Result<CompileResult, Error> {
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
        check_gl()?;
        let mut info_log_length = 0;
        gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut info_log_length);
        check_gl()?;
        let log = if info_log_length == 0 {
            ""
        } else {
            let mut info_log = vec![0; info_log_length as usize];
            let ptr = info_log.as_mut_ptr();
            gl::GetShaderInfoLog(shader, info_log_length, null_mut(), ptr);
            CStr::from_bytes_with_nul(std::slice::from_raw_parts(
                ptr as *const u8,
                info_log_length as usize,
            ))?
            .to_str()?
        };
        check_gl()?;
        Ok(CompileResult {
            shader,
            success: success == (gl::TRUE as _),
            log: log.to_string(),
        })
    }
}
