//! `weave explain <file>` — the pull side of the findings channel.
//!
//! One file, on demand, in the words a reader needs and no others. The
//! alternative — a findings document materialized beside every conflicted
//! file — cost bytes on every merge without moving the outcome. The document
//! was not wrong; it was unasked.
//!
//! Two things this does that the document did not:
//!
//! * it counts **contested** hunks — hunks BOTH sides wrote in — instead of
//!   diff hunks. "158 hunks, whole-file conflict" for three real
//!   disagreements is disorienting enough that a reader will reasonably
//!   re-derive the truth by hand with `git merge-file` instead of trusting
//!   it, and
//! * it reads the three stages out of the INDEX (`:1:`/`:2:`/`:3:`), so it
//!   explains the conflict git actually produced, not a re-merge of two
//!   branch tips.

use std::path::Path;

use weave_cli::gitscan;

type R<T> = Result<T, Box<dyn std::error::Error>>;

pub(crate) fn run(file: &str, json: bool, host: &weave_core::host::Host) -> R<()> {
    let dir = Path::new(".");
    let (base, ours, theirs) = gitscan::file_stages(dir, file)?;
    let doc = weave_core::explain::explain(&base, &ours, &theirs, file, host);
    if json {
        println!("{}", serde_json::to_string_pretty(&doc)?);
    } else {
        print!("{}", doc.render());
        if !doc.conflicts.is_empty() {
            println!(
                "\nrun `weave check` after you edit to verify the resolution against these \
                 three stages."
            );
        }
    }
    Ok(())
}
