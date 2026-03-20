/// Tests for symbol disambiguation logic (SYMB-01, SYMB-02, SYMB-03).
///
/// These tests exercise `resolve_symbol_selector` (src/live_index/query.rs)
/// through the public `capture_context_bundle_view` API which returns
/// `ContextBundleView` variants that directly reflect the resolution outcome:
/// - `Found` -> single symbol selected (container-vs-member heuristic succeeded)
/// - `AmbiguousSymbol` -> genuine ambiguity with candidate line numbers
///
/// The container-vs-member heuristic selects the single container kind
/// (Class, Struct, Enum, Trait, Interface, Module) when exactly one
/// exists among same-named candidates.
use std::fs;
use std::path::Path;
use symforge::live_index::{ContextBundleView, LiveIndex};
use tempfile::tempdir;

// --------------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------------

fn write_file(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

// --------------------------------------------------------------------------
// SYMB-01: Container-vs-member auto-disambiguation
//
// When two symbols share the same name but one is a container (Class) and
// the other is a non-container (Function), resolving by name alone (no
// kind filter) should auto-select the container.
// --------------------------------------------------------------------------

#[test]
fn test_symb01_container_vs_member_auto_disambiguation() {
    let dir = tempdir().unwrap();

    // JavaScript: class Foo and function Foo in the same file.
    // The parser produces Foo(Class) and Foo(Function).
    write_file(
        dir.path(),
        "ambig.js",
        "class Foo {\n    bar() { return 1; }\n}\n\nfunction Foo() {\n    return new Foo();\n}\n",
    );

    let shared = LiveIndex::load(dir.path()).unwrap();
    let index = shared.read();

    // Resolve "Foo" without specifying kind or line -- the heuristic should
    // pick the Class over the Function.
    let result = index.capture_context_bundle_view("ambig.js", "Foo", None, None);

    match &result {
        ContextBundleView::Found(found) => {
            assert_eq!(
                found.kind_label.to_lowercase(),
                "class",
                "SYMB-01: expected container (class) to be auto-selected, got kind={}",
                found.kind_label
            );
            assert!(
                found.body.contains("class Foo"),
                "SYMB-01: expected class body, got: {}",
                found.body
            );
        }
        ContextBundleView::AmbiguousSymbol {
            candidate_lines, ..
        } => {
            panic!(
                "SYMB-01: expected auto-disambiguation to select the class, \
                 but got Ambiguous with candidate lines: {:?}",
                candidate_lines
            );
        }
        other => {
            panic!("SYMB-01: unexpected result variant: {:?}", other);
        }
    }
}

// --------------------------------------------------------------------------
// SYMB-02: C# class/constructor disambiguation
//
// The C# parser maps constructor_declaration to SymbolKind::Function.
// A class Foo with a constructor Foo(int x) produces two symbols:
//   - Foo (Class)
//   - Foo (Function)
// Resolving "Foo" without kind should return the class, not ambiguity.
// --------------------------------------------------------------------------

#[test]
fn test_symb02_csharp_class_constructor_disambiguation() {
    let dir = tempdir().unwrap();

    write_file(
        dir.path(),
        "Foo.cs",
        "public class Foo {\n    public Foo(int x) { }\n    public void Bar() { }\n}\n",
    );

    let shared = LiveIndex::load(dir.path()).unwrap();
    let index = shared.read();

    let result = index.capture_context_bundle_view("Foo.cs", "Foo", None, None);

    match &result {
        ContextBundleView::Found(found) => {
            assert_eq!(
                found.kind_label.to_lowercase(),
                "class",
                "SYMB-02: expected Class to be selected for C# class/constructor pair, got kind={}",
                found.kind_label
            );
            assert!(
                found.body.contains("public class Foo"),
                "SYMB-02: expected class body in result, got: {}",
                found.body
            );
        }
        ContextBundleView::AmbiguousSymbol {
            candidate_lines, ..
        } => {
            panic!(
                "SYMB-02: expected auto-disambiguation to select the class, \
                 but got Ambiguous with candidate lines: {:?}",
                candidate_lines
            );
        }
        other => {
            panic!("SYMB-02: unexpected result variant: {:?}", other);
        }
    }
}

// --------------------------------------------------------------------------
// SYMB-03: Genuine ambiguity preserved
//
// When two symbols share the same name AND the same tier (both functions),
// resolving by name alone must return Ambiguous with candidate line numbers
// -- NOT silently pick one.
// --------------------------------------------------------------------------

#[test]
fn test_symb03_genuine_ambiguity_preserved() {
    let dir = tempdir().unwrap();

    // Python: two top-level functions with the same name.
    // The parser produces two Function symbols both named "helper".
    write_file(
        dir.path(),
        "dupes.py",
        "def helper():\n    return 1\n\ndef helper():\n    return 2\n",
    );

    let shared = LiveIndex::load(dir.path()).unwrap();
    let index = shared.read();

    let result = index.capture_context_bundle_view("dupes.py", "helper", None, None);

    match &result {
        ContextBundleView::AmbiguousSymbol {
            name,
            candidate_lines,
            ..
        } => {
            assert_eq!(
                name, "helper",
                "SYMB-03: ambiguous result should report the queried name"
            );
            assert_eq!(
                candidate_lines.len(),
                2,
                "SYMB-03: expected exactly 2 candidate lines, got {:?}",
                candidate_lines
            );
            // Verify both line numbers are present and different
            assert_ne!(
                candidate_lines[0], candidate_lines[1],
                "SYMB-03: candidate lines should be distinct"
            );
        }
        ContextBundleView::Found(found) => {
            panic!(
                "SYMB-03: expected Ambiguous for two same-tier symbols, \
                 but got Found with kind={}, lines {:?}",
                found.kind_label, found.line_range
            );
        }
        other => {
            panic!("SYMB-03: unexpected result variant: {:?}", other);
        }
    }
}
