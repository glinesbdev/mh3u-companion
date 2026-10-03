/// A contiguous run of bytes that differ between two buffers.
pub struct Change {
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

/// Find differing runs, merging runs separated by fewer than `gap` equal bytes.
pub fn changes(a: &[u8], b: &[u8], gap: usize) -> Vec<Change> {
    let n = a.len().min(b.len());
    let mut out: Vec<Change> = Vec::new();
    let mut i = 0;
    while i < n {
        if a[i] == b[i] {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i + 1;
        let mut j = end;
        while j < n && j - end <= gap {
            if a[j] != b[j] {
                end = j + 1;
            }
            j += 1;
        }
        out.push(Change {
            offset: start,
            before: a[start..end].to_vec(),
            after: b[start..end].to_vec(),
        });
        i = end;
    }
    out
}
