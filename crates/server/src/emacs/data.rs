//! Rust equivalents of Emacs `data.c` — type classification, arithmetic,
//! bitwise operations, and symbol accessors.

// ---- Arithmetic ----

/// Emacs `%` (integer modulo, sign follows dividend like C).
pub fn emacs_mod(a: i64, b: i64) -> Option<i64> {
    if b == 0 {
        return None;
    }
    Some(a % b)
}

/// `logcount` — population count (number of 1-bits in unsigned representation).
/// For negative numbers, Emacs counts zero bits.
pub fn logcount(n: i64) -> u32 {
    if n >= 0 {
        (n as u64).count_ones()
    } else {
        // For negative: count zeros in the two's complement = count ones of bitwise NOT
        (!(n as u64)).count_ones()
    }
}

/// `byteorder` — 66 (B) for big-endian, 108 (l) for little-endian.
pub fn byteorder() -> i64 {
    if cfg!(target_endian = "little") {
        108
    } else {
        66
    }
}

/// `ash` — arithmetic shift. Positive COUNT shifts left, negative shifts right.
pub fn arithmetic_shift(value: i64, count: i64) -> i64 {
    if count >= 0 {
        if count >= 64 {
            0 // shifted out entirely
        } else {
            value.wrapping_shl(count as u32)
        }
    } else {
        let right = (-count) as u32;
        if right >= 64 {
            if value < 0 { -1 } else { 0 }
        } else {
            value >> right // arithmetic (sign-extending) for i64
        }
    }
}

/// `logand` — bitwise AND of a list of integers.
pub fn logand(values: &[i64]) -> i64 {
    values.iter().copied().fold(-1i64, |acc, v| acc & v)
}

/// `logior` — bitwise OR.
pub fn logior(values: &[i64]) -> i64 {
    values.iter().copied().fold(0i64, |acc, v| acc | v)
}

/// `logxor` — bitwise XOR.
pub fn logxor(values: &[i64]) -> i64 {
    values.iter().copied().fold(0i64, |acc, v| acc ^ v)
}

/// `lognot` — bitwise complement.
pub fn lognot(n: i64) -> i64 {
    !n
}

// ---- Subr introspection ----

/// Represents a subroutine's arity as `(min_args, max_args)`.
/// `max_args` of `None` means `&rest` (many).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubrArity {
    pub min_args: u16,
    pub max_args: Option<u16>,
}

impl SubrArity {
    pub const MANY: Self = Self {
        min_args: 0,
        max_args: None,
    };

    pub const fn fixed(n: u16) -> Self {
        Self {
            min_args: n,
            max_args: Some(n),
        }
    }
}

/// Known arities for built-in primitives.
/// Returns `None` for unknown primitives (caller falls back to `(0 . many)`).
pub fn subr_arity(name: &str) -> Option<SubrArity> {
    Some(match name {
        // data.c
        "car" | "cdr" | "car-safe" | "cdr-safe" | "not" | "null" | "atom"
        | "symbolp" | "stringp" | "consp" | "listp" | "nlistp" | "numberp"
        | "integerp" | "floatp" | "natnump" | "zerop" | "keywordp"
        | "subrp" | "functionp" | "arrayp" | "sequencep" | "bufferp"
        | "markerp" | "vectorp" | "booleanp" | "characterp" | "hash-table-p"
        | "byte-code-function-p" | "char-or-string-p" | "symbol-name"
        | "symbol-plist" | "symbol-function" | "symbol-value"
        | "type-of" | "1+" | "1-" | "lognot" | "logcount" | "float"
        | "identity" | "cadr" | "cddr"
        | "caar" | "cdar" | "safe-length" | "length"
        | "indirect-function" | "subr-name" | "subr-arity" => SubrArity::fixed(1),

        "cons" | "eq" | "equal" | "=" | "<" | ">" | "<=" | ">="
        | "/=" | "setcar" | "setcdr" | "setplist" | "fset" | "defalias"
        | "aref" | "assq" | "assoc" | "memq" | "member" | "nth"
        | "nthcdr" | "elt" | "mod" | "%" | "ash" | "put" | "get"
        | "plist-get" | "plist-member" | "string=" | "string<"
        | "rassq" | "rassoc" | "remhash" | "last" => SubrArity::fixed(2),

        "aset" | "plist-put" | "substring" | "define-key" => SubrArity::fixed(3),

        "+" | "-" | "*" | "max" | "min" | "logand" | "logior" | "logxor"
        | "list" | "append" | "concat" | "vconcat" | "nconc"
        | "format" | "message" | "error" | "signal" => SubrArity::MANY,

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logcount() {
        assert_eq!(logcount(0), 0);
        assert_eq!(logcount(1), 1);
        assert_eq!(logcount(7), 3);
        assert_eq!(logcount(255), 8);
        assert_eq!(logcount(-1), 0); // -1 is all 1-bits, NOT gives all 0-bits → count is 0
    }

    #[test]
    fn test_emacs_mod() {
        assert_eq!(emacs_mod(10, 3), Some(1));
        assert_eq!(emacs_mod(-10, 3), Some(-1));
        assert_eq!(emacs_mod(10, -3), Some(1));
        assert_eq!(emacs_mod(0, 5), Some(0));
        assert_eq!(emacs_mod(5, 0), None);
    }

    #[test]
    fn test_arithmetic_shift() {
        assert_eq!(arithmetic_shift(1, 4), 16);
        assert_eq!(arithmetic_shift(16, -2), 4);
        assert_eq!(arithmetic_shift(-1, -1), -1); // sign-extending
        assert_eq!(arithmetic_shift(1, 63), i64::MIN); // wrapping
    }

    #[test]
    fn test_byteorder() {
        let b = byteorder();
        assert!(b == 108 || b == 66);
    }

    #[test]
    fn test_bitwise() {
        assert_eq!(logand(&[0xFF, 0x0F]), 0x0F);
        assert_eq!(logior(&[0xF0, 0x0F]), 0xFF);
        assert_eq!(logxor(&[0xFF, 0x0F]), 0xF0);
        assert_eq!(lognot(0), -1);
    }

    #[test]
    fn test_subr_arity_known() {
        assert_eq!(subr_arity("car"), Some(SubrArity::fixed(1)));
        assert_eq!(subr_arity("cons"), Some(SubrArity::fixed(2)));
        assert_eq!(subr_arity("+"), Some(SubrArity::MANY));
    }

    #[test]
    fn test_subr_arity_unknown() {
        assert_eq!(subr_arity("my-custom-fn"), None);
    }
}
