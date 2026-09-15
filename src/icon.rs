use tray_icon::Icon;

const SIZE: u32 = 32;

pub fn menu_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    fill_round_rect(&mut rgba, 4, 7, 24, 22, 4, [36, 44, 68, 255]);
    fill_round_rect(&mut rgba, 6, 9, 20, 18, 3, [244, 246, 252, 255]);
    fill_round_rect(&mut rgba, 11, 3, 10, 8, 2, [96, 140, 255, 255]);
    fill_round_rect(&mut rgba, 13, 5, 6, 3, 1, [232, 237, 255, 255]);
    draw_brace_left(&mut rgba);
    draw_brace_right(&mut rgba);
    Ok(Icon::from_rgba(rgba, SIZE, SIZE)?)
}

fn put(rgba: &mut [u8], x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 {
        return;
    }
    let i = ((y as u32 * SIZE + x as u32) * 4) as usize;
    rgba[i] = color[0];
    rgba[i + 1] = color[1];
    rgba[i + 2] = color[2];
    rgba[i + 3] = color[3];
}

fn fill_round_rect(rgba: &mut [u8], x: i32, y: i32, w: i32, h: i32, r: i32, color: [u8; 4]) {
    for py in y..y + h {
        for px in x..x + w {
            if inside_round_rect(px, py, x, y, w, h, r) {
                put(rgba, px, py, color);
            }
        }
    }
}

fn inside_round_rect(px: i32, py: i32, x: i32, y: i32, w: i32, h: i32, r: i32) -> bool {
    let cx = if px < x + r {
        px - (x + r)
    } else if px >= x + w - r {
        px - (x + w - 1 - r)
    } else {
        0
    };
    let cy = if py < y + r {
        py - (y + r)
    } else if py >= y + h - r {
        py - (y + h - 1 - r)
    } else {
        0
    };
    cx * cx + cy * cy <= r * r
}

fn draw_brace_left(rgba: &mut [u8]) {
    let c = [45, 196, 176, 255];
    for y in 14..26 {
        put(rgba, 11, y, c);
    }
    put(rgba, 12, 14, c);
    put(rgba, 12, 25, c);
    put(rgba, 10, 19, c);
    put(rgba, 10, 20, c);
}

fn draw_brace_right(rgba: &mut [u8]) {
    let c = [96, 140, 255, 255];
    for y in 14..26 {
        put(rgba, 20, y, c);
    }
    put(rgba, 19, 14, c);
    put(rgba, 19, 25, c);
    put(rgba, 21, 19, c);
    put(rgba, 21, 20, c);
}

#[cfg(test)]
mod tests {
    use super::menu_icon;

    #[test]
    fn builds_rgba_icon() {
        assert!(menu_icon().is_ok());
    }
}
