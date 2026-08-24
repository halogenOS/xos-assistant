//! The model's markdown, rendered as the formatting this platform speaks.
//!
//! The core hands the adapter prose. Models write that prose in markdown
//! because that is what they write in, and the platform renders none of it —
//! so bold arrived as asterisks and code fences arrived as backticks, in
//! front of the whole group.
//!
//! Turning the platform's own parse mode on is not enough by itself, and is
//! worse than nothing done carelessly: the API rejects a message whose
//! entities it cannot parse, and a rejected send is a lost answer. Handing it
//! prose written by a model — which may contain a stray `<`, an unpaired
//! asterisk, or an underscore inside an identifier — is exactly how that
//! rejection happens.
//!
//! So the conversion is done here rather than delegated: this module reads a
//! bounded subset of markdown and writes the platform's HTML, escaping
//! everything it does not recognise. Two properties make it safe to send.
//!
//! **It only ever emits balanced tags.** Every opener is written only once its
//! closer has been found, so a truncated chunk or an unpaired marker degrades
//! to literal text rather than to broken markup. A converter that emitted an
//! opener hopefully would produce exactly the rejection this module exists to
//! avoid.
//!
//! **It escapes first and inserts second.** Text reaches the platform with
//! `&`, `<` and `>` already replaced, so a member quoting HTML at the
//! assistant cannot close a tag the assistant opened.
//!
//! The subset is what a chat answer actually uses: fenced code, inline code,
//! bold, italic and links. Lists, headings and tables have no platform
//! markup, so their source characters are left as the text they already read
//! as. Anything else is text.

/// Escape the three characters the platform's HTML mode reserves.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Render one markdown chunk as the platform's HTML.
///
/// Chunking happens before this runs, on the markdown, so a long answer is
/// split by the platform's length rule and each piece converted on its own. A
/// marker split across two chunks simply fails to pair and stays literal,
/// which is the degradation this module is built to make safe.
pub(crate) fn to_html(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len() + markdown.len() / 8);
    let mut rest = markdown;

    while !rest.is_empty() {
        // Fenced code first: nothing inside a fence is markup.
        if let Some(after) = rest.strip_prefix("```")
            && let Some(end) = after.find("```")
        {
            let body = &after[..end];
            // A fence may open with a language tag on the first line;
            // the platform has no place for it, so it is dropped rather
            // than printed as content.
            let body = match body.split_once('\n') {
                Some((first, remainder)) if !first.contains(' ') && !first.is_empty() => remainder,
                _ => body,
            };
            out.push_str("<pre>");
            out.push_str(&escape(body));
            out.push_str("</pre>");
            rest = &after[end + 3..];
            continue;
        }
        if let Some(after) = rest.strip_prefix('`')
            && let Some(end) = after.find('`')
        {
            out.push_str("<code>");
            out.push_str(&escape(&after[..end]));
            out.push_str("</code>");
            rest = &after[end + 1..];
            continue;
        }
        if let Some(after) = rest.strip_prefix('[')
            && let Some((label, remainder)) = link(after)
        {
            out.push_str(&label);
            rest = remainder;
            continue;
        }
        if let Some(after) = rest.strip_prefix("**")
            && let Some(end) = after.find("**")
        {
            out.push_str("<b>");
            out.push_str(&to_html(&after[..end]));
            out.push_str("</b>");
            rest = &after[end + 2..];
            continue;
        }
        if let Some((marker, after)) = rest
            .strip_prefix('*')
            .map(|after| ('*', after))
            .or_else(|| rest.strip_prefix('_').map(|after| ('_', after)))
        {
            // An underscore inside a word — snake_case, a file name — is not
            // emphasis, and treating it as such is how identifiers arrive
            // mangled.
            let inside_word =
                marker == '_' && out.chars().next_back().is_some_and(char::is_alphanumeric);
            if !inside_word && let Some(end) = after.find(marker) {
                let body = &after[..end];
                if !body.is_empty() && !body.starts_with(char::is_whitespace) {
                    out.push_str("<i>");
                    out.push_str(&to_html(body));
                    out.push_str("</i>");
                    rest = &after[end + 1..];
                    continue;
                }
            }
        }
        // Nothing matched: take one character as text. Taking a whole run
        // would be faster and would also have to re-implement the checks
        // above to know where the run ends.
        let mut characters = rest.chars();
        let character = characters.next().expect("the remainder is not empty");
        out.push_str(&escape(&character.to_string()));
        rest = characters.as_str();
    }

    out
}

/// One `[label](target)` from just past its opening bracket, as rendered
/// anchor plus the remainder. `None` when it does not close, in which case
/// the bracket is text like any other character.
fn link(after_bracket: &str) -> Option<(String, &str)> {
    let close = after_bracket.find(']')?;
    let remainder = &after_bracket[close + 1..];
    let after_paren = remainder.strip_prefix('(')?;
    let end = after_paren.find(')')?;
    let target = &after_paren[..end];
    // Only the two schemes a chat answer legitimately links with. Anything
    // else — `javascript:`, a bare word, an empty target — renders as the
    // text it was, which is honest and unclickable.
    if !(target.starts_with("https://") || target.starts_with("http://")) {
        return None;
    }
    let label = to_html(&after_bracket[..close]);
    Some((
        format!("<a href=\"{}\">{label}</a>", escape(target)),
        &after_paren[end + 1..],
    ))
}

#[cfg(test)]
mod tests {
    use super::to_html;

    /// The shapes a chat answer actually carries, each rendered as the
    /// platform's own markup rather than printed as source.
    #[test]
    fn the_subset_renders_as_platform_markup() {
        assert_eq!(to_html("**Proxmox VE**"), "<b>Proxmox VE</b>");
        assert_eq!(to_html("*maybe*"), "<i>maybe</i>");
        assert_eq!(to_html("_maybe_"), "<i>maybe</i>");
        assert_eq!(to_html("`nix build`"), "<code>nix build</code>");
        assert_eq!(
            to_html("```sh\nnix build\n```"),
            "<pre>nix build\n</pre>",
            "the language tag has no place to go and is not printed as content"
        );
        assert_eq!(
            to_html("[the wiki](https://example.org/a)"),
            "<a href=\"https://example.org/a\">the wiki</a>"
        );
        assert_eq!(
            to_html("**bold with `code`**"),
            "<b>bold with <code>code</code></b>",
            "emphasis carries its own markup inside it"
        );
    }

    /// The reason this module exists rather than a parse mode being switched
    /// on: everything it does not recognise has to reach the platform as
    /// text, or the send is rejected and the answer is lost.
    #[test]
    fn everything_unrecognised_degrades_to_text() {
        assert_eq!(
            to_html("an unpaired **marker"),
            "an unpaired **marker",
            "a marker with no closer stays literal instead of opening a tag"
        );
        assert_eq!(
            to_html("2 * 3 * 4"),
            "2 * 3 * 4",
            "a spaced asterisk is arithmetic, not emphasis"
        );
        assert_eq!(
            to_html("snake_case_name"),
            "snake_case_name",
            "an underscore inside a word is an identifier, not emphasis"
        );
        assert_eq!(
            to_html("[label](javascript:alert(1))"),
            "[label](javascript:alert(1))",
            "only http and https render as links; the rest is text"
        );
        assert_eq!(
            to_html("a < b & c > d"),
            "a &lt; b &amp; c &gt; d",
            "the reserved characters are escaped wherever they appear"
        );
        assert_eq!(
            to_html("<b>not mine</b>"),
            "&lt;b&gt;not mine&lt;/b&gt;",
            "a member quoting markup cannot close a tag the assistant opened"
        );
        assert_eq!(
            to_html("`<script>`"),
            "<code>&lt;script&gt;</code>",
            "code content is escaped too"
        );
    }

    /// A chunk boundary can fall anywhere, including mid-marker. Whatever it
    /// produces has to be sendable on its own.
    #[test]
    fn a_truncated_chunk_still_balances() {
        for cut in 1.."**bold** and `code`".len() {
            let piece = &"**bold** and `code`"[..cut];
            let html = to_html(piece);
            let openers = html.matches('<').count();
            let closers = html.matches("</").count() + html.matches("<a href").count();
            assert!(
                openers == 0 || closers > 0,
                "a tag was opened with nothing to close it, from {piece:?} -> {html:?}"
            );
        }
    }
}
