use crate::prelude::*;
use rand::Rng;

const EMPTY_CHAR: char = '\u{0000}';
const PREDICTABLE_CHARS: &str = "💯❤💩.!foobarbaz!.💩❤💯";

fixturator!(char, EMPTY_CHAR, rand::random(), {
    let ret = PREDICTABLE_CHARS
        .chars()
        .nth(self.0.index % PREDICTABLE_CHARS.chars().count())
        .unwrap();
    self.0.index = self.0.index + 1;
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

const EMPTY_STR: &str = "";
const PREDICTABLE_STRS: [&str; 10] = ["💯", "❤", "💩", ".", "!", "foo", "bar", "baz", "bing", "!"];
const UNPREDICTABLE_MIN_LEN: usize = 0;
const UNPREDICTABLE_MAX_LEN: usize = 64;

fixturator!(
    String,
    String::from(EMPTY_STR),
    {
        let mut rng = rand::thread_rng();
        let len = rng.gen_range(UNPREDICTABLE_MIN_LEN, UNPREDICTABLE_MAX_LEN);
        let vec: Vec<char> = (0..len).map(|_| rng.gen()).collect();
        let string: String = vec.iter().collect();
        string
    },
    {
        let ret = PREDICTABLE_STRS
            .iter()
            .cycle()
            .nth(self.0.index)
            .unwrap()
            .to_string();
        self.0.index = self.0.index + 1;
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
