//! The glob and regex filter.
//!
//! Two rules, and they are not symmetric — this is the single most bug-prone
//! part of the traversal:
//!
//! - A **vector** of glob patterns, combined with **AND** semantics, matched
//!   against **root-relative** paths. Root-relative, not absolute: a `**/*.ts`
//!   must match `src/a.ts` when the root is `src`, and matching the absolute
//!   path would make every pattern depend on where the checkout lives.
//! - A **regex**, matched against the **full absolute path**, because that is
//!   what a regex written by a user almost always means.
//!
//! The reference implementation conflated these and globbed a single `String`.
//! Spec, including the glob-semantics matrix (nested vs. root-level `**/*.ts`,
//! `*.ts`, Windows separators): `.knowledge/implementation/code-rust-walker.md`.
//!
//! ## Why this is not one `GlobSet`
//!
//! The reference compiles every pattern into a single [`globset::GlobSet`] and
//! calls `is_match`. That is **OR**, and the specified semantics are **AND**
//! (`.knowledge/features/walk-traversal.md`: "when passed as an array, an entry
//! must match *all* patterns"). A `GlobSet` of `["**/*.ts", "**/*.rs"]` under
//! `is_match` accepts both a `.ts` and a `.rs` file; under the specified
//! semantics it accepts neither, because no path is both.
//!
//! So the positives are kept as individual matchers and `all`-ed. A `GlobSet`
//! is still the right structure for the **negatives** — `!`-prefixed patterns,
//! globby-style — because "excluded if it matches *any* exclusion" is exactly
//! the OR a `GlobSet` evaluates in one pass.

use std::path::Path;

use globset::{GlobBuilder, GlobMatcher, GlobSet, GlobSetBuilder};
use regex::Regex;

use crate::error::{Error, Result};

/// The compiled filter for one walk.
///
/// Compiled once per walk, never per entry: the reference's per-entry
/// `Glob::new` would re-parse the pattern 100 000 times on a 100 000-file tree.
#[derive(Debug, Default)]
pub struct Matcher {
    /// Non-negated globs. An entry must match **every** one of these.
    positive: Vec<GlobMatcher>,
    /// `!`-prefixed globs. An entry must match **none** of these.
    negative: Option<GlobSet>,
    /// Matched against the absolute path.
    regex: Option<Regex>,
}

impl Matcher {
    /// Compiles a filter.
    ///
    /// A glob pattern beginning with `!` is a negation, following globby. The
    /// `!` is stripped before compiling, so `!**/generated/**` excludes exactly
    /// what `**/generated/**` would have included.
    ///
    /// # Errors
    ///
    /// [`Error::Glob`] or [`Error::Regex`] if a pattern does not compile. Both
    /// carry the pattern as the caller wrote it — including the `!`, so the
    /// message matches what is in their source.
    pub fn new(globs: &[String], regex: Option<&str>) -> Result<Self> {
        let mut positive = Vec::new();
        let mut negatives = GlobSetBuilder::new();
        let mut has_negative = false;

        for pattern in globs {
            let (source, negated) = match pattern.strip_prefix('!') {
                Some(rest) => (rest, true),
                None => (pattern.as_str(), false),
            };

            // `literal_separator(true)` is load-bearing and is NOT globset's
            // default: out of the box `*` happily matches across `/`, so
            // `*.ts` would accept `src/a.ts`. fast-glob, globby and every
            // other glob a JavaScript caller has used treat `*` as
            // separator-bounded, and `**` as the way to opt out of that.
            // Without this line the distinction between `*.ts` and `**/*.ts`
            // does not exist, which makes half the matrix below meaningless.
            let glob = GlobBuilder::new(source)
                .literal_separator(true)
                .build()
                .map_err(|error| Error::Glob {
                    pattern: pattern.clone(),
                    message: error.to_string(),
                })?;

            if negated {
                negatives.add(glob);
                has_negative = true;
            } else {
                positive.push(glob.compile_matcher());
            }
        }

        let negative = if has_negative {
            Some(negatives.build().map_err(|error| Error::Glob {
                pattern: globs.join(", "),
                message: error.to_string(),
            })?)
        } else {
            None
        };

        let regex = regex
            .map(|source| {
                Regex::new(source).map_err(|error| Error::Regex {
                    pattern: source.to_owned(),
                    message: error.to_string(),
                })
            })
            .transpose()?;

        Ok(Self {
            positive,
            negative,
            regex,
        })
    }

    /// Whether this filter would accept everything.
    ///
    /// Lets the scanner skip the call entirely on an unfiltered walk, which is
    /// the common case for `walk()` with no options.
    #[must_use]
    pub fn is_unfiltered(&self) -> bool {
        self.positive.is_empty() && self.negative.is_none() && self.regex.is_none()
    }

    /// Whether an entry passes.
    ///
    /// `relative` is the path from the walk root; `absolute` is the full path.
    /// Passing both rather than deriving one here keeps the asymmetry explicit
    /// at the call site: the caller can see that the glob gets one and the
    /// regex gets the other.
    #[must_use]
    pub fn accepts(&self, relative: &Path, absolute: &Path) -> bool {
        let candidate = normalize(relative);
        let candidate = Path::new(candidate.as_ref());

        if !self.positive.iter().all(|glob| glob.is_match(candidate)) {
            return false;
        }

        if let Some(negative) = &self.negative {
            if negative.is_match(candidate) {
                return false;
            }
        }

        if let Some(regex) = &self.regex {
            if !regex.is_match(&absolute.to_string_lossy()) {
                return false;
            }
        }

        true
    }
}

/// Renders a root-relative path in the form glob patterns are written in.
///
/// Glob syntax uses `/`, and on Windows a path does not. Converting there is
/// what makes `**/*.ts` match `src\a.ts`; converting on Unix would be a bug,
/// because `\` is a legal character in a Unix filename and `a\b.ts` is one
/// file, not two components.
#[cfg(windows)]
fn normalize(path: &Path) -> std::borrow::Cow<'_, str> {
    match path.to_string_lossy() {
        std::borrow::Cow::Borrowed(s) if !s.contains('\\') => std::borrow::Cow::Borrowed(s),
        other => std::borrow::Cow::Owned(other.replace('\\', "/")),
    }
}

/// See the Windows variant: on Unix the path is already in glob form.
#[cfg(not(windows))]
fn normalize(path: &Path) -> std::borrow::Cow<'_, str> {
    path.to_string_lossy()
}

#[cfg(test)]
mod tests {
    use super::Matcher;
    use std::path::Path;

    fn globs(patterns: &[&str]) -> Vec<String> {
        patterns.iter().map(|s| (*s).to_owned()).collect()
    }

    /// Accepts a root-relative path, supplying a plausible absolute path for
    /// the regex half so the glob cases read as glob cases.
    fn accepts(matcher: &Matcher, relative: &str) -> bool {
        let absolute = Path::new("/repo").join(relative);
        matcher.accepts(Path::new(relative), &absolute)
    }

    // ── the glob-semantics matrix (task-2 AC #3) ────────────────────────────
    //
    // The 2025 draft matched globs against *absolute* paths. Glob patterns are
    // anchored and `*` does not cross `/`, so `**/*.ts` against `/repo/src/a.ts`
    // matched only root-level files. Each row below fails under that bug.

    #[test]
    fn double_star_matches_at_every_depth() {
        let matcher = Matcher::new(&globs(&["**/*.ts"]), None).unwrap();

        assert!(accepts(&matcher, "a.ts"), "root-level");
        assert!(accepts(&matcher, "src/a.ts"), "nested one deep");
        assert!(accepts(&matcher, "a/b/c/d.ts"), "nested four deep");
        assert!(!accepts(&matcher, "src/a.rs"), "wrong extension");
    }

    #[test]
    fn a_bare_star_does_not_cross_a_separator() {
        let matcher = Matcher::new(&globs(&["*.ts"]), None).unwrap();

        assert!(accepts(&matcher, "a.ts"), "root-level basename");
        assert!(
            !accepts(&matcher, "src/a.ts"),
            "`*` must not match across `/`"
        );
    }

    #[test]
    fn a_directory_prefix_anchors_at_the_root() {
        let matcher = Matcher::new(&globs(&["packages/*/src/**"]), None).unwrap();

        assert!(accepts(&matcher, "packages/path/src/index.ts"));
        assert!(!accepts(&matcher, "apps/docs/src/index.ts"));
    }

    #[test]
    fn a_brace_alternation_matches_either_extension() {
        let matcher = Matcher::new(&globs(&["**/*.{ts,tsx}"]), None).unwrap();

        assert!(accepts(&matcher, "src/a.ts"));
        assert!(accepts(&matcher, "src/a.tsx"));
        assert!(!accepts(&matcher, "src/a.js"));
    }

    /// On Windows the separator is `\` and glob syntax is `/`, so the relative
    /// path is normalised before matching. On Unix `\` is an ordinary filename
    /// character and must *not* be treated as a separator — the same assertion
    /// would be a bug there, so each platform asserts its own behaviour.
    #[test]
    fn separators_are_normalised_on_windows_and_literal_on_unix() {
        let matcher = Matcher::new(&globs(&["**/*.ts"]), None).unwrap();

        #[cfg(windows)]
        {
            let relative = Path::new(r"src\a.ts");
            assert!(matcher.accepts(relative, Path::new(r"C:\repo\src\a.ts")));
        }

        #[cfg(not(windows))]
        {
            // One file whose name contains a backslash. `**/*.ts` still matches
            // it as a basename, which is correct: it is a single component.
            let relative = Path::new(r"src\a.ts");
            assert_eq!(relative.components().count(), 1);
            assert!(matcher.accepts(relative, Path::new(r"/repo/src\a.ts")));

            // And it is *not* reachable as a two-component path.
            let nested = Matcher::new(&globs(&["src/*.ts"]), None).unwrap();
            assert!(!nested.accepts(relative, Path::new(r"/repo/src\a.ts")));
        }
    }

    // ── AND semantics, and negation ─────────────────────────────────────────

    /// The bug this guards: `GlobSet::is_match` is OR. Under OR this passes
    /// with either pattern; the specified semantics require both.
    #[test]
    fn several_globs_are_combined_with_and_not_or() {
        let matcher = Matcher::new(&globs(&["**/*.ts", "src/**"]), None).unwrap();

        assert!(accepts(&matcher, "src/a.ts"), "matches both");
        assert!(
            !accepts(&matcher, "test/a.ts"),
            "matches only the extension"
        );
        assert!(!accepts(&matcher, "src/a.rs"), "matches only the directory");
    }

    #[test]
    fn two_globs_that_cannot_both_match_accept_nothing() {
        let matcher = Matcher::new(&globs(&["**/*.ts", "**/*.rs"]), None).unwrap();

        assert!(!accepts(&matcher, "a.ts"));
        assert!(!accepts(&matcher, "a.rs"));
    }

    #[test]
    fn a_bang_prefix_excludes_like_globby() {
        let matcher = Matcher::new(&globs(&["**/*.ts", "!**/generated/**"]), None).unwrap();

        assert!(accepts(&matcher, "src/a.ts"));
        assert!(!accepts(&matcher, "src/generated/a.ts"));
    }

    #[test]
    fn a_negation_alone_excludes_from_everything_else() {
        let matcher = Matcher::new(&globs(&["!**/*.map"]), None).unwrap();

        assert!(accepts(&matcher, "src/a.ts"));
        assert!(!accepts(&matcher, "src/a.map"));
    }

    // ── regex is absolute, globs are relative ───────────────────────────────

    #[test]
    fn the_regex_sees_the_absolute_path() {
        let matcher = Matcher::new(&[], Some(r"^/repo/src/")).unwrap();

        assert!(matcher.accepts(Path::new("src/a.ts"), Path::new("/repo/src/a.ts")));
        assert!(!matcher.accepts(Path::new("lib/a.ts"), Path::new("/repo/lib/a.ts")));
    }

    #[test]
    fn a_glob_and_a_regex_are_combined_with_and() {
        let matcher = Matcher::new(&globs(&["**/*.ts"]), Some(r"\.test\.ts$")).unwrap();

        assert!(matcher.accepts(Path::new("src/a.test.ts"), Path::new("/repo/src/a.test.ts")));
        assert!(!matcher.accepts(Path::new("src/a.ts"), Path::new("/repo/src/a.ts")));
    }

    // ── compilation failures ────────────────────────────────────────────────

    #[test]
    fn an_unfiltered_matcher_accepts_everything_and_says_so() {
        let matcher = Matcher::new(&[], None).unwrap();

        assert!(matcher.is_unfiltered());
        assert!(accepts(&matcher, "literally/anything.xyz"));
    }

    #[test]
    fn a_bad_glob_reports_the_pattern_as_the_caller_wrote_it() {
        let error = Matcher::new(&globs(&["!src/**/["]), None).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("!src/**/["), "{message}");
    }

    #[test]
    fn a_bad_regex_reports_the_pattern() {
        let error = Matcher::new(&[], Some("(unclosed")).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("(unclosed"), "{message}");
    }
}
