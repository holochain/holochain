use crate::hash_path::path::Component;
use crate::hash_path::path::Path;
use std::str::FromStr;

pub const SHARDSPLIT: &str = ":";
pub const SHARDEND: &str = "#";

pub type ShardWidth = u32;
pub type ShardDepth = u32;

#[derive(Debug)]
pub struct ShardStrategy(ShardWidth, ShardDepth);

impl ShardStrategy {
    fn width(&self) -> ShardWidth {
        self.0
    }

    fn depth(&self) -> ShardDepth {
        self.1
    }
}

#[derive(Debug)]
pub enum ParseShardStrategyError {
    BadDepth,
    BadWidth,
    ShardSplitNotFound,
    ShardEndNotFound,
    FirstCharNotADigit,
    EmptyString,
}

impl FromStr for ShardStrategy {
    type Err = ParseShardStrategyError;

    /// a shard strategy is parsed as "width:depth#..." at the start of a string
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // the first char needs to be a digit
        match s.chars().nth(0) {
            Some(first_char) => {
                match u32::from_str(&first_char.to_string()) {
                    Ok(_) => {
                        // there needs to be a #
                        match s.find(SHARDEND) {
                            Some(end_index) => {
                                let (maybe_strategy, _) = s.split_at(end_index);
                                match maybe_strategy.find(SHARDSPLIT) {
                                    Some(split_index) => {
                                        let (maybe_width, maybe_depth) =
                                            maybe_strategy.split_at(split_index);
                                        match u32::from_str(maybe_width) {
                                            Ok(width) => {
                                                match u32::from_str(
                                                    maybe_depth.trim_start_matches(SHARDSPLIT),
                                                ) {
                                                    Ok(depth) => Ok(ShardStrategy(width, depth)),
                                                    Err(_) => {
                                                        Err(ParseShardStrategyError::BadDepth)
                                                    }
                                                }
                                            }
                                            Err(_) => Err(ParseShardStrategyError::BadWidth),
                                        }
                                    }
                                    None => Err(ParseShardStrategyError::ShardSplitNotFound),
                                }
                            }
                            None => Err(ParseShardStrategyError::ShardEndNotFound),
                        }
                    }
                    Err(_) => Err(ParseShardStrategyError::FirstCharNotADigit),
                }
            }
            None => Err(ParseShardStrategyError::EmptyString),
        }
    }
}

impl From<(&ShardStrategy, &[u8])> for Path {
    fn from((strategy, bytes): (&ShardStrategy, &[u8])) -> Path {
        let full_length = strategy.width() * strategy.depth();
        // fold a flat slice of bytes into `strategy.depth` number of `strategy.width` length byte
        // Components
        let sharded: Vec<Component> = bytes
            .iter()
            .take(full_length as _)
            .fold((vec![], vec![]), |acc, b| {
                let (mut ret, mut build) = acc;
                build.push(b);
                if build.len() == strategy.width() as usize {
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
        Path::from(sharded)
    }
}
impl From<(&ShardStrategy, &Vec<u8>)> for Path {
    fn from((strategy, bytes): (&ShardStrategy, &Vec<u8>)) -> Path {
        let bytes: &[u8] = bytes.as_ref();
        Path::from((strategy, bytes))
    }
}
impl From<(&ShardStrategy, Vec<u8>)> for Path {
    fn from((strategy, bytes): (&ShardStrategy, Vec<u8>)) -> Path {
        let bytes: &[u8] = bytes.as_ref();
        Path::from((strategy, bytes))
    }
}

impl From<(&ShardStrategy, &str)> for Path {
    fn from((strategy, s): (&ShardStrategy, &str)) -> Path {
        // truncate the string to only relevant chars
        let full_length = strategy.width() * strategy.depth();
        let shard_string: String = s.chars().take(full_length as _).collect();

        Path::from((
            &ShardStrategy(
                // relies on the fact that we're encoding string characters as fixed width u32
                // bytes rather than variable width utf8 bytes
                strategy.width() * std::mem::size_of::<u32>() as u32,
                strategy.depth(),
            ),
            // defer to the standard utf32 string handling to get the fixed size byte and endian
            // handling correct
            Component::from(&shard_string).as_ref(),
        ))
    }
}
impl From<(&ShardStrategy, &String)> for Path {
    fn from((strategy, s): (&ShardStrategy, &String)) -> Path {
        Path::from((strategy, s.as_str()))
    }
}
impl From<(&ShardStrategy, String)> for Path {
    fn from((strategy, s): (&ShardStrategy, String)) -> Path {
        Path::from((strategy, s.as_str()))
    }
}

#[test]
#[cfg(test)]
fn hash_path_shard_bytes() {
    for (width, depth, b, output) in vec![
        // anything with a zero results in an empty path
        (0, 0, vec![1, 2, 3, 4, 5], Path::from(vec![])),
        (0, 1, vec![1, 2, 3, 4, 5], Path::from(vec![])),
        (1, 0, vec![1, 2, 3, 4, 5], Path::from(vec![])),
        (0, 2, vec![1, 2, 3, 4, 5], Path::from(vec![])),
        (2, 0, vec![1, 2, 3, 4, 5], Path::from(vec![])),
        // basic sharding behaviour
        (
            1,
            1,
            vec![1, 2, 3, 4, 5],
            Path::from(vec![Component::from(vec![1_u8])]),
        ),
        (
            2,
            1,
            vec![1, 2, 3, 4, 5],
            Path::from(vec![Component::from(vec![1_u8, 2_u8])]),
        ),
        (
            1,
            2,
            vec![1, 2, 3, 4, 5],
            Path::from(vec![
                Component::from(vec![1_u8]),
                Component::from(vec![2_u8]),
            ]),
        ),
        (
            2,
            2,
            vec![1, 2, 3, 4, 5],
            Path::from(vec![
                Component::from(vec![1_u8, 2_u8]),
                Component::from(vec![3_u8, 4_u8]),
            ]),
        ),
    ] {
        assert_eq!(output, Path::from((&ShardStrategy(width, depth), &b)));
        let bytes: &[u8] = b.as_ref();
        assert_eq!(output, Path::from((&ShardStrategy(width, depth), bytes)));
        assert_eq!(output, Path::from((&ShardStrategy(width, depth), b)));
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
        (1, 2, "foobar", Path::from("f.o")),
        (2, 2, "foobar", Path::from("fo.ob")),
        // multibyte characters should be handled the way a naive understanding of strings would
        // expect, i.e. that a 2-byte utf8 character is represented as 1 4-byte utf32 character and
        // so counts as 1 "width" and 1 "depth" for the purpose of sharding
        (2, 2, "€€€€", Path::from("€€.€€")),
        // if the string is shorter than the width and depth we go as deep as we can cleanly and
        // truncate the end
        (4, 4, "foobar", Path::from("foob")),
        (4, 4, "foobarbaz", Path::from("foob.arba")),
        (4, 4, "€€€€€€€€€", Path::from("€€€€.€€€€")),
    ] {
        assert_eq!(output, Path::from((&ShardStrategy(width, depth), s)));
        assert_eq!(
            output,
            Path::from((&ShardStrategy(width, depth), s.to_string()))
        );
        assert_eq!(
            output,
            Path::from((&ShardStrategy(width, depth), &s.to_string()))
        );
    }
}
