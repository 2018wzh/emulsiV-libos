//! Structural documentation checks. These are not an ASD-STE100 certification.
use crate::project::Project;
use anyhow::{ensure, Context, Result};
use serde_json::json;
use std::{fs, path::Path};
fn blocks(text: &str) -> Result<Vec<String>> {
    let mut code = Vec::new();
    let mut block = None;
    for line in text.lines() {
        if line.starts_with("```") {
            if let Some(s) = block.take() {
                code.push(s);
            } else {
                block = Some(String::new());
            }
        } else if let Some(s) = &mut block {
            s.push_str(line);
            s.push('\n');
        }
    }
    ensure!(block.is_none(), "unclosed code fence");
    Ok(code)
}
fn links(text: &str) -> Vec<&str> {
    text.split("](")
        .skip(1)
        .filter_map(|s| s.split_once(')').map(|(v, _)| v))
        .collect()
}
fn local_links(root: &Path, name: &str, text: &str) -> Result<()> {
    for link in links(text) {
        if link.contains("://") || link.starts_with("mailto:") || link.starts_with('#') {
            continue;
        }
        let path = link.split('#').next().unwrap();
        ensure!(
            root.join(path).is_file() || root.join(path).is_dir(),
            "{name}: missing link target {path}"
        );
    }
    Ok(())
}
pub fn check(p: &Project) -> Result<()> {
    let en = fs::read_to_string(p.root.join("README.md"))?;
    let zh = fs::read_to_string(p.root.join("README.zh-CN.md"))?;
    ensure!(
        blocks(&en)? == blocks(&zh)?,
        "English and Chinese README code blocks differ"
    );
    for (name, text) in [("README.md", &en), ("README.zh-CN.md", &zh)] {
        for anchor in [
            "scope",
            "requirements",
            "quick-start",
            "features",
            "heap",
            "examples",
            "xtask",
            "integration",
            "verification",
            "troubleshooting",
            "release",
            "contributing",
            "license",
        ] {
            ensure!(
                text.contains(&format!("id=\"{anchor}\"")),
                "{name}: missing {anchor} section"
            );
        }
        ensure!(text.contains(&p.version()?), "{name}: version mismatch");
        for e in p.examples()? {
            ensure!(
                text.contains(&format!("`{}`", e.name)),
                "{name}: undocumented example {}",
                e.name
            );
        }
        local_links(&p.root, name, text)?;
        ensure!(
            !text.contains("tools/verify.sh") && !text.contains("python3 tools/"),
            "{name}: obsolete active tool command"
        );
    }
    for name in [
        "docs/HEAP.md",
        "docs/PORTING.md",
        "docs/VERIFICATION.md",
        "docs/WRITING.md",
        "docs/RELEASE.md",
        "LICENSE",
        "CHANGELOG.md",
        "CONTRIBUTING.md",
    ] {
        ensure!(
            fs::metadata(p.root.join(name))
                .with_context(|| format!("required document {name}"))?
                .len()
                > 0,
            "empty document"
        );
    }
    p.write_json("docs-verification.json",&json!({"english_readme":true,"chinese_readme":true,"matching_code_blocks":true,"local_links":true,"all_examples_documented":true,"style":"STE-inspired technical writing; not formal ASD-STE100 certification"}))?;
    println!(
        "PASS bilingual README structure, matching commands, local links and example coverage"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_code() {
        assert_eq!(blocks("text\n```rust\nx\n```\n").unwrap(), vec!["x\n"]);
    }
    #[test]
    fn rejects_unclosed_fence() {
        assert!(blocks("```rust\nx").is_err());
    }
    #[test]
    fn empty_document_has_no_code() {
        assert!(blocks("").unwrap().is_empty());
    }
    #[test]
    fn extracts_markdown_links() {
        assert_eq!(
            links("[a](README.md) and [b](https://example.org)"),
            vec!["README.md", "https://example.org"]
        );
    }
}
