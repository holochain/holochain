use crate::prelude::*;
use rand::Rng;

const EMPTY_CHAR: char = '\u{0000}';
const PREDICTABLE_CHARS: &str = "💯❤💩.!foobarbaz!.💩❤💯";

impl Iterator for Fixturator<char, Empty> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        Some(EMPTY_CHAR)
    }
}

impl Iterator for Fixturator<char, Unpredictable> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        Some(rand::random())
    }
}

impl Iterator for Fixturator<char, Predictable> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = PREDICTABLE_CHARS
            .chars()
            .nth(self.index % PREDICTABLE_CHARS.chars().count());
        self.index = self.index + 1;
        ret
    }
}

impl Fixt for char {}

const EMPTY_STR: &str = "";
const PREDICTABLE_STRS: [&str; 10] = ["💯", "❤", "💩", ".", "!", "foo", "bar", "baz", "bing", "!"];

impl Iterator for Fixturator<String, Empty> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        Some(String::from(EMPTY_STR))
    }
}

impl Iterator for Fixturator<String, Unpredictable> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let mut rng = rand::thread_rng();
        let len = rng.gen_range(0, 64);
        let vec: Vec<char> = (0..len).map(|_| rng.gen()).collect();
        let string: String = vec.into_iter().collect();
        Some(string)
    }
}

impl Iterator for Fixturator<String, Predictable> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let ret: &str = PREDICTABLE_STRS[self.index % PREDICTABLE_STRS.len()];
        self.index = self.index + 1;
        Some(String::from(ret))
    }
}

impl Fixt for String {}

#[cfg(test)]
mod tests {
    use super::*;

    basic_test!(
        char,
        EMPTY_CHAR,
        PREDICTABLE_CHARS.chars().count() * 2,
        vec![
            '💯', '❤', '💩', '.', '!', 'f', 'o', 'o', 'b', 'a', 'r', 'b', 'a', 'z', '!', '.', '💩',
            '❤', '💯', '💯', '❤', '💩', '.', '!', 'f', 'o', 'o', 'b', 'a', 'r', 'b', 'a', 'z', '!',
            '.', '💩', '❤', '💯'
        ]
    );

    basic_test!(
        String,
        String::from(EMPTY_STR),
        PREDICTABLE_STRS.len() * 2,
        vec![
            "💯", "❤", "💩", ".", "!", "foo", "bar", "baz", "bing", "!", "💯", "❤", "💩", ".", "!",
            "foo", "bar", "baz", "bing", "!"
        ]
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<String>>()
    );
}
