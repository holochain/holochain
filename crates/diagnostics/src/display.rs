use holochain_state::{error::DatabaseResult, transaction::Reader};

const BYTE_SIZE_MARKERS: [char; 6] = [' ', 'K', 'M', 'G', 'T', 'P'];

pub fn human_size(size: usize) -> String {
    fn recurse(size: f32, marker_index: usize) -> String {
        if size > 1024. {
            recurse(size / 1024., marker_index + 1)
        } else {
            if marker_index > 0 {
                format!("{:.1} {}B", size, BYTE_SIZE_MARKERS[marker_index])
            } else {
                format!("{} B", size)
            }
        }
    }
    recurse(size as f32, 0)
}

pub fn dump_kv(reader: &Reader, name: &str, db: rkv::SingleStore) -> DatabaseResult<()> {
    let items = db
        .iter_start(reader)?
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
