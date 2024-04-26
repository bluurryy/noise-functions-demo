use image::{codecs::png::PngEncoder, ImageBuffer, Pixel, Rgb};
use noise_functions::*;

fn noise_to_image(
    s: impl Sample2,
    width: usize,
    height: usize,
) -> ImageBuffer<Rgb<u8>, Vec<<Rgb<u8> as Pixel>::Subpixel>> {
    let mut image = ImageBuffer::new(width as u32, height as u32);
    let scalar = 1.0 / width.max(height) as f32;
    let scalar_times_2 = scalar * 2.0;

    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let x = x as f32 * scalar_times_2 - 1.0;
        let y = y as f32 * scalar_times_2 - 1.0;
        let value = s.sample2([x, y]);
        let value = ((value * 0.5 + 0.5) * 255.0) as u8;
        *pixel = Rgb([value, value, value]);
    }

    image
}

fn png(image: ImageBuffer<Rgb<u8>, Vec<<Rgb<u8> as Pixel>::Subpixel>>) -> Vec<u8> {
    let mut vec = Vec::<u8>::new();
    let encoder = PngEncoder::new_with_quality(
        &mut vec,
        image::codecs::png::CompressionType::Best,
        image::codecs::png::FilterType::Adaptive,
    );
    image.write_with_encoder(encoder).unwrap();
    vec
}

fn main() {
    let ico_noise = OpenSimplex2;
    let png_noise = OpenSimplex2.frequency(3.0);

    let path = |file: &str| format!("assets/{file}");

    let create_ico = |file: &str, size: usize| {
        noise_to_image(&ico_noise, size, size)
            .save(path(file))
            .unwrap()
    };

    let create_png = |file: &str, size: usize| {
        let image = noise_to_image(&png_noise, size, size);
        std::fs::write(path(file), png(image)).unwrap()
    };

    create_ico("favicon.ico", 48);
    create_png("icon_ios_touch_192.png", 192);
    create_png("icon-256.png", 256);
    create_png("icon-1024.png", 1024);
    create_png("maskable_icon_x512.png", 512);
}
