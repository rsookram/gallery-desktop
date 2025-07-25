use std::io::Cursor;

use skia_safe::{
    codec::{jpeg_decoder, png_decoder, webp_decoder},
    Color, ISize, Paint, Rect, SamplingOptions,
};

use crate::state::State;

pub fn render_frame(state: &mut State, canvas: &skia_safe::Canvas) {
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
