//! The system prompt, loaded from the prompt directory.
//!
//! The prompt is prose in files, not a constant in code, so editing the
//! assistant's voice is not a code change. Every regular file in the
//! directory contributes, joined in file-name order with a blank line — one
//! file is the common case, and a split into several stays possible without
//! touching this loader.
//!
//! What this repository ships is the prose every deployment shares: the
//! project context and the conduct rules. **The persona — who the assistant
//! is and how it speaks — is deliberately not here** (decided 2026-08-24).
//! A character belongs to the deployment that wears it, not to the shared
//! source, and keeping it out means changing a voice is a deployment edit
//! rather than a new revision of the bot. A deployment supplies its own file
//! in this directory; the numbering leaves room between the context and the
//! conduct for exactly that. A deployment that supplies none simply has no
//! persona, which is a coherent thing to be rather than an error.

use std::path::Path;

use crate::StartError;

/// Read the prompt directory into the one system prompt string.
///
/// # Errors
///
/// [`StartError::PromptUnread`] when the directory or a file in it cannot be
/// read; [`StartError::PromptEmpty`] when no file contributes any text — a
/// silently empty prompt would strip the assistant of its role without a
/// trace, so it refuses instead.
pub fn load(dir: &Path) -> Result<String, StartError> {
    let unread = |error: std::io::Error| StartError::PromptUnread {
        dir: dir.to_path_buf(),
        error,
    };
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(unread)?
        .collect::<Result<_, _>>()
        .map_err(unread)?;
    files.sort_by_key(std::fs::DirEntry::file_name);
    let mut parts = Vec::new();
    for entry in files {
        if entry.file_type().map_err(unread)?.is_file() {
            let text = std::fs::read_to_string(entry.path()).map_err(unread)?;
            let text = text.trim();
            if !text.is_empty() {
                parts.push(text.to_owned());
            }
        }
    }
    if parts.is_empty() {
        return Err(StartError::PromptEmpty {
            dir: dir.to_path_buf(),
        });
    }
    Ok(parts.join("\n\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique directory in the temp location, removed with its content on
    /// drop, so parallel tests never share files and no run leaves litter.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let unique = format!(
                "assistant-prompt-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("the clock is past the epoch")
                    .as_nanos()
            );
            let dir = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(&dir).expect("the temp directory creates");
            Self(dir)
        }

        fn write(&self, name: &str, content: &str) {
            std::fs::write(self.0.join(name), content).expect("the prompt file writes");
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn files_join_in_name_order_trimmed_with_a_blank_line_between() {
        let dir = TempDir::new("join");
        dir.write("b-closing.md", "The closing part.\n");
        dir.write("a-opening.md", "\nThe opening part.\n");
        assert_eq!(
            load(&dir.0).expect("the prompt loads"),
            "The opening part.\n\nThe closing part."
        );
    }

    #[test]
    fn a_directory_without_prompt_text_refuses() {
        let dir = TempDir::new("empty");
        dir.write("only-whitespace.md", " \n\t\n");
        assert!(
            matches!(load(&dir.0), Err(StartError::PromptEmpty { .. })),
            "a silently empty prompt must refuse the start"
        );
    }

    #[test]
    fn an_unreadable_directory_refuses() {
        let dir = TempDir::new("missing");
        assert!(
            matches!(
                load(&dir.0.join("absent")),
                Err(StartError::PromptUnread { .. })
            ),
            "an unreadable prompt directory must refuse the start"
        );
    }
}
