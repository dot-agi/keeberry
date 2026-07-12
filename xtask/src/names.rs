// SPDX-License-Identifier: GPL-2.0-or-later
//! The five spellings of one feature name that the templates and the wiring substitute.
//!
//! A scaffolder's whole job is "one name into paths *and* contents under the right case
//! filter" (Yeoman / cargo-generate / cookiecutter all do exactly this), so every case a
//! keeberry site needs is derived once, here, from the single `PascalCase` argument.

/// Every spelling of a feature's name, derived from its `PascalCase` form.
pub struct Names {
    /// `DemoGizmo` — the Rust type, the `FeatureId` variant, the descriptor base name.
    pub pascal: String,
    /// `demo_gizmo` — the module, the Cargo feature, the file stem, the TS codec file.
    pub snake: String,
    /// `DEMO_GIZMO` — the registry singleton, the `CMD_*`/`group::` consts, the config-region prefix.
    pub screaming: String,
    /// `Demo Gizmo` — the human label (`Feature::name`, the panel title).
    pub display: String,
    /// `demoGizmo` — the camelCase TS `GroupName` member and the descriptor export symbol.
    pub camel: String,
}

impl Names {
    /// Derive every spelling from a `PascalCase` ASCII name (e.g. `DemoGizmo`), rejecting
    /// anything that is not a leading-upper alphanumeric identifier so a bad name fails loudly
    /// at the entry point rather than stamping a malformed file.
    pub fn from_pascal(pascal: &str) -> Result<Self, String> {
        let leads_upper = pascal.chars().next().is_some_and(|c| c.is_ascii_uppercase());
        if !leads_upper || !pascal.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!(
                "feature name must be PascalCase ASCII (e.g. `DemoGizmo`), got `{pascal}`"
            ));
        }
        let words = split_words(pascal);
        let snake = words.join("_").to_lowercase();
        Ok(Self {
            screaming: snake.to_uppercase(),
            display: words.join(" "),
            camel: lower_first(pascal),
            snake,
            pascal: pascal.to_string(),
        })
    }
}

/// Split a `PascalCase`/`camelCase` identifier into words at each upper-case boundary, so
/// `DemoGizmo` becomes `["Demo", "Gizmo"]`. A digit stays with the word it follows.
fn split_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_uppercase() && !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

/// `DemoGizmo` -> `demoGizmo`: lower-case only the first character.
fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}
