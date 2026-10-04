//! Looking for a weapon's sharpness bar in memory (the `find` debug command).
//!
//! Where the game keeps sharpness is not known: the bar's numbers (red, orange, yellow ...) as a public database lists them were not found
//! in the executable as bytes, words, floats or running totals. The game may keep them in other units, so this looks for values that are
//! *in proportion* to the bar: any scale, as bytes, 16-bit or 32-bit values in either byte order, 32-bit floats, and the bar's running
//! totals. The database's numbers are rounded, so each may be off by half a unit of the bar's scale.

/// How the numbers of a hit are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    U8,
    U16Be,
    U16Le,
    U32Be,
    U32Le,
    F32Be,
}

impl Layout {
    pub const ALL: [Layout; 6] = [
        Layout::U8,
        Layout::U16Be,
        Layout::U16Le,
        Layout::U32Be,
        Layout::U32Le,
        Layout::F32Be,
    ];

    pub fn width(self) -> usize {
        match self {
            Layout::U8 => 1,
            Layout::U16Be | Layout::U16Le => 2,
            _ => 4,
        }
    }

    fn read(self, b: &[u8]) -> f64 {
        match self {
            Layout::U8 => f64::from(b[0]),
            Layout::U16Be => f64::from(u16::from_be_bytes([b[0], b[1]])),
            Layout::U16Le => f64::from(u16::from_le_bytes([b[0], b[1]])),
            Layout::U32Be => f64::from(u32::from_be_bytes([b[0], b[1], b[2], b[3]])),
            Layout::U32Le => f64::from(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            Layout::F32Be => f64::from(f32::from_be_bytes([b[0], b[1], b[2], b[3]])),
        }
    }
}

/// Numbers found in proportion to the bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// Offset of the first number in the data searched.
    pub offset: usize,
    pub layout: Layout,
    /// The bar was matched as its running totals.
    pub running: bool,
    pub values: Vec<f64>,
}

fn matches(values: &[f64], bar: &[u32]) -> bool {
    // the scale from the first number; every other is within half a bar unit of it (and a zero stays zero)
    if values[0] <= 0.0 || !values[0].is_finite() {
        return false;
    }
    let scale = values[0] / f64::from(bar[0]);
    values.iter().zip(bar).all(|(&v, &b)| {
        if b == 0 {
            v == 0.0
        } else {
            v.is_finite() && (v - scale * f64::from(b)).abs() <= scale * 0.6
        }
    })
}

/// Every place in `data` (up to `start_below`) where the bar's numbers, or its running totals, are stored in one of the layouts. Whole
/// numbers must be at least 0.8 of a database unit each (a smaller scale only matches zeros and ones, and everything is in it).
pub fn find(data: &[u8], bar: &[u32], start_below: usize) -> Vec<Hit> {
    let mut out = Vec::new();
    if bar.len() < 3 || bar[0] == 0 {
        return out;
    }
    let totals: Vec<u32> = bar
        .iter()
        .scan(0, |sum, &v| {
            *sum += v;
            Some(*sum)
        })
        .collect();
    let mut values = vec![0.0; bar.len()];
    for layout in Layout::ALL {
        let w = layout.width();
        let span = w * bar.len();
        let mut o = 0;
        while o < start_below.min(data.len().saturating_sub(span - 1)) {
            // cheap test first: the first two numbers
            let first = layout.read(&data[o..]);
            if first > 0.0 {
                for (running, target) in [(false, bar), (true, &totals[..])] {
                    let scale = first / f64::from(target[0]);
                    if scale < 0.8 && layout != Layout::F32Be {
                        continue;
                    }
                    let second = layout.read(&data[o + w..]);
                    if (second - scale * f64::from(target[1])).abs() > scale * 0.6 {
                        continue;
                    }
                    for (k, v) in values.iter_mut().enumerate() {
                        *v = layout.read(&data[o + k * w..]);
                    }
                    if matches(&values, target) {
                        out.push(Hit {
                            offset: o,
                            layout,
                            running,
                            values: values.clone(),
                        });
                    }
                }
            }
            o += w;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_bar_stored_in_another_unit() {
        // 22 11 22 11 20 2 at a scale of 10, as 16-bit words, after some other numbers
        let mut data = vec![7u8, 7, 7, 7];
        for v in [220u16, 110, 220, 110, 200, 20] {
            data.extend(v.to_be_bytes());
        }
        data.extend([9, 9]);
        let hits = find(&data, &[22, 11, 22, 11, 20, 2], data.len());
        assert!(
            hits.iter().any(|h| h.offset == 4 && h.layout == Layout::U16Be && !h.running),
            "{hits:?}"
        );
    }

    #[test]
    fn allows_rounding_but_not_a_different_bar() {
        // the true numbers are 18, 8, 4 (database: 20, 9, 4 after rounding at 1.1)
        let data = [18u8, 8, 4, 0, 0, 0];
        assert!(!find(&data, &[20, 9, 4, 0, 0, 0], data.len()).is_empty());
        assert!(find(&[18u8, 4, 8, 0, 0, 0], &[20, 9, 4, 0, 0, 0], 6).is_empty());
    }

    #[test]
    fn finds_running_totals_and_floats() {
        let totals = [15u8, 24, 50, 65, 65, 65];
        let hits = find(&totals, &[15, 9, 26, 15, 0, 0], 6);
        assert!(hits.iter().any(|h| h.running), "{hits:?}");
        let mut f = Vec::new();
        for v in [20.0f32, 9.0, 4.0, 0.0, 0.0, 0.0] {
            f.extend(v.to_be_bytes());
        }
        assert!(find(&f, &[20, 9, 4, 0, 0, 0], f.len()).iter().any(|h| h.layout == Layout::F32Be));
    }
}
