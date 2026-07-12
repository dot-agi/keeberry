// SPDX-License-Identifier: GPL-2.0-or-later
//! Build script. Two jobs:
//!
//! 1. Place `memory.x` on the linker search path so the `cortex-m-rt` `link.x`
//!    script can locate the FLASH/RAM region definitions.
//! 2. When the `autocorrect` feature is enabled, compile the typo→correction
//!    dictionary ([`DICT`]) into a flat trie emitted as `autocorrect_data.rs`
//!    (included by `features::autocorrect`). Generating the trie here — rather
//!    than hand-authoring the node array — mirrors QMK's
//!    `qmk generate-autocorrect-data`: the readable word list is the source of
//!    truth and the compact, prefix-shared trie is a build artefact, so the
//!    dictionary stays auditable while the firmware matches in O(word length).

use std::collections::BTreeMap;
use std::env;
use std::fmt::Write as _;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    // Only build the dictionary when the feature is on, so a minimal build pays
    // nothing for it. The module that `include!`s the file is itself `#[cfg]`-gated,
    // so the two are present together or absent together.
    if env::var_os("CARGO_FEATURE_AUTOCORRECT").is_some() {
        File::create(out.join("autocorrect_data.rs"))
            .unwrap()
            .write_all(generate_autocorrect().as_bytes())
            .unwrap();
    }

    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}

/// Common English typo → correction pairs, lowercase `a`–`z` only. Each is a whole
/// word the firmware corrects the moment it is typed in full (QMK's autocorrect
/// model). Derived forms are omitted on purpose: correcting the root re-forms them
/// (`recieve`→`receive`, then a typed `d` gives `received`). The list is curated to
/// hold words that are neither valid English nor a prefix of a common word, so a
/// completed match is never a false positive.
const DICT: &[(&str, &str)] = &[
    ("teh", "the"),
    ("recieve", "receive"),
    ("seperate", "separate"),
    ("definately", "definitely"),
    ("occured", "occurred"),
    ("untill", "until"),
    ("wich", "which"),
    ("thier", "their"),
    ("becuase", "because"),
    ("beleive", "believe"),
    ("accomodate", "accommodate"),
    ("acheive", "achieve"),
    ("adress", "address"),
    ("calender", "calendar"),
    ("cemetary", "cemetery"),
    ("concious", "conscious"),
    ("embarass", "embarrass"),
    ("enviroment", "environment"),
    ("existance", "existence"),
    ("foriegn", "foreign"),
    ("goverment", "government"),
    ("grammer", "grammar"),
    ("independant", "independent"),
    ("occassion", "occasion"),
    ("persistant", "persistent"),
    ("priviledge", "privilege"),
    ("recomend", "recommend"),
    ("refered", "referred"),
    ("relevent", "relevant"),
    ("succesful", "successful"),
    ("tommorow", "tomorrow"),
    ("truely", "truly"),
    ("unfortunatly", "unfortunately"),
    ("wierd", "weird"),
    ("writen", "written"),
    ("neccessary", "necessary"),
    ("occurance", "occurrence"),
    ("posession", "possession"),
    ("seige", "siege"),
    ("threshhold", "threshold"),
    ("tendancy", "tendency"),
    ("gaurd", "guard"),
    ("hieght", "height"),
    ("lenght", "length"),
    ("libary", "library"),
    ("mispell", "misspell"),
    ("noticable", "noticeable"),
    ("paralel", "parallel"),
    ("publically", "publicly"),
    ("rythm", "rhythm"),
    ("strenght", "strength"),
    ("suprise", "surprise"),
    ("yeild", "yield"),
];

/// HID keyboard usage (page `0x07`) of lowercase letter `c`: `a` = `0x04`.
fn usage(c: u8) -> u8 {
    c - b'a' + 0x04
}

/// One trie node under construction: its child edges (keyed by the HID usage of the
/// letter consumed to reach the child) and, when the path from the root to here
/// spells a complete typo, the index of its correction.
#[derive(Default)]
struct Node {
    children: BTreeMap<u8, usize>,
    corr: Option<usize>,
}

/// Compile [`DICT`] into the `autocorrect_data.rs` source: the flat trie node arrays,
/// the concatenated correction strings, and the two dimensioning constants the
/// firmware asserts against.
fn generate_autocorrect() -> String {
    // Insert every typo into a trie keyed by its letters in typed (root→leaf) order,
    // so the firmware walks the rolling buffer oldest-letter-first. Node 0 is the
    // root; a child / sibling index of 0 therefore doubles as the "none" sentinel.
    let mut nodes: Vec<Node> = vec![Node::default()];
    let mut corrections: Vec<Vec<u8>> = Vec::new();
    let mut max_len = 0usize;

    for (typo, corr) in DICT {
        max_len = max_len.max(typo.len()).max(corr.len());
        let mut cur = 0usize;
        for &c in typo.as_bytes() {
            let u = usage(c);
            cur = match nodes[cur].children.get(&u) {
                Some(&i) => i,
                None => {
                    let i = nodes.len();
                    nodes.push(Node::default());
                    nodes[cur].children.insert(u, i);
                    i
                }
            };
        }
        let cid = corrections.len();
        corrections.push(corr.bytes().map(usage).collect());
        nodes[cur].corr = Some(cid);
    }

    // Flatten each node's children into a singly-linked sibling chain: a node records
    // the letter on its in-edge, its first child, its next sibling, and its
    // correction id (biased by one, so 0 = none).
    let mut letter = vec![0u8; nodes.len()];
    let mut child = vec![0u16; nodes.len()];
    let mut sibling = vec![0u16; nodes.len()];
    let mut corr_id = vec![0u16; nodes.len()];
    for (i, node) in nodes.iter().enumerate() {
        let edges: Vec<(&u8, &usize)> = node.children.iter().collect();
        if let Some((_, &first)) = edges.first() {
            child[i] = first as u16;
        }
        for w in edges.windows(2) {
            sibling[*w[0].1] = *w[1].1 as u16;
        }
        for (&u, &c) in &node.children {
            letter[c] = u;
        }
        if let Some(cid) = node.corr {
            corr_id[i] = (cid + 1) as u16;
        }
    }

    // Concatenate the correction strings into one byte pool with a `[start, end]`
    // offset table, so a correction is a slice with no per-string pointer overhead.
    let mut corr_data: Vec<u8> = Vec::new();
    let mut corr_off: Vec<u16> = vec![0];
    for c in &corrections {
        corr_data.extend_from_slice(c);
        corr_off.push(corr_data.len() as u16);
    }

    let mut s = String::new();
    s.push_str("// @generated by build.rs from DICT — do not edit.\n");
    let _ = writeln!(s, "pub const AC_MAX_LEN: usize = {max_len};");
    let _ = writeln!(s, "pub const AC_ENTRY_COUNT: usize = {};", DICT.len());
    emit_u8(&mut s, "AC_LETTER", &letter);
    emit_u16(&mut s, "AC_CHILD", &child);
    emit_u16(&mut s, "AC_SIBLING", &sibling);
    emit_u16(&mut s, "AC_CORR_ID", &corr_id);
    emit_u8(&mut s, "AC_CORR_DATA", &corr_data);
    emit_u16(&mut s, "AC_CORR_OFF", &corr_off);
    s
}

/// Emit `static NAME: &[u8] = &[..];`.
fn emit_u8(s: &mut String, name: &str, data: &[u8]) {
    let _ = write!(s, "static {name}: &[u8] = &[");
    for v in data {
        let _ = write!(s, "{v},");
    }
    s.push_str("];\n");
}

/// Emit `static NAME: &[u16] = &[..];`.
fn emit_u16(s: &mut String, name: &str, data: &[u16]) {
    let _ = write!(s, "static {name}: &[u16] = &[");
    for v in data {
        let _ = write!(s, "{v},");
    }
    s.push_str("];\n");
}
