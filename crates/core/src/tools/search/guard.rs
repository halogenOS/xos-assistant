//! The search query's person guard (decided 2026-08-27): a query carrying a
//! DELIBERATE person reference is refused whole, and nothing is sent.
//!
//! The rule is a form, not a list. A person reference is the handle form —
//! an at sign starting a token, followed by a name — and the tool refuses
//! it whoever it belongs to, because a member list cannot answer the
//! question: the principals table is adapter-scoped and holds only people
//! who spoke, so a check against it would miss exactly the mentioned
//! bystanders the guard exists for. The form is platform-neutral by
//! construction: it covers a bare handle and a federated one alike, and no
//! platform vocabulary enters the core.
//!
//! The grammar, operationally:
//!
//! - a NAME CHARACTER is a letter, a digit or an underscore;
//! - a CANDIDATE TOKEN is an at sign followed by name characters of which
//!   the FIRST is a letter — so a version pin like `package@1.2.3` is
//!   outside the grammar itself instead of being a special case;
//! - an at sign PRECEDED by a name character IN THE QUERY AS WRITTEN is an
//!   email address's and is never a candidate — which is what lets
//!   `a.duffy@example.com` through whole, dotted local part and all;
//! - a candidate token ENDED by a slash is a scoped package name
//!   (`@scope/package`), not a person.
//!
//! Matching runs on a normalised view so spaced, dotted and zero-width
//! evasion is one token with the plain form: formatting characters are
//! dropped, and a single separator between the name characters of a
//! candidate token is collapsed, so `@ h a n d l e`, `@h.a.n.d.l.e` and
//! `@handle` all read as one candidate. The slash is deliberately NOT a
//! collapse separator: it is what ENDS a candidate, and collapsing it would
//! swallow the scoped-package exception. Case folding is part of the
//! normalised view's definition and is a no-op for this grammar, which
//! tests character CLASSES and never a literal — stated here instead of
//! performed, so nobody goes looking for a fold that would change nothing.
//!
//! The email exception alone reads the query AS WRITTEN and never the
//! normalised view. Every other part of the grammar widens what is refused,
//! so reading the normalised view can only refuse more; this one part
//! narrows it, and on the normalised view an invisible character between a
//! word and an at sign would fuse the two — `word\u{200B}@handle` would read
//! as an email's local part and the handle would travel to the vendor. The
//! character immediately before the at sign in the ORIGINAL text is what
//! decides, so an invisible character there means the at sign begins a
//! token and the handle form is refused.
//!
//! Normalisation is applied to FIND a token and never to the whole query as
//! one string, so word boundaries survive and a single ordinary word is
//! never matched, however exactly it happens to equal somebody's handle:
//! the operator's rule of 2026-08-25 — a search for a word somebody happens
//! to be called is chance, not a submission of personal data.
//!
//! What is sent to the vendor is always the query as written, or nothing.
//! The guard reads; it never rewrites.
//!
//! Confusable folding was rejected with the unit: a UTS-39 table is a new
//! dependency against an adversary this guard does not face — the query's
//! author is our own model, the failure mode is carelessness, and no
//! lexical rule stops a model that paraphrases a person instead. The guard
//! is a discipline device and is recorded as one.

/// One character a name may be made of: a letter, a digit or an underscore.
/// Unicode-wide on purpose — a handle written in another script is a handle.
fn name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// A formatting character the normalised view drops outright: the
/// zero-width marks, every bidirectional control, the word joiner and the
/// soft hyphen — the invisible padding a spelled-out handle hides in.
/// Dropped instead of being treated as a separator, because they are not
/// visible separation at all.
///
/// The bidirectional controls are covered WHOLE: the embedding and override
/// range (U+202A..U+202E), the isolates (U+2066..U+2069) that replaced them
/// and that modern keyboards emit for right-to-left text, and the Arabic
/// letter mark (U+061C). Half the family is worse than none — a handle
/// padded with an isolate would read as prose while the same handle padded
/// with an override was refused.
fn ignorable(c: char) -> bool {
    matches!(c,
        '\u{00AD}'
        | '\u{061C}'
        | '\u{180E}'
        | '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{2069}'
        | '\u{FEFF}')
}

/// A separator the normalised view collapses INSIDE a candidate token:
/// whitespace and the visible padding a spelled-out handle uses. The slash
/// is not one and must never become one — it ends a candidate token, which
/// is what keeps a scoped package name out of the guard.
fn collapse_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '.' | ',' | '-' | '·' | '‧' | '•')
}

/// Whether the query carries a deliberate person reference — the handle
/// form of the module's grammar. `true` refuses the query whole; the
/// caller's refusal names the rule and never the token.
#[must_use]
pub(crate) fn carries_person_reference(query: &str) -> bool {
    let (chars, email_preceded) = normalised(query);
    (0..chars.len()).any(|at| chars[at] == '@' && !email_preceded[at] && candidate_at(&chars, at))
}

/// The normalised view of one query: every character that is not a
/// formatting character, beside the one fact the email exception needs from
/// the text as written — whether the character immediately before this one
/// IN THE ORIGINAL QUERY was a name character. The two vectors share their
/// indices. The flag is carried per character because dropping the
/// formatting characters destroys exactly the adjacency it states.
fn normalised(query: &str) -> (Vec<char>, Vec<bool>) {
    let mut chars = Vec::new();
    let mut email_preceded = Vec::new();
    let mut previous = None;
    for character in query.chars() {
        if !ignorable(character) {
            chars.push(character);
            email_preceded.push(previous.is_some_and(name_char));
        }
        previous = Some(character);
    }
    (chars, email_preceded)
}

/// Whether the at sign at `at` starts a candidate token, under the
/// separator collapse and the scoped-package exception.
fn candidate_at(chars: &[char], at: usize) -> bool {
    let mut cursor = at + 1;
    let mut first = true;
    loop {
        // At most one separator carries the token across visible padding.
        let mut next = cursor;
        if next < chars.len() && collapse_separator(chars[next]) {
            next += 1;
        }
        let Some(&c) = chars.get(next) else { break };
        if !name_char(c) {
            break;
        }
        if first {
            // The first name character must be a letter: an at sign
            // directly in front of a digit is a version pin, not a person.
            if !c.is_alphabetic() {
                return false;
            }
            first = false;
        }
        cursor = next + 1;
    }
    if first {
        // Nothing nameable followed the at sign at all.
        return false;
    }
    // A token ended by a slash is a scoped package name.
    chars.get(cursor) != Some(&'/')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `AC7b`: the deliberate handle form, in every evasion the normalised
    /// view exists for — bare, spaced out, dotted, dashed, padded with a
    /// zero-width mark, a soft hyphen or any bidirectional control (the
    /// isolates included, not only the older overrides), mixed case, and
    /// mid-sentence.
    #[test]
    fn the_deliberate_handle_form_is_refused_however_it_is_spelled() {
        for query in [
            "@handle",
            "who is @handle",
            "@ h a n d l e",
            "@h.a.n.d.l.e",
            "@h-a-n-d-l-e",
            "@han\u{200b}dle",
            "@\u{200b}handle",
            "@han\u{00ad}dle",
            "@\u{2066}handle",
            "@\u{2068}handle\u{2069}",
            "@\u{2069}handle",
            "@\u{061c}handle",
            "@\u{180e}handle",
            "@\u{202a}handle",
            "ask \u{2066}@han\u{2069}dle\u{2066} about the build",
            "@HaNdLe",
            "what did @handle say about the kernel",
            "@handle_2 and the build",
            "@пользователь",
            // The email exception judged as written: an invisible character
            // between a word and the at sign is not a local part, so the
            // handle is still a handle.
            "word\u{200b}@handle",
            "word\u{00ad}@handle",
            "word\u{2066}@handle",
            "word\u{feff}@handle",
        ] {
            assert!(
                carries_person_reference(query),
                "the handle form must be refused: {query:?}"
            );
        }
    }

    /// `AC7d`, carrying the same weight: an ordinary search passes untouched.
    /// A single common word that happens to be somebody's handle is chance,
    /// an email address with a dotted local part is an address, a scoped
    /// package is a package and a version pin is a version.
    #[test]
    fn ordinary_queries_pass_including_the_pinned_exceptions() {
        for query in [
            "duffy",
            "sparrow",
            "how do I flash a device",
            "a.duffy@example.com",
            "duffy@example.com",
            "mail me at a.duffy@example.com about the build",
            "@scope/package",
            "@angular/core changelog",
            "package@1.2.3",
            "install package@2.0.0-beta.1",
            "the price is 30 @ 2 each",
            "email@example.co.uk",
            "a plain question with no at sign at all",
        ] {
            assert!(
                !carries_person_reference(query),
                "an ordinary query must pass: {query:?}"
            );
        }
    }

    /// The exceptions are the grammar's own edges, pinned one by one so a
    /// later edit cannot quietly turn one of them into a refusal — or a
    /// refusal into a pass.
    #[test]
    fn the_grammar_edges_hold() {
        assert!(
            !carries_person_reference("@"),
            "an at sign with nothing nameable behind it names nobody"
        );
        assert!(
            !carries_person_reference("@ "),
            "an at sign and a space name nobody"
        );
        assert!(
            !carries_person_reference("@/scope"),
            "a slash straight after the at sign starts no name"
        );
        assert!(
            !carries_person_reference("@1handle"),
            "a digit first is the version-pin case, by the grammar itself"
        );
        assert!(
            !carries_person_reference("@handle/repo"),
            "a scoped name is a package path"
        );
        assert!(
            !carries_person_reference("@handle/"),
            "the exception is the slash itself, as the unit spells it: an \
             at-name followed by a slash is a scoped package, and the guard \
             does not go on to ask what follows the slash"
        );
    }

    /// The email exception reads the query AS WRITTEN, and that is the whole
    /// point: an invisible character before the at sign fuses a word to a
    /// handle only in the normalised view, and a view-side reading would let
    /// the handle travel to the vendor as somebody's local part. Both sides
    /// are pinned together, because the exception exists for the address and
    /// must keep letting it through.
    #[test]
    fn an_invisible_character_before_the_at_sign_is_no_local_part() {
        assert!(
            carries_person_reference("word\u{200b}@handle"),
            "a zero-width space before the at sign is not an email's local part"
        );
        assert!(
            !carries_person_reference("a.duffy@example.com"),
            "an address written as an address still passes whole"
        );
    }
}
