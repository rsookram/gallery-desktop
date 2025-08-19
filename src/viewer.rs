use skia_safe::{
    codec::{jpeg_decoder, png_decoder, webp_decoder},
    Color, ISize, Paint, Rect, SamplingOptions,
};
use std::io::Cursor;
use std::path::PathBuf;

use crate::file_container::FileContainer;

pub struct Screen {
    show_progress: bool,
    paths: Paths,
    current_file: CurrentFile,
}

pub struct Paths {
    data: Vec<PathBuf>,
    /// The index into `data` of the file to display
    index: usize,
}

pub struct CurrentFile {
    file: FileContainer,
    /// The index of the image within the current file to display
    index: usize,
}

impl Screen {
    pub fn new(paths: Vec<PathBuf>) -> Self {
        Self {
            current_file: CurrentFile {
                file: FileContainer::open(&paths[0]),
                index: 0,
            },
            paths: Paths {
                data: paths,
                index: 0,
            },
            show_progress: false,
        }
    }

    pub fn current_image_bytes(&mut self) -> Vec<u8> {
        self.current_file.file.read_at(self.current_file.index)
    }

    pub fn next_image(&mut self) {
        if self.current_file.index == self.current_file.file.len() - 1 {
            if self.paths.index == self.paths.data.len() - 1 {
                return;
            }

            self.paths.index += 1;
            self.current_file.file = FileContainer::open(&self.paths.data[self.paths.index]);
            self.current_file.index = 0;
        } else {
            self.current_file.index += 1;
        }
    }

    pub fn previous_image(&mut self) {
        if self.current_file.index == 0 {
            if self.paths.index == 0 {
                return;
            }

            self.paths.index -= 1;
            self.current_file.file = FileContainer::open(&self.paths.data[self.paths.index]);
            self.current_file.index = self.current_file.file.len() - 1;
        } else {
            self.current_file.index -= 1;
        }
    }

    pub fn next_file(&mut self) {
        if self.paths.index == 0 {
            return;
        }

        self.paths.index -= 1;
        self.current_file.file = FileContainer::open(&self.paths.data[self.paths.index]);
        self.current_file.index = 0;
    }

    pub fn previous_file(&mut self) {
        if self.paths.index == self.paths.data.len() - 1 {
            return;
        }

        self.paths.index += 1;
        self.current_file.file = FileContainer::open(&self.paths.data[self.paths.index]);
        self.current_file.index = 0;
    }

    pub fn toggle_progress_display(&mut self) {
        self.show_progress = !self.show_progress;
    }
}

pub fn render_frame(
    screen_width: i32,
    screen_height: i32,
    state: &mut Screen,
    canvas: &skia_safe::Canvas,
) {
    let image_bytes = state.current_image_bytes();

    let mut c = Cursor::new(&image_bytes);

    let mut codec = if image_bytes.starts_with(b"\xFF\xD8\xFF") {
        jpeg_decoder::decode_stream(&mut c).unwrap()
    } else if image_bytes.starts_with(b"\x89PNG\x0D\x0A\x1A\x0A") {
        png_decoder::decode_stream(&mut c).unwrap()
    } else if image_bytes.len() > b"RIFF\0\0\0\0WEBPVP".len()
        && image_bytes.starts_with(b"RIFF")
        && &image_bytes[8..][..6] == b"WEBPVP"
    {
        webp_decoder::decode_stream(&mut c).unwrap()
    } else {
        panic!("unsupported file type");
    };

    let image = codec.get_image(codec.info(), None).unwrap();
    let info = codec.info();

    let ISize {
        mut width,
        mut height,
    } = info.dimensions();

    // Determine the scaling factor based on the window dimensions
    let scale_x = screen_width as f32 / width as f32;
    let scale_y = screen_height as f32 / height as f32;

    // Use the smaller scaling factor to fit within the window
    let scale = scale_x.min(scale_y);

    width = (width as f32 * scale) as i32;
    height = (height as f32 * scale) as i32;

    let x_offset = (screen_width - width) / 2;
    let y_offset = (screen_height - height) / 2;

    canvas.draw_image_rect_with_sampling_options(
        image,
        None,
        Rect {
            left: x_offset as f32,
            top: y_offset as f32,
            right: (screen_width - x_offset) as f32,
            bottom: (screen_height - y_offset) as f32,
        },
        SamplingOptions {
            max_aniso: 0,
            use_cubic: false,
            cubic: skia_safe::CubicResampler { b: 0.0, c: 0.0 },
            filter: skia_safe::FilterMode::Linear,
            mipmap: skia_safe::MipmapMode::None,
        },
        &Paint::default(),
    );

    if state.show_progress {
        render_progress(
            state.current_file.index,
            state.current_file.file.len(),
            canvas,
        );
    }
}

fn render_progress(index: usize, len: usize, canvas: &skia_safe::Canvas) {
    let progress = index * 10 / len; // out of 10

    let mut paint = Paint::default();
    paint.set_color(Color::WHITE);

    const RADIUS: f32 = 16.0;
    const SPACING: f32 = 8.0;

    for i in 0..progress {
        let top_offset = (2.0 * RADIUS + SPACING) * (1 + i / 3) as f32;
        let left_offset = (2.0 * RADIUS + SPACING) * (1 + i % 3) as f32;
        canvas.draw_circle((left_offset, top_offset), RADIUS, &paint);
    }
}
