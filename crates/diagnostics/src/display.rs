const BYTE_SIZE_MARKERS: [char; 6] = [' ', 'K', 'M', 'G', 'T', 'P'];

pub fn human_size(size: usize) -> String {
    fn recurse(size: f32, marker_index: usize) -> String {
        if size > 1024. {
            recurse(size / 1024., marker_index + 1)
        } else {
            if marker_index > 0 {
                format!("{} {}B", size, BYTE_SIZE_MARKERS[marker_index])
            } else {
                format!("{} B", size)
            }
        }
    }
    recurse(size as f32, 0)
}
