use webrender::api::ColorF;

mod colors {
    use std::collections::HashMap;

    include!(concat!(env!("OUT_DIR"), "/colors.rs"));

    lazy_static! {
        pub static ref COLOR_MAP: HashMap<&'static str, (u8, u8, u8)> = init_color();
    }
}

pub fn pixel_to_color(pixel: u64) -> ColorF {
    let pixel_array: [u16; 4] = unsafe { std::mem::transmute(pixel) };

    ColorF::new(
        pixel_array[0] as f32 / 255.0, // red
        pixel_array[1] as f32 / 255.0, // green
        pixel_array[2] as f32 / 255.0, // blue
        1.0,
    )
}

pub fn color_to_pixel(color: ColorF) -> u64 {
    let red = color.r * 255.0;
    let green = color.g * 255.0;
    let blue = color.b * 255.0;

    (blue as u64) << 32 | (green as u64) << 16 | (red as u64)
}

pub fn lookup_color_by_name_or_hex(color_string: &str) -> Option<(u8, u8, u8)> {
    // HEX value color, color_string is the hex string.
    if color_string.starts_with('#') {
        let red: u8 = u8::from_str_radix(&color_string[1..3], 16).unwrap();
        let green: u8 = u8::from_str_radix(&color_string[3..5], 16).unwrap();
        let blue: u8 = u8::from_str_radix(&color_string[5..7], 16).unwrap();

        Some((red, green, blue))
    } else {
        // pre-defined color, `color_string` is the color name.
        self::colors::COLOR_MAP
            .get::<str>(&color_string.to_lowercase())
            .copied()
    }
}
