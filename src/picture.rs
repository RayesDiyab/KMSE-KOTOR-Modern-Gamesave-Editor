//! A picture for a save that has none: the load screen of the module it loads in.
//!
//! The game writes `Screen.tga` into every save it makes: a 256 by 256, 24-bit, uncompressed
//! TGA, bottom row first, 196,626 bytes (measured 2026-11-08 in KotOR and KotOR II saves,
//! Steam, macOS and Windows). A save made by a tool from a save without one (KMRP's test
//! saves) has none, and its Load screen and the editor's list show an empty picture. The
//! game shows each module's own load screen while it loads, a texture named
//! `load_<module>` (`load_end_m01aa`, `load_203TEL`, in TexturePacks/swpc_tex_gui.erf or
//! the archives), else `load_default`. An area's `LoadScreenID` was 0 (`LOAD_DEFAULT` in
//! loadscreens.2da) in every area read, both games, so it is not used. The picture is that
//! texture made 256 by 256 as the game's own are (a 4:3 screen in a square).

use crate::formats::tga::Image;
use crate::gamedata::GameData;

/// The game's size for Screen.tga.
pub const SIZE: u32 = 256;

/// The load screen of `module` as the game shows it: `load_<module>`, else `load_default`.
pub fn load_screen(data: &GameData, module: &str) -> Option<Image> {
    if !crate::map::is_module_name(module) {
        return None;
    }
    let decode = |name: &str| {
        let (bytes, tpc) = data.texture(name)?;
        if tpc {
            crate::formats::tpc::decode(&bytes)
        } else {
            crate::formats::tga::decode(&bytes)
        }
    };
    decode(&format!("load_{}", module)).or_else(|| decode("load_default"))
}

/// `img` made `w` by `h`: each new pixel the average of the pixels it covers (a box filter),
/// so a 1024 or 512 pixel picture comes down without the stairs of taking every fourth pixel.
pub fn resize(img: &Image, w: u32, h: u32) -> Image {
    let (sw, sh) = (img.width.max(1) as u64, img.height.max(1) as u64);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h as u64 {
        let (y0, y1) = (y * sh / h as u64, ((y + 1) * sh / h as u64).max(y * sh / h as u64 + 1).min(sh));
        for x in 0..w as u64 {
            let (x0, x1) = (x * sw / w as u64, ((x + 1) * sw / w as u64).max(x * sw / w as u64 + 1).min(sw));
            let mut sum = [0u64; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let at = ((sy * sw + sx) * 4) as usize;
                    for c in 0..4 {
                        sum[c] += *img.rgba.get(at + c).unwrap_or(&0) as u64;
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)).max(1);
            let at = ((y * w as u64 + x) * 4) as usize;
            for c in 0..4 {
                rgba[at + c] = (sum[c] / n) as u8;
            }
        }
    }
    Image { width: w, height: h, rgba }
}

/// `img` as the game writes Screen.tga: uncompressed truecolour (type 2), 24 bits, blue green
/// red, bottom row first, no image id. Alpha is dropped, onto black.
pub fn screen_tga(img: &Image) -> Vec<u8> {
    let (w, h) = (img.width as usize, img.height as usize);
    let mut out = Vec::with_capacity(18 + w * h * 3);
    out.extend_from_slice(&[0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(&(w as u16).to_le_bytes());
    out.extend_from_slice(&(h as u16).to_le_bytes());
    out.extend_from_slice(&[24, 0]);
    for y in (0..h).rev() {
        for x in 0..w {
            let p = &img.rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
            let a = p[3] as u16;
            let on_black = |c: u8| ((c as u16 * a + 127) / 255) as u8;
            out.extend_from_slice(&[on_black(p[2]), on_black(p[1]), on_black(p[0])]);
        }
    }
    out
}

/// The Screen.tga to give a save whose next load is in `module`: its load screen, 256 by 256.
pub fn for_module(data: &GameData, module: &str) -> Option<Vec<u8>> {
    let img = load_screen(data, module)?;
    Some(screen_tga(&resize(&img, SIZE, SIZE)))
}
