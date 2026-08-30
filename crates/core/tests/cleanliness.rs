//! The core carries no emoji (unit 39, 2026-08-30) — two scans that can
//! actually fail for the property they claim.
//!
//! The platform-vocabulary scan beside this one cannot see an emoji and
//! cannot be made to: it matches runs of alphanumeric characters against a
//! word list, and an emoji is not alphanumeric, so it is a separator and
//! never a token. Claiming it as evidence would be a green run reading as
//! proof of something it never checked. These two scans are what replace
//! that empty claim.
//!
//! Both read PRODUCTION source only. Each file's trailing `#[cfg(test)]`
//! module is cut away before either scan looks, because test text
//! legitimately speaks other scripts and holds confusable characters on
//! purpose — the search guard's own confusable fixtures above all. The cut
//! is exact rather than approximate, and says so out loud: the marker it
//! cuts at is asserted to be the file's only one, at the top level, and
//! opening a module that runs to the end of the file. A cut anywhere else
//! would carry production code past both scans in silence.
//!
//! 1. THE CHARACTER SCAN. Every non-ASCII character in production source
//!    is one of an enumerated allowlist, each entry carrying the reason it
//!    belongs. A glyph entering the core fails loudly, and a new
//!    punctuation mark is a deliberate one-line addition here.
//! 2. THE ESCAPE SCAN. No `\u{...}` escape in production source names a
//!    codepoint an emoji can be spelled with, as [`EMOJI_CODEPOINTS`]
//!    enumerates them. The character scan alone would miss exactly the
//!    form the byte-hazard rule requires an emoji to be written in, so an
//!    emoji list smuggled into the core as escapes would pass it.
//!
//! Each scan carries a deliberately-failing fixture, run through the same
//! predicate the scan uses, so a green run proves the scan bites.

use std::fs;
use std::path::{Path, PathBuf};

/// One allowed non-ASCII character in production core source, with the
/// reason it belongs. Enumerated against the tree: this list is the whole
/// of what the core may carry, and growing it is a deliberate act with a
/// reason attached.
struct Allowed {
    character: char,
    reason: &'static str,
}

/// Every non-ASCII character production core source may carry.
const ALLOWLIST: &[Allowed] = &[
    Allowed {
        character: '\u{2014}',
        reason: "the em dash the prose of every module is written with",
    },
    Allowed {
        character: '\u{2500}',
        reason: "the box-drawing rule that separates a module's sections",
    },
    Allowed {
        character: '\u{2026}',
        reason: "the ellipsis a truncated lookup result and its prose use",
    },
    Allowed {
        character: '\u{00B7}',
        reason: "a middle dot: one of the search guard's collapse separators, \
                 which the handle grammar must read as a separator",
    },
    Allowed {
        character: '\u{2027}',
        reason: "a hyphenation point: a second search-guard collapse separator",
    },
    Allowed {
        character: '\u{2022}',
        reason: "a bullet: the third search-guard collapse separator",
    },
];

/// One codepoint range the escape scan refuses, inclusive, with the reason
/// it is here — the same enumerate-with-a-reason discipline the character
/// allowlist keeps, read the other way round.
struct Refused {
    low: u32,
    high: u32,
    reason: &'static str,
}

/// Every codepoint an emoji may be spelled with, as far as this scan
/// claims: the two large emoji blocks, the joining and selecting
/// codepoints a compound entry is built from, and the emoji-capable
/// singletons that sit outside both blocks. That last group is why this
/// list is not "two blocks" — a smuggled list could carry a star, a
/// keycap, a legal symbol or a tag-sequence flag, and a scan that missed
/// them would pass exactly the entries a reader would least expect it to.
///
/// It is NOT every codepoint Unicode gives the Emoji property to: that
/// property covers bare digits and letters, which production source is
/// made of. The scan's claim is bounded to what it enumerates, and a new
/// platform list reaching outside it is a deliberate line added here.
///
/// The search guard's own format-control escapes lie outside the whole
/// set, so the scan does not have to except them.
const EMOJI_CODEPOINTS: &[Refused] = &[
    Refused {
        low: 0x1F000,
        high: 0x1FAFF,
        reason: "the emoji blocks proper: pictographs, faces, symbols and the \
                 regional indicators a flag is built from",
    },
    Refused {
        low: 0x2600,
        high: 0x27BF,
        reason: "miscellaneous symbols and dingbats, which every platform reaction \
                 set draws on",
    },
    Refused {
        low: 0x200D,
        high: 0x200D,
        reason: "the zero-width joiner: how a compound entry is spelled",
    },
    Refused {
        low: 0xFE0F,
        high: 0xFE0F,
        reason: "the variation selector: a selector-carrying escape is an emoji \
                 spelling and nothing else",
    },
    Refused {
        low: 0x20E3,
        high: 0x20E3,
        reason: "the combining keycap, which turns a digit escape into an emoji",
    },
    Refused {
        low: 0x2B00,
        high: 0x2BFF,
        reason: "arrows and stars: the block holding the star, the circles and the \
                 squares platform sets carry",
    },
    Refused {
        low: 0x00A9,
        high: 0x00A9,
        reason: "the copyright sign, an emoji-capable singleton",
    },
    Refused {
        low: 0x00AE,
        high: 0x00AE,
        reason: "the registered sign, an emoji-capable singleton",
    },
    Refused {
        low: 0x2122,
        high: 0x2122,
        reason: "the trade mark sign, an emoji-capable singleton",
    },
    Refused {
        low: 0x3030,
        high: 0x3030,
        reason: "the wavy dash, an emoji-capable singleton outside every block above",
    },
    Refused {
        low: 0x303D,
        high: 0x303D,
        reason: "the part alternation mark, likewise",
    },
    Refused {
        low: 0x3297,
        high: 0x3299,
        reason: "the congratulation and secret ideographs, likewise",
    },
    Refused {
        low: 0xE0020,
        high: 0xE007F,
        reason: "the tag block: how a subdivision flag spells its region",
    },
];

/// Whether the character is outside the allowlist — the character scan's
/// whole predicate, so the failing fixture below exercises exactly what
/// the scan runs.
fn is_forbidden_character(character: char) -> bool {
    !character.is_ascii()
        && !ALLOWLIST
            .iter()
            .any(|allowed| allowed.character == character)
}

/// The entry that refuses this codepoint, `None` for one the scan admits
/// — the escape scan's whole predicate, answering WHICH reason it failed
/// for so the failure line carries it.
fn refused_emoji(codepoint: u32) -> Option<&'static Refused> {
    EMOJI_CODEPOINTS
        .iter()
        .find(|refused| (refused.low..=refused.high).contains(&codepoint))
}

/// Every `\u{...}` escape one line spells, as codepoints. A malformed or
/// non-hexadecimal escape yields nothing: it is not source that compiles,
/// and this scan judges emoji, not syntax.
fn escaped_codepoints(line: &str) -> Vec<u32> {
    line.match_indices("\\u{")
        .filter_map(|(at, _)| {
            let rest = &line[at + 3..];
            let end = rest.find('}')?;
            u32::from_str_radix(&rest[..end], 16).ok()
        })
        .collect()
}

/// Every production `.rs` file under the core's source tree, collected
/// recursively so a new module joins both scans by existing.
fn production_files() -> Vec<PathBuf> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rust_files(&source, &mut files);
    assert!(
        files.iter().any(|file| file.ends_with("lib.rs")),
        "the scans must reach the crate's sources"
    );
    files.sort();
    files
}

fn collect_rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("the source directory {} lists: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("a listable directory entry").path();
        if path.is_dir() {
            collect_rust_files(&path, into);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            into.push(path);
        }
    }
}

/// One file's production lines, numbered from one: everything ahead of the
/// trailing test module's `#[cfg(test)]` marker.
///
/// The cut hides nothing, and the three facts that make that true are
/// asserted rather than assumed — a cut at a marker that is NOT the file's
/// trailing test module would carry production code past the scans in
/// silence, which is the one way a scan like this rots:
///
/// 1. At most one marker in the file. A second would mean an inline test
///    module elsewhere, and the cut at the first would hide everything
///    behind it.
/// 2. The marker sits at the top level and introduces a module. An
///    indented one is a module nested inside something else, with that
///    something else's code still to come.
/// 3. The file's last non-blank line closes that module at the top level.
///    Together with the two above, the module the cut removes runs to the
///    end of the file, so nothing production was cut with it.
fn production_lines(path: &Path) -> Vec<(usize, String)> {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("the source file {} reads: {e}", path.display()));
    let markers: Vec<usize> = content
        .lines()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with("#[cfg(test)]"))
        .map(|(index, _)| index)
        .collect();
    assert!(
        markers.len() <= 1,
        "{}: {} test-module markers; the production cut assumes at most one, at the tail",
        path.display(),
        markers.len()
    );
    if let Some(&marker) = markers.first() {
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(
            lines[marker],
            "#[cfg(test)]",
            "{}:{}: the marker is indented, so it introduces a nested module and the cut \
             would hide the production code around it",
            path.display(),
            marker + 1
        );
        let introduced = lines.get(marker + 1).copied().unwrap_or_default();
        assert!(
            introduced.starts_with("mod ") && introduced.trim_end().ends_with('{'),
            "{}:{}: the marker introduces {introduced:?} instead of a top-level module; \
             the cut assumes it opens the trailing test module",
            path.display(),
            marker + 2
        );
        let last = lines
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .copied()
            .unwrap_or_default();
        assert_eq!(
            last,
            "}",
            "{}: the file does not end by closing a top-level block, so the module the \
             cut removes may not run to the end of the file",
            path.display()
        );
    }
    content
        .lines()
        .enumerate()
        .take_while(|(_, line)| !line.trim_start().starts_with("#[cfg(test)]"))
        .map(|(index, line)| (index + 1, line.to_owned()))
        .collect()
}

/// The character scan: production core source carries no non-ASCII
/// character outside the enumerated allowlist. The failure names the file,
/// the line and the codepoint, so a glyph that slipped in is found without
/// a second search.
#[test]
fn the_core_carries_no_glyph_outside_the_allowlist() {
    let mut findings = Vec::new();
    for file in production_files() {
        for (number, line) in production_lines(&file) {
            for character in line.chars().filter(|c| is_forbidden_character(*c)) {
                findings.push(format!(
                    "{}:{number}: U+{:04X} {character:?}",
                    file.display(),
                    character as u32
                ));
            }
        }
    }
    assert!(
        findings.is_empty(),
        "a non-ASCII character outside the allowlist entered the core:\n{}\n\
         If it belongs, add it to ALLOWLIST with the reason it does.",
        findings.join("\n")
    );
}

/// The escape scan: production core source spells no emoji codepoint as an
/// escape sequence either — no codepoint of [`EMOJI_CODEPOINTS`], which is
/// the whole of what this scan claims. It is the scan that would catch a
/// platform's reaction list smuggled into the core in the one form the
/// byte-hazard rule requires such a list to be written in.
#[test]
fn the_core_spells_no_emoji_as_an_escape() {
    let mut findings = Vec::new();
    for file in production_files() {
        for (number, line) in production_lines(&file) {
            for (codepoint, refused) in escaped_codepoints(&line)
                .into_iter()
                .filter_map(|codepoint| Some((codepoint, refused_emoji(codepoint)?)))
            {
                findings.push(format!(
                    "{}:{number}: \\u{{{codepoint:04X}}} — {}",
                    file.display(),
                    refused.reason
                ));
            }
        }
    }
    assert!(
        findings.is_empty(),
        "an emoji escape entered the core:\n{}\n\
         Which emoji a platform can carry is a platform fact and belongs in an adapter.",
        findings.join("\n")
    );
}

/// The character scan bites: a line carrying a glyph is rejected by the
/// same predicate the scan runs, while the allowlist's own characters and
/// plain ASCII pass. Without this a green scan would prove nothing about a
/// predicate that had quietly stopped refusing anything.
#[test]
fn the_character_scan_refuses_a_glyph_and_admits_the_allowlist() {
    let smuggled = "const CHEER: &str = \"\u{1F389}\";";
    assert!(
        smuggled.chars().any(is_forbidden_character),
        "the deliberately-failing fixture must be refused: {smuggled}"
    );
    for allowed in ALLOWLIST {
        assert!(
            !is_forbidden_character(allowed.character),
            "the allowlist admits U+{:04X} — {}",
            allowed.character as u32,
            allowed.reason
        );
    }
    assert!(
        !"a plain ascii line;".chars().any(is_forbidden_character),
        "ordinary source passes"
    );
}

/// The escape scan bites: a line spelling an emoji as an escape is
/// rejected by the same reading the scan runs — a single codepoint, a
/// joined sequence and a selector-carrying form alike — while the escapes
/// production source legitimately holds pass. The zero-width space in the
/// search guard's own documentation is the live case that must not trip.
#[test]
fn the_escape_scan_refuses_an_emoji_escape_and_admits_the_others() {
    for smuggled in [
        r#"const SEEN: &str = "\u{1F440}";"#,
        r#"const CHEER: &str = "\u{2764}\u{200D}\u{1F525}";"#,
        r#"const HEART: &str = "\u{2764}\u{FE0F}";"#,
        r#"const LIST: [&str; 2] = ["\u{1F44D}", "\u{1F44E}"];"#,
        // The entries outside the two blocks, which the enumerated
        // singletons are here for: a star, a keycap, a legal symbol and a
        // tag-sequence flag.
        r#"const STAR: &str = "\u{2B50}";"#,
        r#"const KEYCAP: &str = "5\u{20E3}";"#,
        r#"const MARK: &str = "\u{2122}";"#,
        r#"const FLAG: &str = "\u{1F3F4}\u{E0067}\u{E007F}";"#,
    ] {
        assert!(
            escaped_codepoints(smuggled)
                .into_iter()
                .any(|codepoint| refused_emoji(codepoint).is_some()),
            "the deliberately-failing fixture must be refused: {smuggled}"
        );
    }
    for legitimate in [
        r"an invisible character: `word\u{200B}@handle` reads as an email",
        r#"const DASH: &str = "\u{2014}";"#,
        "no escape at all",
    ] {
        assert!(
            !escaped_codepoints(legitimate)
                .into_iter()
                .any(|codepoint| refused_emoji(codepoint).is_some()),
            "source that carries no emoji escape passes: {legitimate}"
        );
    }
    assert_eq!(
        escaped_codepoints(r#""\u{2764}\u{200D}\u{1F525}""#),
        vec![0x2764, 0x200D, 0x1F525],
        "every escape on a line is read, not only the first"
    );
}
