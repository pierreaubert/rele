//! Rust equivalents of Emacs `fns.c` — sequence operations, hash table
//! operations, string utilities, and general-purpose functions.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

// ---- Sequence operations ----

/// `take` — return the first N elements of a slice.
pub fn take<T: Clone>(n: usize, items: &[T]) -> Vec<T> {
    items.iter().take(n).cloned().collect()
}

/// `ntake` — destructive version of take. In Rust, same as take since we own the vec.
pub fn ntake<T>(n: usize, mut items: Vec<T>) -> Vec<T> {
    items.truncate(n);
    items
}

/// `delete` — remove all elements `equal` to ELT from a sequence.
pub fn delete<T: PartialEq + Clone>(elt: &T, items: &[T]) -> Vec<T> {
    items.iter().filter(|x| *x != elt).cloned().collect()
}

/// `rassq` — find first entry in alist whose CDR is `eq` to KEY.
/// Returns the index of the matching entry, or `None`.
pub fn rassq_index<K: Eq, V: Eq>(key: &V, alist: &[(K, V)]) -> Option<usize> {
    alist.iter().position(|(_, v)| v == key)
}

/// `rassoc` — find first entry in alist whose CDR is `equal` to KEY.
pub fn rassoc_index<K, V: PartialEq>(key: &V, alist: &[(K, V)]) -> Option<usize> {
    alist.iter().position(|(_, v)| v == key)
}

/// `fillarray` — fill a mutable slice with a single value.
pub fn fillarray<T: Clone>(array: &mut [T], value: &T) {
    array.fill(value.clone());
}

/// `length<` / `length>` / `length=` — compare sequence length against N.
pub fn length_compare(len: usize, n: i64, ordering: std::cmp::Ordering) -> bool {
    let len_i = len as i64;
    len_i.cmp(&n) == ordering
}

// ---- String operations ----

/// `string-bytes` — byte length of a UTF-8 string.
pub fn string_bytes(s: &str) -> usize {
    s.len()
}

/// `substring-no-properties` — same as substring (no text properties in Rust).
/// Uses byte-offset slicing — no intermediate `Vec<char>`.
pub fn substring(s: &str, from: usize, to: Option<usize>) -> Option<String> {
    let mut char_indices = s.char_indices();
    let start_byte = if from == 0 {
        0
    } else {
        char_indices.nth(from - 1).map(|(i, c)| i + c.len_utf8())?
    };
    let end_byte = match to {
        None => s.len(),
        Some(end) => {
            if end <= from {
                return if end == from { Some(String::new()) } else { None };
            }
            let skip = end - from - 1;
            // char_indices is already positioned after `from`, advance to `end`
            match char_indices.nth(skip) {
                Some((i, c)) => i + c.len_utf8(),
                None => return None,
            }
        }
    };
    Some(s[start_byte..end_byte].to_string())
}

/// `string-distance` — Levenshtein edit distance.
pub fn string_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    let mut prev = (0..=n).collect::<Vec<_>>();
    let mut curr = vec![0; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = usize::from(a_chars[i - 1] != b_chars[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// `compare-strings` — compare substrings, optionally case-insensitive.
/// Returns 0 if equal, negative if a < b, positive if a > b.
/// The magnitude is 1 + index of first differing char.
/// Zero-allocation: uses char iterators instead of collecting to `Vec<char>`.
pub fn compare_strings(
    a: &str,
    a_start: usize,
    a_end: Option<usize>,
    b: &str,
    b_start: usize,
    b_end: Option<usize>,
    ignore_case: bool,
) -> i64 {
    let a_count = a_end.map_or(usize::MAX, |e| e.saturating_sub(a_start));
    let b_count = b_end.map_or(usize::MAX, |e| e.saturating_sub(b_start));
    let a_iter = a.chars().skip(a_start).take(a_count);
    let b_iter = b.chars().skip(b_start).take(b_count);

    for (i, (ac, bc)) in a_iter.clone().zip(b_iter.clone()).enumerate() {
        let (ca, cb) = if ignore_case {
            (
                ac.to_lowercase().next().unwrap_or(ac),
                bc.to_lowercase().next().unwrap_or(bc),
            )
        } else {
            (ac, bc)
        };
        if ca != cb {
            return if ca < cb {
                -(i as i64 + 1)
            } else {
                i as i64 + 1
            };
        }
    }
    // Count total lengths to compare when prefixes match
    let a_len = a_iter.count();
    let b_len = b_iter.count();
    if a_len == b_len {
        0
    } else if a_len < b_len {
        -(a_len as i64 + 1)
    } else {
        b_len as i64 + 1
    }
}

// ---- Hash operations ----

/// `sxhash-equal` — hash a string (most common case).
pub fn sxhash_string(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// `sxhash-eq` for integers.
pub fn sxhash_integer(n: i64) -> u64 {
    let mut hasher = DefaultHasher::new();
    n.hash(&mut hasher);
    hasher.finish()
}

/// Generic Emacs-style hash table backed by Rust `HashMap`.
#[derive(Debug, Clone)]
pub struct EmacsHashTable<K: Eq + Hash, V> {
    pub data: HashMap<K, V>,
    pub test: HashTest,
}

/// Hash table equality test (mirrors `hash-table-test`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashTest {
    Eq,
    Eql,
    Equal,
}

impl HashTest {
    pub fn name(self) -> &'static str {
        match self {
            Self::Eq => "eq",
            Self::Eql => "eql",
            Self::Equal => "equal",
        }
    }
}

impl<K: Eq + Hash, V> EmacsHashTable<K, V> {
    pub fn new(test: HashTest) -> Self {
        Self {
            data: HashMap::new(),
            test,
        }
    }

    pub fn count(&self) -> usize {
        self.data.len()
    }

    pub fn size(&self) -> usize {
        self.data.capacity()
    }
}

impl<K: Eq + Hash + Clone, V: Clone> EmacsHashTable<K, V> {
    pub fn copy(&self) -> Self {
        Self {
            data: self.data.clone(),
            test: self.test,
        }
    }
}

// ---- Sorting ----

/// Stable sort using a bool predicate (true = a < b).
pub fn stable_sort_by<T>(items: &mut [T], mut less_than: impl FnMut(&T, &T) -> bool) {
    items.sort_by(|a, b| {
        if less_than(a, b) {
            std::cmp::Ordering::Less
        } else if less_than(b, a) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_take() {
        assert_eq!(take(3, &[1, 2, 3, 4, 5]), vec![1, 2, 3]);
        assert_eq!(take(10, &[1, 2]), vec![1, 2]);
        assert_eq!(take(0, &[1, 2]), Vec::<i32>::new());
    }

    #[test]
    fn test_ntake() {
        assert_eq!(ntake(2, vec![1, 2, 3, 4]), vec![1, 2]);
    }

    #[test]
    fn test_fillarray() {
        let mut arr = vec![0, 0, 0];
        fillarray(&mut arr, &42);
        assert_eq!(arr, vec![42, 42, 42]);
    }

    #[test]
    fn test_string_bytes() {
        assert_eq!(string_bytes("hello"), 5);
        assert_eq!(string_bytes("héllo"), 6); // é is 2 bytes in UTF-8
    }

    #[test]
    fn test_substring() {
        assert_eq!(substring("hello", 1, Some(3)), Some("el".to_string()));
        assert_eq!(substring("hello", 0, None), Some("hello".to_string()));
        assert_eq!(substring("hello", 5, Some(10)), None);
    }

    #[test]
    fn test_string_distance() {
        assert_eq!(string_distance("kitten", "sitting"), 3);
        assert_eq!(string_distance("", "abc"), 3);
        assert_eq!(string_distance("abc", "abc"), 0);
    }

    #[test]
    fn test_compare_strings() {
        assert_eq!(compare_strings("abc", 0, None, "abc", 0, None, false), 0);
        assert!(compare_strings("abc", 0, None, "abd", 0, None, false) < 0);
        assert!(compare_strings("abd", 0, None, "abc", 0, None, false) > 0);
        assert_eq!(compare_strings("ABC", 0, None, "abc", 0, None, true), 0);
    }

    #[test]
    fn test_length_compare() {
        assert!(length_compare(3, 5, std::cmp::Ordering::Less));
        assert!(length_compare(5, 3, std::cmp::Ordering::Greater));
        assert!(length_compare(3, 3, std::cmp::Ordering::Equal));
    }

    #[test]
    fn test_sxhash() {
        let h1 = sxhash_string("hello");
        let h2 = sxhash_string("hello");
        assert_eq!(h1, h2);
        assert_ne!(sxhash_string("hello"), sxhash_string("world"));
    }

    #[test]
    fn test_hash_table() {
        let mut ht: EmacsHashTable<String, i64> = EmacsHashTable::new(HashTest::Equal);
        ht.data.insert("a".to_string(), 1);
        ht.data.insert("b".to_string(), 2);
        assert_eq!(ht.count(), 2);
        let copy = ht.copy();
        assert_eq!(copy.count(), 2);
        assert_eq!(copy.test, HashTest::Equal);
    }

    #[test]
    fn test_stable_sort() {
        let mut items = vec![3, 1, 4, 1, 5, 9];
        stable_sort_by(&mut items, |a, b| a < b);
        assert_eq!(items, vec![1, 1, 3, 4, 5, 9]);
    }

    #[test]
    fn test_rassq() {
        let alist = vec![("a", 1), ("b", 2), ("c", 3)];
        assert_eq!(rassq_index(&2, &alist), Some(1));
        assert_eq!(rassq_index(&4, &alist), None);
    }
}
