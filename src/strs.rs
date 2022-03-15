use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

const UNLIM: usize = -1i64 as usize;

pub fn cut(s: &str, start: usize, len: usize) -> String {
    let w= s.width();
    if start == 0 && w<= len {
        return s.to_string();
    }
    if len == 0 {
        return String::new();
    }
    let mut first: usize = UNLIM;
    let mut end: usize = UNLIM;
    let mut curr_width: usize = 0;
    for (bidx, c) in s.char_indices().skip(start) {
        let w = c.width().unwrap_or(0);
        if w + curr_width > len {
            end = bidx;
            break;
        }
        if first == UNLIM {
            first = bidx;
        }
        if len == UNLIM {
            break;
        }
        curr_width += w;
    }
    if first == UNLIM {
        return String::new();
    }
    if end == UNLIM {
        return s.get(first..).unwrap().to_string()
    }
    s.get(first..end).unwrap().to_string()
}

pub fn center(s: &str, width: usize) -> String {
    let w = s.width();
    if w == width {
        return s.to_string();
    }
    if w < width {
        let half = (width - w) / 2;
        return " ".repeat(half) + s + &(" ".repeat(width - w - half));
    }
    let mut skip = (w - width) / 2;
    let mut first: usize = UNLIM;
    let mut end: usize = UNLIM;
    let mut curr_width: usize = 0;
    for (bidx, c) in s.char_indices() {
        let wc = c.width().unwrap_or(0);
        if first == UNLIM {
            if skip < wc {
                first = bidx;
            } else {
                skip -= wc;
            }
            continue;
        }
        if curr_width+wc > width {
            break;
        }
        end = bidx;
        curr_width += wc;
    }
    if first == UNLIM {
        return String::new();
    }
    if end == UNLIM {
        return s.get(first..).unwrap().to_string()
    }
    s.get(first..end).unwrap().to_string()
}

pub fn right(s: &str, width: usize) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut w = s.width();
    if w < width {
        return " ".repeat(width-w) + s;
    }
    let mut first: usize = UNLIM;
    w = 0;
    for (bidx, c) in s.char_indices().rev() {
        let wc = c.width().unwrap_or(0);
        if w + wc > width {
            break;
        }
        w += wc;
        first = bidx;
    }
    if first == UNLIM {
        return String::new();
    }
    s.get(first..).unwrap().to_string()
}

pub fn pad(s: &str, width: usize) -> String {
    let w = s.width();
    if w >= width {
        return s.to_string();
    }
    s.to_string() + &" ".repeat(width-w)
}

#[rustfmt::skip]
#[cfg(test)]
mod str_test {
    use super::*;

    #[test]
    fn cut_utf_test() {
        let s = "ʃʤaíckəʊkʌɪg";
        let c = cut(s, 0, UNLIM);
        assert_eq!(c, s.to_string());
        let c = cut(s, 2, UNLIM);
        assert_eq!(c, "aíckəʊkʌɪg".to_string());
        let c = cut(s, UNLIM, UNLIM);
        assert_eq!(c, "".to_string());
        let c = cut(s, 4, 30);
        assert_eq!(c, "ckəʊkʌɪg".to_string());
        let c = cut(s, 3, 5);
        assert_eq!(c, "íckəʊ".to_string());
        let c = cut(s, 13, 4);
        assert_eq!(c, "".to_string());
    }

    #[test]
    fn center_utf_test() {
        let s = "ʃʤaíckəʊkʌɪg";
        let c = center(s, 12);
        assert_eq!(c, s.to_string());
        let c = center(s, 10);
        assert_eq!(c, "ʤaíckəʊkʌɪ".to_string());
        let c = center(s, 9);
        assert_eq!(c, "ʤaíckəʊkʌ".to_string());
        let c = center(s, 13);
        assert_eq!(c, "ʃʤaíckəʊkʌɪg ".to_string());
        let c = center(s, 16);
        assert_eq!(c, "  ʃʤaíckəʊkʌɪg  ".to_string());
    }

    #[test]
    fn right_utf_test() {
        let s = "ʃʤaíckəʊkʌɪg";
        let c = right(s, 12);
        assert_eq!(c, s.to_string());
        let c = right(s, 10);
        assert_eq!(c, "aíckəʊkʌɪg".to_string());
        let c = right(s, 9);
        assert_eq!(c, "íckəʊkʌɪg".to_string());
        let c = right(s, 13);
        assert_eq!(c, " ʃʤaíckəʊkʌɪg".to_string());
        let c = right(s, 16);
        assert_eq!(c, "    ʃʤaíckəʊkʌɪg".to_string());
    }

    #[test]
    fn pad_utf_test() {
        let s = "ʃʤaíckəʊkʌɪg";
        let c = pad(s, 12);
        assert_eq!(c, s.to_string());
        let c = pad(s, 10);
        assert_eq!(c, s.to_string());
        let c = pad(s, 13);
        assert_eq!(c, "ʃʤaíckəʊkʌɪg ".to_string());
        let c = pad(s, 16);
        assert_eq!(c, "ʃʤaíckəʊkʌɪg    ".to_string());
    }
}
