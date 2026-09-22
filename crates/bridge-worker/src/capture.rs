use std::collections::VecDeque;

/// Fixed-size first/last capture. No unbounded channel or reader thread exists.
#[derive(Debug)]
pub struct Capture {
    head: Vec<u8>,
    tail: VecDeque<u8>,
    limit: usize,
    pub observed: u64,
    pub error_marker_seen: bool,
    scan_tail: Vec<u8>,
}
impl Capture {
    pub(crate) fn new(limit: usize) -> Self {
        Self { head: Vec::new(), tail: VecDeque::new(), limit, observed: 0,
            error_marker_seen: false, scan_tail: Vec::new() }
    }
    pub(crate) fn push(&mut self, bytes: &[u8]) {
        self.observed = self.observed.saturating_add(bytes.len() as u64);
        // Check every byte, not only retained output. Keep overlap for split markers.
        let mut scan = std::mem::take(&mut self.scan_tail);
        scan.extend_from_slice(bytes);
        self.error_marker_seen |= [b"SCRIPT ERROR:".as_slice(), b"ERROR:", b"Parse Error:"]
            .iter().any(|needle| scan.windows(needle.len()).any(|w| w == *needle));
        self.scan_tail = scan[scan.len().saturating_sub(32)..].to_vec();
        let take = (self.limit / 2 - self.head.len()).min(bytes.len());
        self.head.extend_from_slice(&bytes[..take]);
        let rest = &bytes[take..];
        let tail_limit = self.limit - self.limit / 2;
        if rest.len() >= tail_limit {
            self.tail.clear();
            self.tail.extend(&rest[rest.len() - tail_limit..]);
        } else {
            let evict = (self.tail.len() + rest.len()).saturating_sub(tail_limit);
            self.tail.drain(..evict);
            self.tail.extend(rest);
        }
    }
    pub fn retained_bytes(&self) -> usize { self.head.len() + self.tail.len() }
    pub fn omitted_bytes(&self) -> u64 { self.observed.saturating_sub(self.retained_bytes() as u64) }
    pub fn text(&self) -> String {
        let mut out = String::from_utf8_lossy(&self.head).into_owned();
        if self.omitted_bytes() > 0 {
            out.push_str(&format!("\n[Bridge omitted {} bytes; first/last output retained]\n", self.omitted_bytes()));
        }
        out.push_str(&String::from_utf8_lossy(&self.tail.iter().copied().collect::<Vec<_>>()));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn head_tail_budget_and_middle_error_survive() {
        let mut c = Capture::new(100);
        c.push(b"start"); c.push(&vec![b'x'; 150]);
        c.push(b"SCRIPT ER"); c.push(b"ROR: discarded middle");
        c.push(&vec![b'y'; 150]); c.push(b"end");
        assert_eq!(c.retained_bytes(), 100);
        assert!(c.text().starts_with("start")); assert!(c.text().ends_with("end"));
        assert!(c.error_marker_seen); assert!(c.omitted_bytes() > 0);
    }
    #[test]
    fn binary_and_partial_utf8_never_panic() {
        let mut c = Capture::new(9);
        c.push(&[0xff; 100]); c.push("é".as_bytes());
        assert_eq!(c.retained_bytes(), 9); assert!(!c.text().is_empty());
    }
}
