use tray_icon::Icon;

const SIZE: u32 = 32;

pub fn menu_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let i = ((y * SIZE + x) * 4) as usize;
            let edge = x < 3 || y < 3 || x >= SIZE - 3 || y >= SIZE - 3;
            let paper = x >= 6 && x < SIZE - 6 && y >= 6 && y < SIZE - 6;
            if edge {
                rgba[i] = 40;
                rgba[i + 1] = 40;
                rgba[i + 2] = 44;
                rgba[i + 3] = 255;
            } else if paper {
                rgba[i] = 245;
                rgba[i + 1] = 245;
                rgba[i + 2] = 247;
                rgba[i + 3] = 255;
            } else {
                rgba[i + 3] = 0;
            }
        }
    }
    // clipboard clip on top
    for x in 12..20 {
        for y in 2..8 {
            let i = ((y * SIZE + x) * 4) as usize;
            rgba[i] = 90;
            rgba[i + 1] = 90;
            rgba[i + 2] = 96;
            rgba[i + 3] = 255;
        }
    }
    Ok(Icon::from_rgba(rgba, SIZE, SIZE)?)
}
