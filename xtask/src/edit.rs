// SPDX-License-Identifier: GPL-2.0-or-later
//! A small, deterministic, idempotent source-editing toolkit.
//!
//! Every wiring step the scaffolder performs is one of three shapes — *insert a block at a
//! `// @scaffold:` anchor or beside an exact line*, *replace exactly one occurrence*, or
//! *append to a Cargo feature list* — and each is **idempotent** (a re-run is a no-op once
//! its result is present) and **loud on a miss** (a moved anchor is an error, never a silent
//! mis-edit). That is what makes the generated diff reviewable and the scaffolder safe to
//! run twice. A [`Doc`] is one loaded file; nothing touches disk until [`Doc::save`].

use std::fs;
use std::path::{Path, PathBuf};

/// One source file, loaded into memory for a batch of edits and written back once.
pub struct Doc {
    path: PathBuf,
    text: String,
}

impl Doc {
    /// Load `path` for editing.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let text =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        Ok(Self { path, text })
    }

    /// Insert `block` immediately before the line containing `anchor`. A no-op if `guard`
    /// is already present anywhere in the file (idempotency); errors if `anchor` is missing.
    pub fn insert_before(&mut self, anchor: &str, guard: &str, block: &str) -> Result<(), String> {
        if self.text.contains(guard) {
            return Ok(());
        }
        let at = self.line_start(anchor)?;
        self.text.insert_str(at, block);
        Ok(())
    }

    /// Insert `block` immediately after the line containing `anchor`. A no-op if `guard` is
    /// already present anywhere in the file; errors if `anchor` is missing.
    pub fn insert_after(&mut self, anchor: &str, guard: &str, block: &str) -> Result<(), String> {
        if self.text.contains(guard) {
            return Ok(());
        }
        let at = self.line_end(anchor)?;
        self.text.insert_str(at, block);
        Ok(())
    }

    /// Insert `block` after the first `anchor` line that falls *within* the function/body
    /// beginning at `scope` (its first top-level `}`), scoping the idempotency check to that
    /// body too. Used where the same anchor (`match id {`) appears in two functions and each
    /// must be edited independently.
    pub fn insert_after_scoped(
        &mut self,
        scope: &str,
        anchor: &str,
        block: &str,
    ) -> Result<(), String> {
        let scope_start = self
            .text
            .find(scope)
            .ok_or_else(|| self.miss("scope", scope))?;
        let scope_end = self.text[scope_start..]
            .find("\n}\n")
            .map(|i| scope_start + i + 1)
            .unwrap_or(self.text.len());
        let body = &self.text[scope_start..scope_end];
        if body.contains(block.trim_end()) {
            return Ok(());
        }
        let anchor_rel = body.find(anchor).ok_or_else(|| self.miss("anchor", anchor))?;
        let nl = self.text[scope_start + anchor_rel..]
            .find('\n')
            .map(|i| scope_start + anchor_rel + i + 1)
            .ok_or_else(|| self.miss("line end of anchor", anchor))?;
        self.text.insert_str(nl, block);
        Ok(())
    }

    /// Replace exactly one occurrence of `from` with `to`. Idempotent: a no-op when `to` is
    /// already present and `from` is gone. Errors unless `from` appears exactly once, so a
    /// renamed or moved target is a loud failure rather than a silent partial edit.
    pub fn replace_once(&mut self, from: &str, to: &str) -> Result<(), String> {
        if !self.text.contains(from) && self.text.contains(to) {
            return Ok(());
        }
        let n = self.text.matches(from).count();
        if n != 1 {
            return Err(format!(
                "{}: expected exactly one `{}`, found {n}",
                self.path.display(),
                first_line(from)
            ));
        }
        self.text = self.text.replacen(from, to, 1);
        Ok(())
    }

    /// Append `"<feature>"` to the Cargo `default = [ … ]` array (so the feature ships in a
    /// stock build), idempotent on the quoted name.
    pub fn append_default_feature(&mut self, feature: &str) -> Result<(), String> {
        let quoted = format!("\"{feature}\"");
        let line_start = self.line_start("default = [")?;
        let line_end = self.text[line_start..]
            .find(']')
            .map(|i| line_start + i)
            .ok_or_else(|| self.miss("`]` closing", "default = ["))?;
        if self.text[line_start..line_end].contains(&quoted) {
            return Ok(());
        }
        self.text.insert_str(line_end, &format!(", {quoted}"));
        Ok(())
    }

    /// Write the buffer back to disk.
    pub fn save(&self) -> Result<(), String> {
        fs::write(&self.path, &self.text).map_err(|e| format!("write {}: {e}", self.path.display()))
    }

    /// Byte offset of the start of the line containing `needle`.
    fn line_start(&self, needle: &str) -> Result<usize, String> {
        let idx = self.find_unique(needle)?;
        Ok(self.text[..idx].rfind('\n').map(|i| i + 1).unwrap_or(0))
    }

    /// Byte offset just past the newline ending the line containing `needle`.
    fn line_end(&self, needle: &str) -> Result<usize, String> {
        let idx = self.find_unique(needle)?;
        Ok(self.text[idx..].find('\n').map(|i| idx + i + 1).unwrap_or(self.text.len()))
    }

    /// The byte offset of the *one* occurrence of `needle`. Loud on a miss (a moved anchor) and
    /// equally loud on an ambiguous match (the anchor appears more than once): silently taking the
    /// first hit would let a duplicated anchor land an edit at the wrong site, so an ambiguous
    /// anchor is a scaffolder bug to fix (disambiguate it) rather than a guess. The idempotency
    /// guards in [`Self::insert_before`]/[`Self::insert_after`] run before this, so a re-run that
    /// already inserted its block short-circuits and never reaches the count check.
    fn find_unique(&self, needle: &str) -> Result<usize, String> {
        let mut hits = self.text.match_indices(needle);
        let first = hits.next().ok_or_else(|| self.miss("anchor", needle))?.0;
        if hits.next().is_some() {
            return Err(format!(
                "{}: anchor `{}` is ambiguous (appears {} times) — disambiguate it so the edit \
                 lands at one site, never the first match",
                self.path.display(),
                first_line(needle),
                self.text.matches(needle).count(),
            ));
        }
        Ok(first)
    }

    fn miss(&self, kind: &str, needle: &str) -> String {
        format!(
            "{}: {kind} `{}` not found — the source moved; update the scaffolder",
            self.path.display(),
            first_line(needle)
        )
    }
}

/// The first line of a (possibly multi-line) needle, for error messages.
fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s).trim()
}

/// Write a brand-new file, erroring if it already exists so the scaffolder never clobbers a
/// feature a contributor has started filling in.
pub fn write_new(path: impl AsRef<Path>, contents: &str) -> Result<(), String> {
    let path = path.as_ref();
    if path.exists() {
        return Err(format!("{} already exists — refusing to overwrite", path.display()));
    }
    fs::write(path, contents).map_err(|e| format!("write {}: {e}", path.display()))
}
