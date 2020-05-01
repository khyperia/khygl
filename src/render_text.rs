use crate::{
    render_texture::TextureRenderer,
    texture::{CpuTexture, Texture},
    Error, Rect,
};
use rusttype::{point, Font, Point, PositionedGlyph, Scale};
use std::{
    collections::{hash_map::Entry, HashMap},
    fs::File,
    io::prelude::*,
    path::Path,
};

struct AtlasEntry {
    texture: Texture<[f32; 4]>,
    x_pos: isize,
    y_pos: isize,
    stride: isize,
}

pub struct TextRenderer {
    pub spacing: usize,
    scale: Scale,
    offset: Point<f32>,
    atlas: HashMap<char, AtlasEntry>,
    font: Font<'static>,
}

impl TextRenderer {
    pub fn new(height: f32) -> Result<Self, Error> {
        let font_data = load_font()?;
        let font = Font::try_from_vec(font_data).ok_or_else(|| "Failed to load font data")?;

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

        Ok(Self {
            spacing: spacing as usize,
            scale,
            offset,
            atlas: HashMap::new(),
            font,
        })
    }

    fn get_entry(&mut self, ch: char) -> Result<&mut AtlasEntry, Error> {
        match self.atlas.entry(ch) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                let chstr = ch.to_string();
                let mut glyphseq = self.font.layout(&chstr, self.scale, self.offset);
                let glyph = glyphseq.next().expect("Empty glyph sequence");
                let rendered = render_char(&glyph)?;
                Ok(entry.insert(rendered))
            }
        }
    }

    pub fn render(
        &mut self,
        renderer: &TextureRenderer,
        text: &str,
        color_rgba: [f32; 4],
        position: (usize, usize),
        screen_size: (usize, usize),
    ) -> Result<Rect<usize>, Error> {
        let mut max_x = position.0 as isize;
        let mut max_y = position.1 as isize;
        let mut x = position.0 as isize;
        let mut y = position.1 as isize;
        for ch in text.chars() {
            if ch == '\n' {
                y += self.spacing as isize;
                x = position.0 as isize;
            } else if ch == ' ' {
                x += self.get_entry('*')?.stride;
            } else {
                let tex = self.get_entry(ch)?;
                let dst = Rect::new(
                    (x + tex.x_pos) as f32,
                    (y + tex.y_pos) as f32,
                    tex.texture.size.0 as f32,
                    tex.texture.size.1 as f32,
                );
                let screen_size = (screen_size.0 as f32, screen_size.1 as f32);
                renderer
                    .render(&tex.texture, screen_size)
                    .dst(dst)
                    .tint(color_rgba)
                    .go()?;
                x += tex.stride;

                max_x = max_x.max(x);

                max_y = max_y.max(y + tex.y_pos + tex.texture.size.0 as isize);
            }
        }
        Ok(Rect::new(
            position.0,
            position.1,
            max_x as usize - position.0,
            max_y as usize - position.1,
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
        x_pos: h_metrics.left_side_bearing.ceil() as isize,
        y_pos: bb.min.y as isize,
        stride: h_metrics.advance_width.ceil() as isize,
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
        "C:\\Windows\\Fonts\\arial.ttf".as_ref(),
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf".as_ref(),
        "/usr/share/fonts/TTF/FiraMono-Regular.ttf".as_ref(),
        "/usr/share/fonts/TTF/DejaVuSans.ttf".as_ref(),
        "/usr/share/fonts/TTF/LiberationSans-Regular.ttf".as_ref(),
        "/Library/Fonts/Andale Mono.ttf".as_ref(),
    ];
    for &location in &locations {
        if location.exists() {
            return Ok(location);
        }
    }
    Err("No font found".into())
}
