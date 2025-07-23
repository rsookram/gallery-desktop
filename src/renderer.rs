use std::io::Cursor;

use skia_safe::{codec::jpeg_decoder, ISize, Paint, Rect, SamplingOptions};

use crate::file::FileContainer;

pub struct State {
    pub width: i32,
    pub height: i32,
    pub file: FileContainer,
    pub index: usize,
}

pub fn render_frame(state: &mut State, canvas: &skia_safe::Canvas) {
    let image_bytes = state.file.read_at(state.index);

    let mut c = Cursor::new(image_bytes);
    let mut codec = jpeg_decoder::decode_stream(&mut c).unwrap();

    let image = codec.get_image(codec.info(), None).unwrap();
    let info = codec.info();

    let ISize {
        mut width,
        mut height,
    } = info.dimensions();

    // Determine the scaling factor based on the window dimensions
    let scale_x = state.width as f32 / width as f32;
    let scale_y = state.height as f32 / height as f32;

    // Use the smaller scaling factor to fit within the window
    let scale = scale_x.min(scale_y);

    width = (width as f32 * scale) as i32;
    height = (height as f32 * scale) as i32;

    let x_offset = (state.width - width) / 2;
    let y_offset = (state.height - height) / 2;

    canvas.draw_image_rect_with_sampling_options(
        image,
        None,
        Rect {
            left: x_offset as f32,
            top: y_offset as f32,
            right: (state.width - x_offset) as f32,
            bottom: (state.height - y_offset) as f32,
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
}
