//! The knowledge-base id rules (Knowledge call C, Ryan's call 7 of Move 1 step 4), checked
//! against synthetic atoms, a `.kb` tree on disk and this repository.

use std::path::Path;

use xtask::ids::{AtomFile, Violation, check, read_kb};

use crate::support::{scratch, tree};

fn atom(path: &str, text: &str) -> AtomFile {
    AtomFile {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}

fn decision(path: &str, id: &str, item: &str, question: &str) -> AtomFile {
    atom(
        path,
        &format!(
            "---\nid: \"{id}\"\ntitle: \"Chosen\"\nkind: decision\nweighin_item: \"{item}\"\n\
             question: \"{question}\"\n---\n\n# Chosen\n"
        ),
    )
}

fn open_question(path: &str, id: &str, item: &str, question: &str) -> AtomFile {
    atom(
        path,
        &format!(
            "---\nid: \"{id}\"\ntitle: \"{question}\"\nkind: open-question\ntracks: \"{item}\"\n\
             ---\n\n# {question}\n"
        ),
    )
}

#[test]
fn distinct_atoms_in_their_folders_pass() {
    let atoms = [
        decision(
            "decision/wi-1a2b3c-chosen.md",
            "kb-decision-wi-1a2b3c",
            "wi-1a2b3c",
            "Where?",
        ),
        decision(
            "decision/0123456789-chosen.md",
            "kb-decision-0123456789",
            "wi-0a1b2c3d4e",
            "When?",
        ),
        open_question(
            "open-question/wi-1a2b3c-where.md",
            "kb-open-question-wi-1a2b3c",
            "wi-1a2b3c",
            "Where?",
        ),
    ];
    assert_eq!(check(&atoms), Vec::new());
}

#[test]
fn two_files_with_one_id_fail_naming_both() {
    let atoms = [
        decision("decision/a-x.md", "kb-decision-a", "wi-1", "Q?"),
        decision("decision/a-y.md", "kb-decision-a", "wi-1", "Q?"),
    ];
    assert_eq!(
        check(&atoms),
        vec![Violation::DuplicateId {
            id: "kb-decision-a".to_owned(),
            paths: vec!["decision/a-x.md".to_owned(), "decision/a-y.md".to_owned()],
        }]
    );
}

#[test]
fn one_wi_id_for_two_questions_fails_as_a_collision() {
    let atoms = [
        decision("decision/a-x.md", "kb-decision-a", "wi-1a2b3c", "Where?"),
        decision("decision/b-x.md", "kb-decision-b", "wi-1a2b3c", "When?"),
    ];
    assert_eq!(
        check(&atoms),
        vec![Violation::ItemCollision {
            item: "wi-1a2b3c".to_owned(),
            first: "decision/a-x.md".to_owned(),
            second: "decision/b-x.md".to_owned(),
        }]
    );
}

#[test]
fn an_open_question_tracking_a_decided_item_with_its_question_passes() {
    let atoms = [
        decision("decision/a-x.md", "kb-decision-a", "wi-1", "Where?"),
        open_question(
            "open-question/b-x.md",
            "kb-open-question-b",
            "wi-1",
            "Where?",
        ),
    ];
    assert_eq!(check(&atoms), Vec::new());
}

#[test]
fn an_open_question_tracking_an_item_under_another_question_collides() {
    let atoms = [
        decision("decision/a-x.md", "kb-decision-a", "wi-1", "Where?"),
        open_question(
            "open-question/b-x.md",
            "kb-open-question-b",
            "wi-1",
            "When?",
        ),
    ];
    assert_eq!(
        check(&atoms),
        vec![Violation::ItemCollision {
            item: "wi-1".to_owned(),
            first: "decision/a-x.md".to_owned(),
            second: "open-question/b-x.md".to_owned(),
        }]
    );
}

#[test]
fn an_atom_outside_its_kinds_folder_fails() {
    let atoms = [
        decision("open-question/a-x.md", "kb-decision-a", "wi-1", "Q?"),
        decision("_intake/decisions/b-x.md", "kb-decision-b", "wi-2", "Q?"),
        decision("decision/nested/c-x.md", "kb-decision-c", "wi-3", "Q?"),
    ];
    let misplaced = |path: &str, id: &str| Violation::Misplaced {
        path: path.to_owned(),
        id: id.to_owned(),
        folder: ".kb/decision".to_owned(),
    };
    assert_eq!(
        check(&atoms),
        vec![
            misplaced("_intake/decisions/b-x.md", "kb-decision-b"),
            misplaced("decision/nested/c-x.md", "kb-decision-c"),
            misplaced("open-question/a-x.md", "kb-decision-a"),
        ]
    );
}

#[test]
fn an_id_of_no_known_kind_fails() {
    let atoms = [
        atom("decision/a.md", "---\nid: \"kb-ruling\"\n---\n"),
        atom("decision/b.md", "---\nid: \"kb-open_question-b\"\n---\n"),
        atom("decision/c.md", "---\nid: \"wi-1a2b3c\"\n---\n"),
    ];
    let unknown = |path: &str, id: &str| Violation::UnknownId {
        path: path.to_owned(),
        id: id.to_owned(),
    };
    assert_eq!(
        check(&atoms),
        vec![
            unknown("decision/a.md", "kb-ruling"),
            unknown("decision/b.md", "kb-open_question-b"),
            unknown("decision/c.md", "wi-1a2b3c"),
        ]
    );
}

#[test]
fn a_file_in_a_kinds_folder_without_an_id_fails_and_others_are_not_atoms() {
    let atoms = [
        atom("decision/a.md", "# No frontmatter\n"),
        atom("decision/b.md", "---\ntitle: \"No id\"\n---\n"),
        atom("README.md", "# The knowledge base\n"),
        atom("notes/c.md", "---\ntitle: \"Not an atom\"\n---\n"),
    ];
    let no_id = |path: &str| Violation::NoId {
        path: path.to_owned(),
    };
    assert_eq!(
        check(&atoms),
        vec![no_id("decision/a.md"), no_id("decision/b.md")]
    );
}

#[test]
fn the_reader_takes_markdown_and_passes_over_ledger_folders() -> std::io::Result<()> {
    let root = scratch("ids-reader")?;
    tree(
        &root,
        &[
            ("decision/a-x.md", "---\nid: \"kb-decision-a\"\n---\n"),
            ("decision/a-x.json", "{}"),
            ("_ledger/journal/s.md", "---\nid: \"kb-decision-a\"\n---\n"),
            (
                "open-question/b-y.md",
                "---\nid: \"kb-open-question-b\"\n---\n",
            ),
        ],
    )?;
    let atoms = read_kb(&root).map_err(std::io::Error::other)?;
    let paths: Vec<&str> = atoms.iter().map(|a| a.path.as_str()).collect();
    assert_eq!(paths, ["decision/a-x.md", "open-question/b-y.md"]);
    Ok(())
}

#[test]
fn an_absent_knowledge_base_holds_no_atoms() -> std::io::Result<()> {
    let root = scratch("ids-absent")?;
    let atoms = read_kb(&root.join(".kb")).map_err(std::io::Error::other)?;
    assert_eq!(atoms, Vec::new());
    Ok(())
}

#[test]
fn this_repository_follows_the_id_rules() -> std::io::Result<()> {
    let kb = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.kb");
    let atoms = read_kb(&kb).map_err(std::io::Error::other)?;
    assert!(!atoms.is_empty(), "the moved Weigh-In atoms are read");
    assert_eq!(check(&atoms), Vec::new());
    Ok(())
}
