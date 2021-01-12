use holochain_sqlite::{error::DatabaseResult, prelude::IntKey, transaction::Reader};

const BYTE_SIZE_MARKERS: [char; 6] = [' ', 'K', 'M', 'G', 'T', 'P'];

pub fn human_size(size: usize) -> String {
    fn recurse(size: f32, marker_index: usize) -> String {
        if size > 1024. {
            recurse(size / 1024., marker_index + 1)
        } else {
            if marker_index > 0 {
                format!("{:.1} {}iB", size, BYTE_SIZE_MARKERS[marker_index])
            } else {
                format!("{} B", size)
            }
        }
    }
    recurse(size as f32, 0)
}

fn dump_iter<'i>(
    name: &str,
    it: impl Iterator<Item = Result<(&'i [u8], Option<rkv::Value<'i>>), rkv::StoreError>>,
) -> DatabaseResult<()> {
    let items = it
        .map(|kv| {
            let (k, v) = kv.unwrap();
            let key = k.len();
            let val = v.map(|v| v.to_bytes().unwrap().len()).unwrap_or(0);
            key + val
        })
        .collect();
    println!("<DB \"{}\">", name);
    println!("{}", SizeStats::new(items));
    Ok(())
}

#[allow(dead_code)]
fn dump_iter_multi<'i>(
    name: &str,
    it: impl Iterator<Item = Result<(&'i [u8], rkv::Value<'i>), rkv::StoreError>>,
) -> DatabaseResult<()> {
    let items = it
        .map(|kv| {
            // FIXME: we're ignoring the key here because its duplicated across items
            let (_, v) = kv.unwrap();
            let val = v.to_bytes().unwrap().len();
            val
        })
        .collect();
    println!("<DB \"{}\">", name);
    println!("{}", SizeStats::new(items));
    Ok(())
}

pub fn dump_kv(reader: &Reader, name: &str, db: rkv::SingleStore) -> DatabaseResult<()> {
    dump_iter(name, db.iter_start(reader)?)
}

pub fn dump_kvi(reader: &Reader, name: &str, db: rkv::IntegerStore<IntKey>) -> DatabaseResult<()> {
    dump_iter(name, db.iter_start(reader)?)
}

// TODO:
// pub fn dump_kvv(reader: &Reader, name: &str, db: rkv::MultiStore) -> DatabaseResult<()> {
//     dump_iter_multi(name, db.iter_start(reader)?)
// }

pub struct SizeStats {
    count: usize,
    total: usize,
    mean: Option<f32>,
    variance: Option<f32>,
}

impl SizeStats {
    pub fn new(items: Vec<usize>) -> Self {
        let count = items.iter().count();
        let total = items.iter().sum();
        if count > 0 {
            let mean = (total as f32) / (count as f32);
            let variance = items
                .into_iter()
                .map(|x| f32::powi(x as f32 - mean, 2))
                .sum::<f32>()
                / (count as f32);
            Self {
                count,
                total,
                mean: Some(mean),
                variance: Some(variance),
            }
        } else {
            Self {
                count,
                total,
                mean: None,
                variance: None,
            }
        }
    }
}

impl std::fmt::Display for SizeStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "     # items = {}", self.count)?;
        writeln!(f, "  total size = {}", human_size(self.total))?;
        if let Some(mean) = self.mean {
            writeln!(f, "           μ = {}", human_size(mean as usize))?;
        }
        if let Some(variance) = self.variance {
            writeln!(f, "          σ² = {}", human_size(variance as usize))?
        }
        Ok(())
    }
}
