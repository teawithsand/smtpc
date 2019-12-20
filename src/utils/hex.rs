#[inline]
pub(crate) fn encode_hex_char(b: u8) -> [u8; 2] {
    let mut first = b % 16;
    let mut second = (b / 16) % 16;
    if first < 10 {
        first += b'0';
    } else {
        first += b'A' - 10;
    }

    if second < 10 {
        second += b'0';
    } else {
        second += b'A' - 10;
    }

    [second, first]
}

#[cfg(test)]
mod test {
    use crate::utils::hex::encode_hex_char;

    const HEX_LOOKUP_TABLE: [&[u8; 2]; 256] = [
        b"00", b"01", b"02", b"03", b"04", b"05", b"06", b"07", b"08", b"09",
        b"0A", b"0B", b"0C", b"0D", b"0E", b"0F", b"10", b"11", b"12", b"13", b"14",
        b"15", b"16", b"17", b"18", b"19", b"1A", b"1B", b"1C", b"1D", b"1E",
        b"1F", b"20", b"21", b"22", b"23", b"24", b"25", b"26", b"27",
        b"28", b"29", b"2A", b"2B", b"2C", b"2D", b"2E", b"2F", b"30",
        b"31", b"32", b"33", b"34", b"35", b"36", b"37", b"38", b"39",
        b"3A", b"3B", b"3C", b"3D", b"3E", b"3F", b"40", b"41", b"42",
        b"43", b"44", b"45", b"46", b"47", b"48", b"49", b"4A", b"4B",
        b"4C", b"4D", b"4E", b"4F", b"50", b"51", b"52", b"53", b"54",
        b"55", b"56", b"57", b"58", b"59", b"5A", b"5B", b"5C", b"5D",
        b"5E", b"5F", b"60", b"61", b"62", b"63", b"64", b"65", b"66",
        b"67", b"68", b"69", b"6A", b"6B", b"6C", b"6D", b"6E", b"6F",
        b"70", b"71", b"72", b"73", b"74", b"75", b"76", b"77", b"78",
        b"79", b"7A", b"7B", b"7C", b"7D", b"7E", b"7F", b"80", b"81",
        b"82", b"83", b"84", b"85", b"86", b"87", b"88", b"89", b"8A",
        b"8B", b"8C", b"8D", b"8E", b"8F", b"90", b"91", b"92", b"93",
        b"94", b"95", b"96", b"97", b"98", b"99", b"9A", b"9B", b"9C",
        b"9D", b"9E", b"9F", b"A0", b"A1", b"A2", b"A3", b"A4", b"A5",
        b"A6", b"A7", b"A8", b"A9", b"AA", b"AB", b"AC", b"AD", b"AE",
        b"AF", b"B0", b"B1", b"B2", b"B3", b"B4", b"B5", b"B6", b"B7",
        b"B8", b"B9", b"BA", b"BB", b"BC", b"BD", b"BE", b"BF", b"C0",
        b"C1", b"C2", b"C3", b"C4", b"C5", b"C6", b"C7", b"C8", b"C9",
        b"CA", b"CB", b"CC", b"CD", b"CE", b"CF", b"D0", b"D1", b"D2",
        b"D3", b"D4", b"D5", b"D6", b"D7", b"D8", b"D9", b"DA", b"DB",
        b"DC", b"DD", b"DE", b"DF", b"E0", b"E1", b"E2", b"E3", b"E4",
        b"E5", b"E6", b"E7", b"E8", b"E9", b"EA", b"EB", b"EC", b"ED",
        b"EE", b"EF", b"F0", b"F1", b"F2", b"F3", b"F4", b"F5", b"F6",
        b"F7", b"F8", b"F9", b"FA", b"FB", b"FC", b"FD", b"FE", b"FF"
    ];

    #[test]
    fn test_can_encode_any_byte() {
        for i in 0..255u8 {
            assert_eq!(&encode_hex_char(i)[..], &HEX_LOOKUP_TABLE[i as usize][..]);
        }
    }
}