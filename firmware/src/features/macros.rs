// SPDX-License-Identifier: GPL-2.0-or-later
//! [`feature!`] — the declarative front door for a compile-time [`Feature`].
//!
//! Every keeberry feature is the same three rote items wrapped around its real
//! behaviour: a **struct** holding the feature's state, one `'static` **singleton** of
//! that struct (the value the [`FEATURES`](crate::features::FEATURES) registry points
//! at), and an `impl Feature` whose [`id`](crate::features::Feature::id) /
//! [`name`](crate::features::Feature::name) / [`flags`](crate::features::Feature::flags)
//! are pure boilerplate. [`feature!`] writes those three from one declaration, so a
//! contributor (or an LLM) authors only the parts that carry meaning — the state the
//! feature owns and the hook bodies that act on it. `caps_word.rs` and `key_lock.rs` are
//! the worked retrofits.
//!
//! # What it deliberately does *not* do — registration stays explicit
//!
//! [`feature!`] does **not** add the feature to [`FEATURES`]. That array's order is the
//! fixed dispatch priority (a load-bearing invariant the linker cannot express — see the
//! [`FEATURES`](crate::features::FEATURES) doc for the exact ordering reasoning), so the
//! entry is written by hand, or stamped by the scaffolder at the
//! `// @scaffold:features-registry` anchor, at the correct position. An explicit registry
//! a reader greps top-to-bottom beats invisible link-time registration: `inventory` is a
//! silent no-op on this `cortex-m` target (its `.init_array` constructor never runs under
//! `cortex-m-rt`) and `linkme`'s link order is not source order, which would forfeit the
//! visible priority ordering. The full rationale is `.planning/sdk-llm-friendly.md` §5.
//!
//! # Grammar
//!
//! ```text
//! feature! {
//!     [#[doc = "..."] ...]              // optional outer attributes/docs for the struct
//!     <Struct> as <SINGLETON>,          // the feature type, and its `static` singleton's name
//!     id    = <expr>,                   // a `FeatureId` discriminant (the stable wire/persist id)
//!     name  = <expr>,                   // the &'static str shown in the auto-rendered Features panel
//!     flags = <expr>,                   // FEATURE_DEFAULT_ON [ | FEATURE_ALWAYS_ON ]
//!     state = { [#[doc...]] <field>: <Ty> = <const-init>, ... },  // the struct fields + initialisers
//!     hooks = { <fn ...> ... },         // the overridden `Feature` hook methods, verbatim
//! }
//! ```
//!
//! * **`state`** becomes the struct's (private) fields and the singleton's initialiser,
//!   one `field: Ty = init` per entry. Each `init` must be `const` (the singleton is a
//!   `static`), exactly as a hand-written singleton requires — an atomic or a blocking
//!   `Mutex` table, as the existing features use. A feature that owns no state writes
//!   `state = {}`.
//! * **`hooks`** is spliced **verbatim** as the body of `impl Feature for <Struct>`, so
//!   each entry is an ordinary hook method (`fn active`, `fn on_disable`, `fn on_report`,
//!   `fn on_matrix`, `fn on_kcp`, …) written exactly as it would be inside a normal
//!   `impl Feature for <Struct> { … }` block; the bodies resolve against the feature
//!   file's own `use` imports. List only the hooks the feature overrides — every other
//!   hook keeps its no-op default from the trait. Do **not** put `id`/`name`/`flags`
//!   here: the macro already emits them, and a duplicate is a compile error.
//!
//! Helper functions and a feature's own press-edge entry points (e.g. Caps Word's
//! `engage()`, Key Lock's `arm()`) stay in a normal `impl <Struct> { … }` block and free
//! functions outside the macro — only the registry-facing boilerplate moves inside it.
//!
//! # Example (the whole of Caps Word's registry wiring)
//!
//! ```ignore
//! feature! {
//!     /// Caps-word feature: holds Left Shift across one word once engaged.
//!     CapsWord as CAPS_WORD,
//!     id = FeatureId::CapsWord,
//!     name = "Caps Word",
//!     flags = FEATURE_DEFAULT_ON,
//!     state = {
//!         /// Whether caps-word is currently engaged. The `active()` fast-path flag.
//!         on: AtomicBool = AtomicBool::new(false),
//!     },
//!     hooks = {
//!         fn active(&self) -> bool { self.on.load(Ordering::Relaxed) }
//!         fn on_disable(&self) { self.end(); }
//!         fn on_report(&self, _c: &Ctx, mods: &mut u8, keys: &mut KeySet) { /* … */ }
//!     },
//! }
//! ```
//!
//! expands to `pub struct CapsWord { on: AtomicBool }`, the
//! `pub static CAPS_WORD: CapsWord = …` singleton, and the `impl Feature for CapsWord`
//! whose `id`/`name`/`flags` are filled in and whose three hooks are the ones written
//! above. The contributor then adds `&caps_word::CAPS_WORD` to [`FEATURES`] at the right
//! priority, and the feature is live.

/// Declare a compile-time [`Feature`](crate::features::Feature) from one block: emit its
/// singleton `struct`, its `static` registry value, and the rote `id`/`name`/`flags`
/// trait methods, leaving only the behaviour hooks to write. See the [module docs](self)
/// for the full grammar, the worked example, and why it does not auto-register.
///
/// In scope for the plugin modules via `#[macro_use] mod macros;` in
/// [`features`](crate::features). The `#[allow(unused_macros)]` covers a minimal
/// `--no-default-features` build, which compiles none of the feature modules that invoke
/// this macro and would otherwise flag it as an unused definition.
#[allow(unused_macros)]
macro_rules! feature {
    (
        $(#[$smeta:meta])*
        $struct:ident as $singleton:ident,
        id = $id:expr,
        name = $name:expr,
        flags = $flags:expr,
        state = { $( $(#[$fmeta:meta])* $field:ident : $fty:ty = $finit:expr ),* $(,)? },
        hooks = { $($hook:tt)* } $(,)?
    ) => {
        $(#[$smeta])*
        pub struct $struct {
            $( $(#[$fmeta])* $field: $fty, )*
        }

        /// The singleton in the [`FEATURES`](crate::features::FEATURES) registry.
        pub static $singleton: $struct = $struct {
            $( $field: $finit, )*
        };

        impl $crate::features::Feature for $struct {
            fn id(&self) -> $crate::features::FeatureId {
                $id
            }

            fn name(&self) -> &'static str {
                $name
            }

            fn flags(&self) -> u8 {
                $flags
            }

            $($hook)*
        }
    };
}
