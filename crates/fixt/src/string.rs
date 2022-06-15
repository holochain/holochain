use crate::prelude::*;
use rand::Rng;

pub const EMPTY_CHAR: char = '\u{0000}';
pub const PREDICTABLE_CHARS: &str = "💯❤💩.!foobarbaz!.💩❤💯";

fixturator!(char, EMPTY_CHAR, crate::rng().gen(), {
    let mut index = get_fixt_index!();
    let ret = PREDICTABLE_CHARS
        .chars()
        .nth(index % PREDICTABLE_CHARS.chars().count())
        .unwrap();
    index += 1;
    set_fixt_index!(index);
    ret
});

#[cfg(test)]
basic_test!(
    char,
    vec![EMPTY_CHAR; 40],
    PREDICTABLE_CHARS
        .chars()
        .cycle()
        .take(40)
        .collect::<Vec<char>>()
);

pub const EMPTY_STR: &str = "";
pub const PREDICTABLE_STRS: [&str; 10] =
    ["💯", "❤", "💩", ".", "!", "foo", "bar", "baz", "bing", "!"];
pub const UNPREDICTABLE_MIN_LEN: usize = 0;
pub const UNPREDICTABLE_MAX_LEN: usize = 64;

fixturator!(
    String,
    String::from(EMPTY_STR),
    {
        let mut rng = crate::rng();
        let len = rng.gen_range(UNPREDICTABLE_MIN_LEN..UNPREDICTABLE_MAX_LEN);
        let vec: Vec<char> = (0..len).map(|_| rng.gen()).collect();
        let string: String = vec.iter().collect();
        string
    },
    {
        let mut index = get_fixt_index!();
        let ret = PREDICTABLE_STRS
            .iter()
            .cycle()
            .nth(index)
            .unwrap()
            .to_string();
        index += 1;
        set_fixt_index!(index);
        ret
    }
);

#[cfg(test)]
basic_test!(
    String,
    vec![String::from(EMPTY_STR); 40],
    PREDICTABLE_STRS
        .iter()
        .map(|s| s.to_string())
        .cycle()
        .take(40)
        .collect::<Vec<String>>()
);
