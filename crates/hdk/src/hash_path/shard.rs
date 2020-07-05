use crate::hash_path::path::Component;
use crate::hash_path::path::Path;

pub type ShardWidth = u32;
pub type ShardDepth = u32;

pub struct ShardStrategy(ShardWidth, ShardDepth);

impl ShardStrategy {
    fn width(&self) -> ShardWidth {
        self.0
    }

    fn depth(&self) -> ShardDepth {
        self.1
    }
}

impl From<(&ShardStrategy, &str)> for Path {
    fn from((strategy, s): (&ShardStrategy, &str)) -> Path {
        let full_length = strategy.width() * strategy.depth();

        let shard_string: String = s.chars().take(full_length as _).collect();

        // defer to the standard utf32 string handling to get the fixed size byte and endian
        // handling correct
        let intermediate_component = Component::from(&shard_string);

        let sharded_component: Vec<Component> = intermediate_component
            .as_ref()
            .iter()
            .fold((vec![], vec![]), |acc, c| {
                let (mut ret, mut build) = acc;
                build.push(c);
                // relies on the fact that we're encoding string characters as fixed width u32 bytes
                // rather than variable width utf8 bytes
                if build.len() == strategy.width() as usize * std::mem::size_of::<u32>() {
                    ret.push(build.clone());
                    build.clear();
                }
                (ret, build)
            })
            .0
            .iter()
            .map(|bytes| {
                let bytes_vec: Vec<u8> = bytes.iter().map(|b| **b).collect();
                Component::from(bytes_vec)
            })
            .collect();

        Path::from(sharded_component)
    }
}

#[test]
#[cfg(test)]
fn hash_path_shard_string() {
    for (width, depth, s, output) in vec![
        // anything with a zero results in an empty path
        (0, 0, "foobar", Path::from("")),
        (0, 1, "foobar", Path::from("")),
        (1, 0, "foobar", Path::from("")),
        (0, 2, "foobar", Path::from("")),
        (2, 0, "foobar", Path::from("")),
        // basic sharding behaviour
        (1, 1, "foobar", Path::from("f")),
        (2, 1, "foobar", Path::from("fo")),
        (1, 2, "foobar", Path::from("f/o")),
        (2, 2, "foobar", Path::from("fo/ob")),
        // multibyte characters should be handled the way a naive understanding of strings would
        // expect, i.e. that a 2-byte utf8 character is represented as 1 4-byte utf32 character and
        // so counts as 1 "width" and 1 "depth" for the purpose of sharding
        (2, 2, "€€€€", Path::from("€€/€€")),
        // if the string is shorter than the width and depth we go as deep as we can cleanly and
        // truncate the end
        (4, 4, "foobar", Path::from("foob")),
        (4, 4, "foobarbaz", Path::from("foob/arba")),
        (4, 4, "€€€€€€€€€", Path::from("€€€€/€€€€")),
    ] {
        assert_eq!(output, Path::from((&ShardStrategy(width, depth), s)));
    }
}
