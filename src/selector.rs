use std::{
    fs::File,
    io::{Cursor, Read, Seek},
    path::{Path, PathBuf},
    thread,
};

use skia_safe::{
    codec::{jpeg_decoder, png_decoder, webp_decoder},
    ISize, Image, ImageInfo, Paint, Rect, SamplingOptions,
};

pub const NUM_COLUMNS: i32 = 4;
pub const NUM_ROWS: i32 = 3;

pub struct Screen {
    pub ofcs: Vec<Ofc>,
    pub page_index: usize,
}

impl Screen {
    pub fn new(paths: Vec<PathBuf>) -> Self {
        Self {
            ofcs: paths
                .into_iter()
                .map(|path| Ofc {
                    path,
                    selected: false,
                })
                .collect(),
            page_index: 0,
        }
    }

    pub fn previous_page(&mut self) {
        self.page_index = self.page_index.saturating_sub(1);
    }

    pub fn next_page(&mut self) {
        self.page_index = (self.page_index + 1).min(self.page_count() - 1);
    }

    fn current_page(&self) -> &[Ofc] {
        let start = &self.ofcs[self.page_index * Self::page_size()..];
        &start[..Self::page_size().min(start.len())]
    }

    fn page_count(&self) -> usize {
        self.ofcs.len().div_ceil(Self::page_size())
    }

    fn page_size() -> usize {
        usize::try_from(NUM_COLUMNS * NUM_ROWS).unwrap()
    }

    pub fn on_click(&mut self, x: f64, y: f64, width: i32, height: i32) {
        let col = x / width as f64 * NUM_COLUMNS as f64;
        let col = col.floor() as usize;

        let row = y / height as f64 * NUM_ROWS as f64;
        let row = row.floor() as usize;

        let index_in_page = row * usize::try_from(NUM_COLUMNS).unwrap() + col;
        let index =
            usize::try_from(NUM_ROWS * NUM_COLUMNS).unwrap() * self.page_index + index_in_page;

        if let Some(ofc) = self.ofcs.get_mut(index) {
            ofc.selected = !ofc.selected;
        }
    }
}

pub struct Ofc {
    pub path: PathBuf,
    pub selected: bool,
}

const SAMPLING_OPTIONS: SamplingOptions = SamplingOptions {
    max_aniso: 0,
    use_cubic: false,
    cubic: skia_safe::CubicResampler { b: 0.0, c: 0.0 },
    filter: skia_safe::FilterMode::Linear,
    mipmap: skia_safe::MipmapMode::Linear,
};

pub fn render_frame(
    screen_width: i32,
    screen_height: i32,
    state: &mut Screen,
    canvas: &skia_safe::Canvas,
) {
    let max_width: i32 = screen_width / NUM_COLUMNS;
    let max_height: i32 = screen_height / NUM_ROWS;

    let ofcs = state.current_page();
    let decoded_images = decode_images(ofcs);

    let mut paint = Paint::default();
    paint.set_color(0xAA000000);

    for (i, decoded_image) in decoded_images.into_iter().enumerate() {
        let i = i32::try_from(i).unwrap();
        let x_offset = (i % NUM_COLUMNS) * max_width;
        let y_offset = (i / NUM_COLUMNS) * max_height;

        draw_cover(
            canvas,
            decoded_image,
            x_offset,
            y_offset,
            max_width,
            max_height,
        );

        if ofcs[usize::try_from(i).unwrap()].selected {
            canvas.draw_rect(
                Rect {
                    left: x_offset as f32,
                    top: y_offset as f32,
                    right: (x_offset + max_width) as f32,
                    bottom: (y_offset + max_height) as f32,
                },
                &paint,
            );
        }
    }
}

fn decode_images(ofcs: &[Ofc]) -> Vec<DecodedImage> {
    thread::scope(|s| {
        let handles: Vec<_> = ofcs
            .iter()
            .map(|ofc| {
                s.spawn(|| {
                    let image_bytes = load_image_bytes(&ofc.path);

                    decode_image(&image_bytes)
                })
            })
            .collect();

        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    })
}

fn draw_cover(
    canvas: &skia_safe::Canvas,
    decoded_image: DecodedImage,
    mut x_offset: i32,
    mut y_offset: i32,
    max_width: i32,
    max_height: i32,
) {
    let DecodedImage { image, info } = decoded_image;

    let (width, height) = scale_to_fit(&info, max_width, max_height);

    x_offset += (max_width - width) / 2;
    y_offset += (max_height - height) / 2;

    canvas.draw_image_rect_with_sampling_options(
        image,
        None,
        Rect {
            left: x_offset as f32,
            top: y_offset as f32,
            right: (x_offset + width) as f32,
            bottom: (y_offset + height) as f32,
        },
        SAMPLING_OPTIONS,
        &Paint::default(),
    );
}

struct DecodedImage {
    image: Image,
    info: ImageInfo,
}

fn decode_image(bytes: &[u8]) -> DecodedImage {
    let mut c = Cursor::new(&bytes);

    let mut codec = if bytes.starts_with(b"\xFF\xD8\xFF") {
        jpeg_decoder::decode_stream(&mut c).unwrap()
    } else if bytes.starts_with(b"\x89PNG\x0D\x0A\x1A\x0A") {
        png_decoder::decode_stream(&mut c).unwrap()
    } else if bytes.len() > b"RIFF\0\0\0\0WEBPVP".len()
        && bytes.starts_with(b"RIFF")
        && &bytes[8..][..6] == b"WEBPVP"
    {
        webp_decoder::decode_stream(&mut c).unwrap()
    } else {
        panic!("unsupported file type");
    };

    DecodedImage {
        image: codec.get_image(codec.info(), None).unwrap(),
        info: codec.info(),
    }
}

fn scale_to_fit(info: &ImageInfo, max_width: i32, max_height: i32) -> (i32, i32) {
    let ISize {
        mut width,
        mut height,
    } = info.dimensions();

    // Determine the scaling factor based on the window dimensions
    let scale_x = max_width as f32 / width as f32;
    let scale_y = max_height as f32 / height as f32;

    // Use the smaller scaling factor to fit within the window
    let scale = scale_x.min(scale_y);

    width = (width as f32 * scale) as i32;
    height = (height as f32 * scale) as i32;

    (width, height)
}

fn load_image_bytes(p: &Path) -> Vec<u8> {
    let mut f = File::open(p).unwrap();

    let mut buf = [0u8; 16];
    f.read_exact(&mut buf).unwrap();

    assert_eq!(&buf[..4], b"ofc\0");

    let num_files = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);

    let end_offset = u64::from_le_bytes([
        buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
    ]);

    f.seek(std::io::SeekFrom::Start(8 + 8 * u64::from(num_files)))
        .unwrap();

    let mut buf = vec![0u8; usize::try_from(end_offset).unwrap()];
    f.read_exact(&mut buf).unwrap();

    buf
}
