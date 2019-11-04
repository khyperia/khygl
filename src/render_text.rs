use crate::{
    render_texture::TextureRendererF32,
    texture::{CpuTexture, Texture},
    Rect,
};
use failure::{err_msg, Error};
use rusttype::{point, FontCollection, PositionedGlyph, Scale};
use std::{convert::TryInto, fs::File, io::prelude::*, path::Path};

const OFFSET: u8 = 33;
const MAX: u8 = 127;

struct AtlasEntry {
    texture: Texture<[f32; 4]>,
    x_pos: usize,
    y_pos: usize,
    stride: usize,
}

pub struct TextRenderer {
    pub spacing: usize,
    atlas: Vec<AtlasEntry>,
}

impl TextRenderer {
    pub fn new(height: f32) -> Result<Self, Error> {
        let font_data = load_font()?;
        let collection = FontCollection::from_bytes(&font_data)?;
        let font = collection.into_font()?;

        let scale = Scale {
            x: height,
            y: height,
        };

        // The origin of a line of text is at the baseline (roughly where
        // non-descending letters sit). We don't want to clip the text, so we shift
        // it down with an offset when laying it out. v_metrics.ascent is the
        // distance between the baseline and the highest edge of any glyph in
        // the font. That's enough to guarantee that there's no clipping.
        let v_metrics = font.v_metrics(scale);
        let offset = point(0.0, v_metrics.ascent);
        let spacing = v_metrics.ascent - v_metrics.descent + v_metrics.line_gap;

        let string = (OFFSET..MAX).map(|c| c as char).collect::<String>();
        let atlas = font
            .layout(&string, scale, offset)
            .map(|glyph| render_char(&glyph))
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(Self {
            atlas,
            spacing: spacing as usize,
        })
    }

    pub fn render(
        &self,
        renderer: &TextureRendererF32,
        text: &str,
        color_rgba: [f32; 4],
        position: (usize, usize),
        screen_size: (usize, usize),
    ) -> Result<Rect<usize>, Error> {
        let mut max_x = position.0;
        let mut max_y = position.1;
        let mut x = position.0;
        let mut y = position.1;
        for ch in text.chars() {
            if ch == '\n' {
                y += self.spacing;
                x = position.0;
            } else if ch == ' ' {
                x += self.atlas[(b'*' - OFFSET) as usize].stride;
            } else if let Some(tex) = (ch as isize - OFFSET as isize)
                .try_into()
                .ok()
                .and_then(|idx: usize| self.atlas.get(idx))
            {
                let src = None;
                let dst = Rect::new(
                    (x + tex.x_pos) as f32,
                    (y + tex.y_pos) as f32,
                    tex.texture.size.0 as f32,
                    tex.texture.size.1 as f32,
                );
                renderer.render(
                    &tex.texture,
                    src,
                    dst,
                    color_rgba,
                    (screen_size.0 as f32, screen_size.1 as f32),
                )?;
                x += tex.stride;

                max_x = max_x.max(x);

                max_y = max_y.max(y + tex.y_pos + tex.texture.size.0);
            } else {
                x += self.atlas[(b'*' - OFFSET) as usize].stride;
            }
        }
        Ok(Rect::new(
            position.0,
            position.1,
            max_x - position.0,
            max_y - position.1,
        ))
    }
}

fn render_char(glyph: &PositionedGlyph) -> Result<AtlasEntry, Error> {
    let bb = glyph
        .pixel_bounding_box()
        .expect("Could not get bounding box of glyph");
    let h_metrics = glyph.unpositioned().h_metrics();
    let width = bb.width();
    let height = bb.height();

    let mut pixels = vec![[0.0, 0.0, 0.0, 0.0]; width as usize * height as usize];

    glyph.draw(|x, y, v| {
        let index = y as usize * width as usize + x as usize;
        pixels[index] = [1.0, 1.0, 1.0, v];
    });

    let mut texture = Texture::new((width as usize, height as usize))?;
    texture.upload(&CpuTexture::new(pixels, (width as usize, height as usize)))?;

    Ok(AtlasEntry {
        texture,
        x_pos: h_metrics.left_side_bearing.ceil() as usize,
        y_pos: bb.min.y as usize,
        stride: h_metrics.advance_width.ceil() as usize,
    })
}

fn load_font() -> Result<Vec<u8>, Error> {
    let path = find_font()?;
    let mut file = File::open(path)?;
    let mut contents = vec![];
    file.read_to_end(&mut contents)?;
    Ok(contents)
}

fn find_font() -> Result<&'static Path, Error> {
    let locations: [&'static Path; 6] = [
        "/usr/share/fonts/TTF/FiraMono-Regular.ttf".as_ref(),
        "/usr/share/fonts/TTF/FiraSans-Regular.ttf".as_ref(),
        "C:\\Windows\\Fonts\\arial.ttf".as_ref(),
        "/usr/share/fonts/TTF/DejaVuSans.ttf".as_ref(),
        "/usr/share/fonts/TTF/LiberationSans-Regular.ttf".as_ref(),
        "/Library/Fonts/Andale Mono.ttf".as_ref(),
    ];
    for &location in &locations {
        if location.exists() {
            return Ok(location);
        }
    }
    Err(err_msg("No font found"))
}
