//! In-memory stub buffer used by Emacs test files that call
//! `with-temp-buffer`, `insert`, `point`, `goto-char`, etc.
//!
//! This is *not* a real `DocumentBuffer` — there's no rope and no
//! syntax-table analysis. It's a String + cursor + narrowing + a
//! per-buffer marker list + a thread-local named-buffer registry +
//! a current-buffer stack so `with-temp-buffer` / `with-current-buffer`
//! can save/restore.
//!
//! ~35-80 % of Emacs test files need a buffer to do anything useful.
//! Without this infrastructure, every `(with-temp-buffer (insert ...)
//! ...)` fails on the first `insert`. With it, the test body runs
//! against a working in-memory text buffer; whether the *test* passes
//! is then a function of the actual primitives built on top.
//!
//! # Buffer IDs
//!
//! Each buffer gets a monotonic `BufferId`. The thread-local registry
//! holds the owning `StubBuffer` structures keyed by id. Elisp-side,
//! a buffer object is represented as `LispObject::String("<name>")`
//! (what Emacs calls "by name") — we resolve to an id through the
//! registry at each call. Markers carry `BufferId` so they stay
//! attached even when the buffer is renamed.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

pub type BufferId = usize;
pub type OverlayId = usize;

static NEXT_BUFFER_ID: AtomicUsize = AtomicUsize::new(1);
static NEXT_MARKER_ID: AtomicUsize = AtomicUsize::new(1);
static NEXT_OVERLAY_ID: AtomicUsize = AtomicUsize::new(1);

/// In-memory overlay, shared by the [`Registry`] and exposed to Elisp as
/// `(overlay . <OverlayId>)`. Only the behaviour exercised by the
/// `buffer-tests` / `overlay-tests` fixtures is modelled — start/end
/// positions, the owning buffer id, front/rear advance flags, and a
/// simple property plist.
#[derive(Debug, Clone)]
pub struct Overlay {
    pub id: OverlayId,
    pub buffer: BufferId,
    /// 1-based inclusive start. `None` after `delete-overlay`.
    pub start: Option<usize>,
    /// 1-based exclusive end. `None` after `delete-overlay`.
    pub end: Option<usize>,
    pub front_advance: bool,
    pub rear_advance: bool,
    /// Property list as a flat `Vec<(key, value)>`.
    pub plist: Vec<(crate::object::LispObject, crate::object::LispObject)>,
}

impl Overlay {
    fn new(
        id: OverlayId,
        buffer: BufferId,
        start: usize,
        end: usize,
        front_advance: bool,
        rear_advance: bool,
    ) -> Self {
        let (a, b) = if start <= end { (start, end) } else { (end, start) };
        Self {
            id,
            buffer,
            start: Some(a),
            end: Some(b),
            front_advance,
            rear_advance,
            plist: Vec::new(),
        }
    }
}

/// Match data saved by the last successful regex search. Shared
/// per-thread (real Emacs: per-buffer, but overwhelmingly the tests
/// just need match-beginning/end to be accessible after a search).
#[derive(Debug, Default, Clone)]
pub struct MatchData {
    /// Vec of (start, end) 1-based char offsets. Index 0 is the whole
    /// match; subsequent entries are groups. None represents an
    /// unmatched group.
    pub groups: Vec<Option<(usize, usize)>>,
    /// The buffer the match was performed against. For string-match,
    /// None.
    pub buffer: Option<BufferId>,
    /// The string that was matched against (for `match-string` on
    /// string-match — we need the original).
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Marker {
    pub id: usize,
    pub buffer: BufferId,
    /// 1-based char offset. `None` means this marker points nowhere
    /// (e.g. after kill-buffer).
    pub position: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct StubBuffer {
    pub id: BufferId,
    /// Buffer text. Operates on character offsets, not byte offsets,
    /// to match Emacs semantics (point is a character index).
    pub text: String,
    /// 1-based char offset matching Emacs's `point` convention.
    pub point: usize,
    pub mark: Option<usize>,
    pub mark_active: bool,
    pub name: String,
    pub modified: bool,
    /// Bumped on every mutating edit. Real Emacs's buffer-modified-tick.
    pub modified_tick: u64,
    /// Narrowing: if `Some((a, b))` then point-min = a and point-max = b
    /// instead of 1..=text.chars().count()+1.
    pub restriction: Option<(usize, usize)>,
    /// File this buffer visits (Emacs's `buffer-file-name`). None for
    /// temp/scratch buffers.
    pub file_name: Option<String>,
    /// Per-buffer local-variable bindings. Keyed by symbol name.
    pub locals: HashMap<String, crate::object::LispObject>,
}

impl StubBuffer {
    fn new_raw(name: String) -> Self {
        let id = NEXT_BUFFER_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            id,
            text: String::new(),
            point: 1,
            mark: None,
            mark_active: false,
            name,
            modified: false,
            modified_tick: 0,
            restriction: None,
            file_name: None,
            locals: HashMap::new(),
        }
    }

    pub fn new(name: impl Into<String>) -> Self {
        Self::new_raw(name.into())
    }

    pub fn point_min(&self) -> usize {
        self.restriction.map(|(a, _)| a).unwrap_or(1)
    }

    pub fn point_max(&self) -> usize {
        match self.restriction {
            Some((_, b)) => b,
            None => self.text.chars().count() + 1,
        }
    }

    pub fn buffer_size(&self) -> usize {
        self.text.chars().count()
    }

    /// Convert a 1-based char offset into a byte offset, clamped to the
    /// actual text bounds (not the narrow restriction — callers clamp
    /// to point-min/point-max themselves).
    pub fn char_to_byte(&self, char_pos: usize) -> usize {
        let clamped = char_pos.saturating_sub(1).min(self.text.chars().count());
        self.text
            .char_indices()
            .nth(clamped)
            .map(|(b, _)| b)
            .unwrap_or(self.text.len())
    }

    pub fn char_at(&self, char_pos: usize) -> Option<char> {
        if char_pos < 1 || char_pos > self.text.chars().count() {
            return None;
        }
        self.text.chars().nth(char_pos - 1)
    }

    pub fn insert(&mut self, s: &str) {
        let byte_idx = self.char_to_byte(self.point);
        self.text.insert_str(byte_idx, s);
        let n = s.chars().count();
        self.point += n;
        self.bump_modified();
    }

    pub fn goto_char(&mut self, pos: usize) {
        let a = self.point_min();
        let b = self.point_max();
        self.point = pos.clamp(a, b);
    }

    pub fn delete_region(&mut self, start: usize, end: usize) {
        let (a, b) = if start <= end { (start, end) } else { (end, start) };
        let pmin = self.point_min();
        let pmax = self.point_max();
        let a = a.clamp(pmin, pmax);
        let b = b.clamp(pmin, pmax);
        let a_byte = self.char_to_byte(a);
        let b_byte = self.char_to_byte(b);
        self.text.replace_range(a_byte..b_byte, "");
        if self.point > a {
            self.point = if self.point > b {
                self.point - (b - a)
            } else {
                a
            };
        }
        // Narrow bounds collapse if we deleted past them.
        if let Some((na, nb)) = self.restriction {
            if b > na {
                let shrink = b.min(nb).saturating_sub(a.max(na));
                self.restriction = Some((na, nb.saturating_sub(shrink).max(na)));
            }
        }
        self.bump_modified();
    }

    pub fn erase(&mut self) {
        self.text.clear();
        self.point = 1;
        self.restriction = None;
        self.bump_modified();
    }

    pub fn buffer_string(&self) -> String {
        // Return the NARROWED text if a restriction is active.
        if let Some((a, b)) = self.restriction {
            return self.substring(a, b);
        }
        self.text.clone()
    }

    pub fn substring(&self, start: usize, end: usize) -> String {
        let (a, b) = if start <= end { (start, end) } else { (end, start) };
        let pmin = self.point_min();
        let pmax = self.point_max();
        let a = a.clamp(pmin, pmax);
        let b = b.clamp(pmin, pmax);
        let a_byte = self.char_to_byte(a);
        let b_byte = self.char_to_byte(b);
        self.text[a_byte..b_byte].to_string()
    }

    pub fn bump_modified(&mut self) {
        self.modified = true;
        self.modified_tick = self.modified_tick.wrapping_add(1);
    }

    /// 1-based index of line containing `pos`.
    pub fn line_number_at_pos(&self, pos: usize) -> usize {
        let byte = self.char_to_byte(pos);
        1 + self.text[..byte].bytes().filter(|&b| b == b'\n').count()
    }

    /// Beginning of line containing `pos`.
    pub fn line_beginning_position(&self, pos: usize) -> usize {
        let byte = self.char_to_byte(pos);
        // Walk back to find the previous \n (or bos).
        if let Some(p) = self.text[..byte].rfind('\n') {
            // p is byte offset of the \n; line begins at next char.
            // Convert back to char offset.
            1 + self.text[..=p].chars().count()
        } else {
            1
        }
    }

    /// End of line containing `pos` (position of \n or point-max).
    pub fn line_end_position(&self, pos: usize) -> usize {
        let byte = self.char_to_byte(pos);
        if let Some(off) = self.text[byte..].find('\n') {
            1 + self.text[..byte + off].chars().count()
        } else {
            self.text.chars().count() + 1
        }
    }

    /// Move point forward by `n` logical lines. Returns remaining
    /// lines that couldn't be moved (positive = hit end-of-buffer).
    pub fn forward_line(&mut self, n: i64) -> i64 {
        if n >= 0 {
            let mut remaining = n;
            while remaining > 0 {
                let eol = self.line_end_position(self.point);
                if eol >= self.point_max() {
                    self.point = self.point_max();
                    return remaining;
                }
                self.point = eol + 1; // past the newline
                remaining -= 1;
            }
            // After reaching the target line, point is at beginning of it.
            self.point = self.line_beginning_position(self.point);
            0
        } else {
            let mut remaining = -n;
            while remaining > 0 {
                let bol = self.line_beginning_position(self.point);
                if bol <= self.point_min() {
                    self.point = self.point_min();
                    return -remaining;
                }
                self.point = bol - 1; // onto previous line's \n
                remaining -= 1;
            }
            self.point = self.line_beginning_position(self.point);
            0
        }
    }
}

#[derive(Debug, Default)]
pub struct Registry {
    pub buffers: HashMap<BufferId, StubBuffer>,
    pub by_name: HashMap<String, BufferId>,
    pub markers: HashMap<usize, Marker>,
    /// All overlays ever created in this registry, keyed by id. Deleted
    /// overlays keep their entry but with `start` and `end` set to
    /// `None` (Emacs's "detached overlay" state), so `overlayp` still
    /// returns t for them.
    pub overlays: HashMap<OverlayId, Overlay>,
    /// Stack of currently active buffer ids. Top is `current-buffer`.
    /// Always has at least one entry (the default `*scratch*`).
    pub stack: Vec<BufferId>,
    pub match_data: MatchData,
}

impl Registry {
    fn new() -> Self {
        let mut r = Self::default();
        let scratch = StubBuffer::new("*scratch*");
        let id = scratch.id;
        r.buffers.insert(id, scratch);
        r.by_name.insert("*scratch*".into(), id);
        r.stack.push(id);
        r
    }

    pub fn current_id(&self) -> BufferId {
        *self.stack.last().expect("buffer stack is never empty")
    }

    pub fn lookup_by_name(&self, name: &str) -> Option<BufferId> {
        self.by_name.get(name).copied()
    }

    pub fn get(&self, id: BufferId) -> Option<&StubBuffer> {
        self.buffers.get(&id)
    }

    pub fn get_mut(&mut self, id: BufferId) -> Option<&mut StubBuffer> {
        self.buffers.get_mut(&id)
    }

    pub fn create(&mut self, name: &str) -> BufferId {
        if let Some(id) = self.lookup_by_name(name) {
            return id;
        }
        let buf = StubBuffer::new(name.to_string());
        let id = buf.id;
        self.buffers.insert(id, buf);
        self.by_name.insert(name.to_string(), id);
        id
    }

    pub fn rename(&mut self, id: BufferId, new_name: &str) -> bool {
        let old_name = match self.buffers.get(&id) {
            Some(b) => b.name.clone(),
            None => return false,
        };
        if self.by_name.contains_key(new_name) && self.by_name[new_name] != id {
            return false;
        }
        self.by_name.remove(&old_name);
        self.by_name.insert(new_name.to_string(), id);
        if let Some(b) = self.buffers.get_mut(&id) {
            b.name = new_name.to_string();
        }
        true
    }

    pub fn kill(&mut self, id: BufferId) -> bool {
        // Can't kill the last-remaining buffer on the stack — Emacs
        // always requires *some* current buffer.
        if self.stack.len() == 1 && self.stack[0] == id {
            return false;
        }
        let name = match self.buffers.remove(&id) {
            Some(b) => b.name,
            None => return false,
        };
        self.by_name.remove(&name);
        self.stack.retain(|&b| b != id);
        // Invalidate markers in the killed buffer.
        for m in self.markers.values_mut() {
            if m.buffer == id {
                m.position = None;
            }
        }
        true
    }

    /// Push a NEW current-buffer frame. Used only by
    /// `with-temp-buffer` / `with-current-buffer` which need to
    /// restore the previous buffer on exit. Regular `set-buffer`,
    /// `switch-to-buffer`, `display-buffer` must go through
    /// [`set_current`] instead so the stack stays bounded.
    pub fn push_stack(&mut self, id: BufferId) {
        self.stack.push(id);
    }

    /// Replace the top-of-stack buffer id. This is the correct
    /// operation for `set-buffer` / `switch-to-buffer`, which change
    /// `current-buffer` *in place* rather than layering a new frame.
    pub fn set_current(&mut self, id: BufferId) {
        if let Some(top) = self.stack.last_mut() {
            *top = id;
        } else {
            self.stack.push(id);
        }
    }

    pub fn pop_stack(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }

    pub fn list(&self) -> Vec<BufferId> {
        // Current buffer first, then the remaining buffers in insertion order.
        let current = self.current_id();
        let mut out = vec![current];
        for &id in self.buffers.keys().filter(|&&id| id != current) {
            out.push(id);
        }
        out
    }

    pub fn make_marker(&mut self, buffer: BufferId) -> usize {
        let id = NEXT_MARKER_ID.fetch_add(1, Ordering::Relaxed);
        self.markers.insert(id, Marker { id, buffer, position: None });
        id
    }

    pub fn marker_set(&mut self, id: usize, buffer: BufferId, pos: Option<usize>) {
        self.markers
            .entry(id)
            .and_modify(|m| {
                m.buffer = buffer;
                m.position = pos;
            })
            .or_insert(Marker { id, buffer, position: pos });
    }

    /// Create a fresh overlay spanning `[start, end)` in `buffer`.
    /// Returns the new overlay's id. `start` / `end` are swapped if
    /// passed in the wrong order so callers don't have to.
    pub fn make_overlay(
        &mut self,
        buffer: BufferId,
        start: usize,
        end: usize,
        front_advance: bool,
        rear_advance: bool,
    ) -> OverlayId {
        let id = NEXT_OVERLAY_ID.fetch_add(1, Ordering::Relaxed);
        let ov = Overlay::new(id, buffer, start, end, front_advance, rear_advance);
        self.overlays.insert(id, ov);
        id
    }

    pub fn overlay_get(&self, id: OverlayId) -> Option<&Overlay> {
        self.overlays.get(&id)
    }

    pub fn overlay_get_mut(&mut self, id: OverlayId) -> Option<&mut Overlay> {
        self.overlays.get_mut(&id)
    }

    /// Mark overlay `id` detached. Sets its `start` / `end` to `None`
    /// but keeps the record so `overlayp` still returns `t`. Returns
    /// true if the id existed.
    pub fn delete_overlay(&mut self, id: OverlayId) -> bool {
        match self.overlays.get_mut(&id) {
            Some(ov) => {
                ov.start = None;
                ov.end = None;
                true
            }
            None => false,
        }
    }

    /// Collect every live overlay in `buffer` that covers position
    /// `pos`. Emacs semantics: `[S, E)` covers `P` iff `S <= P < E`.
    /// A zero-length overlay (`S == E`) covers `P` iff `P == S`.
    pub fn overlays_at(&self, buffer: BufferId, pos: usize) -> Vec<OverlayId> {
        let mut out = Vec::new();
        for ov in self.overlays.values() {
            if ov.buffer != buffer {
                continue;
            }
            let (Some(s), Some(e)) = (ov.start, ov.end) else {
                continue;
            };
            let covers = if s == e { pos == s } else { pos >= s && pos < e };
            if covers {
                out.push(ov.id);
            }
        }
        out
    }

    /// Collect every live overlay in `buffer` that overlaps the
    /// half-open range `[beg, end)`. An overlay is included iff its
    /// span shares any position with the range; zero-length overlays
    /// exactly at `beg` or `end` also count.
    pub fn overlays_in(&self, buffer: BufferId, beg: usize, end: usize) -> Vec<OverlayId> {
        let (b, e) = if beg <= end { (beg, end) } else { (end, beg) };
        let mut out = Vec::new();
        for ov in self.overlays.values() {
            if ov.buffer != buffer {
                continue;
            }
            let (Some(s), Some(ee)) = (ov.start, ov.end) else {
                continue;
            };
            let overlaps = if s == ee {
                s >= b && s <= e
            } else if b == e {
                b >= s && b <= ee
            } else {
                s < e && ee > b
            };
            if overlaps {
                out.push(ov.id);
            }
        }
        out
    }
}

thread_local! {
    /// Per-thread registry. Each worker subprocess has one.
    pub static REGISTRY: RefCell<Registry> = RefCell::new(Registry::new());
}

pub fn with_registry<R>(f: impl FnOnce(&Registry) -> R) -> R {
    REGISTRY.with(|r| f(&r.borrow()))
}

pub fn with_registry_mut<R>(f: impl FnOnce(&mut Registry) -> R) -> R {
    REGISTRY.with(|r| f(&mut r.borrow_mut()))
}

/// Run `f` against the current buffer. Read-only access.
pub fn with_current<R>(f: impl FnOnce(&StubBuffer) -> R) -> R {
    with_registry(|r| {
        let id = r.current_id();
        f(r.get(id).expect("current buffer missing"))
    })
}

/// Run `f` against the current buffer with mutable access.
pub fn with_current_mut<R>(f: impl FnOnce(&mut StubBuffer) -> R) -> R {
    with_registry_mut(|r| {
        let id = r.current_id();
        f(r.get_mut(id).expect("current buffer missing"))
    })
}

/// Push a fresh anonymous buffer onto the stack. Use this for
/// `with-temp-buffer`. The caller MUST `pop_buffer` after running the
/// body (typically via unwind_protect).
pub fn push_temp_buffer() {
    with_registry_mut(|r| {
        let id = r.create(" *temp*");
        // `(let ((*temp*)) ...)` semantics: each push makes a fresh
        // distinct buffer even if the name clashes. So generate a
        // unique name.
        let unique = format!(" *temp*<{id}>");
        r.rename(id, &unique);
        r.push_stack(id);
    });
}

/// Pop the current buffer if it's not the bottom-of-stack scratch.
pub fn pop_buffer() {
    with_registry_mut(|r| r.pop_stack());
}

/// Reset the registry to just a fresh `*scratch*`. Used by tests that
/// want a clean slate.
#[allow(dead_code)]
pub fn reset() {
    REGISTRY.with(|r| *r.borrow_mut() = Registry::new());
}

/// Switch the current buffer to the named one, creating it if needed.
/// Returns the old current-buffer name. Replaces the top-of-stack in
/// place — to *save* the previous buffer use `push_stack` + callers'
/// own unwind logic.
pub fn set_current_by_name(name: &str) -> String {
    with_registry_mut(|r| {
        let id = r.create(name);
        let old = r.get(r.current_id()).map(|b| b.name.clone()).unwrap_or_default();
        r.set_current(id);
        old
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_advances_point() {
        reset();
        with_current_mut(|b| b.insert("hello"));
        assert_eq!(with_current(|b| b.text.clone()), "hello");
        assert_eq!(with_current(|b| b.point), 6);
    }

    #[test]
    fn goto_char_clamps() {
        reset();
        with_current_mut(|b| b.insert("hi"));
        with_current_mut(|b| b.goto_char(99));
        assert_eq!(with_current(|b| b.point), 3); // point-max
        with_current_mut(|b| b.goto_char(0));
        assert_eq!(with_current(|b| b.point), 1); // point-min
    }

    #[test]
    fn temp_buffer_isolates() {
        reset();
        with_current_mut(|b| b.insert("outer"));
        push_temp_buffer();
        with_current_mut(|b| b.insert("inner"));
        assert_eq!(with_current(|b| b.text.clone()), "inner");
        pop_buffer();
        assert_eq!(with_current(|b| b.text.clone()), "outer");
    }

    #[test]
    fn delete_region_removes_chars() {
        reset();
        with_current_mut(|b| b.insert("0123456789"));
        with_current_mut(|b| b.delete_region(3, 6)); // delete "234"
        assert_eq!(with_current(|b| b.text.clone()), "0156789");
    }

    #[test]
    fn narrowing_affects_point_min_max() {
        reset();
        with_current_mut(|b| {
            b.insert("0123456789");
            b.restriction = Some((3, 7)); // narrow to "2345"
        });
        assert_eq!(with_current(|b| b.point_min()), 3);
        assert_eq!(with_current(|b| b.point_max()), 7);
    }

    #[test]
    fn line_positions() {
        reset();
        with_current_mut(|b| b.insert("aaa\nbbb\nccc"));
        // point after "aaa\n" = 5; line-beginning = 5; line-end = 8
        with_current_mut(|b| b.goto_char(5));
        assert_eq!(with_current(|b| b.line_beginning_position(b.point)), 5);
        assert_eq!(with_current(|b| b.line_end_position(b.point)), 8);
        assert_eq!(with_current(|b| b.line_number_at_pos(b.point)), 2);
    }

    #[test]
    fn forward_line_walks() {
        reset();
        with_current_mut(|b| b.insert("a\nb\nc\nd"));
        with_current_mut(|b| b.goto_char(1));
        let remain = with_current_mut(|b| b.forward_line(2));
        assert_eq!(remain, 0);
        assert_eq!(with_current(|b| b.point), 5); // start of "c"
    }

    #[test]
    fn named_buffer_registry() {
        reset();
        let id_a = with_registry_mut(|r| r.create("a"));
        let id_b = with_registry_mut(|r| r.create("b"));
        assert_ne!(id_a, id_b);
        assert_eq!(with_registry(|r| r.lookup_by_name("a")), Some(id_a));
        assert!(with_registry_mut(|r| r.rename(id_a, "renamed")));
        assert_eq!(with_registry(|r| r.lookup_by_name("a")), None);
        assert_eq!(
            with_registry(|r| r.lookup_by_name("renamed")),
            Some(id_a)
        );
    }

    /// Regression: R1. `set-buffer` (and `switch-to-buffer` /
    /// `display-buffer`) used to `push_stack(id)` on every call,
    /// growing the stack indefinitely. Each iteration of a loop that
    /// uses `with-current-buffer` (expanded as `let saved + set-buffer
    /// TARGET + unwind set-buffer saved`) leaked two entries. Here we
    /// exercise the pattern directly against the Registry and assert
    /// the stack stays bounded.
    #[test]
    fn set_current_does_not_grow_stack() {
        reset();
        let id_a = with_registry_mut(|r| r.create("a"));
        let id_b = with_registry_mut(|r| r.create("b"));
        let start_depth = with_registry(|r| r.stack.len());
        // Simulate 100 `with-current-buffer` style alternations.
        for _ in 0..100 {
            with_registry_mut(|r| r.set_current(id_a));
            with_registry_mut(|r| r.set_current(id_b));
        }
        assert_eq!(
            with_registry(|r| r.stack.len()),
            start_depth,
            "set_current must not grow the stack",
        );
        assert_eq!(with_registry(|r| r.current_id()), id_b);
    }

    /// `push_stack` is still the right op for `with-temp-buffer` /
    /// `with-current-buffer` so the previous buffer can be restored
    /// on exit. Verify that push/pop round-trips.
    #[test]
    fn push_stack_still_works_for_scoped_forms() {
        reset();
        let id_a = with_registry_mut(|r| r.create("a"));
        let initial = with_registry(|r| r.current_id());
        with_registry_mut(|r| r.push_stack(id_a));
        assert_eq!(with_registry(|r| r.current_id()), id_a);
        with_registry_mut(|r| r.pop_stack());
        assert_eq!(with_registry(|r| r.current_id()), initial);
    }
}
