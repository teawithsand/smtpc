#[inline]
pub fn is_white_space(c: char) -> bool {
    c == ' ' || c == '\t'
}

#[inline]
pub fn is_qtext(c: char) -> bool {
    match c {
        '"' | '\\' => false,
        _ => is_vchar(c)
    }
}

#[inline]
pub fn is_vchar(c: char) -> bool {
    match c {
        '!'..='~' => true,
        _ => c.len_utf8() > 1
    }
}

#[inline]
pub fn is_atext(c: char, dot: bool, permissive: bool) -> bool {
    match c {
        '.' => dot,
        '(' | ')' | '[' | ']' | ';' | '@' | '\\' | ',' => permissive,
        '<' | '>' | '"' | ':' => false,
        c => is_vchar(c)
    }
}