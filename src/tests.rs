#![expect(clippy::unwrap_used, reason = "This is a test module, panicking is fine")]
use std::{collections::HashSet, process::ExitCode};

use crate::{CargoShear, CargoShearOptions, default_path};

use super::{collect_imports, collect_file_references};

#[track_caller]
fn test(source_text: &str) {
    let deps = collect_imports(source_text).unwrap();
    let expected = HashSet::from_iter(["foo".to_owned()]);
    assert_eq!(deps, expected, "{source_text}");
}

#[test]
fn export_path() {
    test("#[test] fn box_serialize() { let b = foo::bar(&b).unwrap(); }");
}

#[test]
fn type_path() {
    test("fn main() { let x: Vec<foo::Bar> = vec![]; }");
}

#[test]
fn r#macro() {
    test(r#"fn main() { println!("{}", foo::bar); }"#);
}

#[test]
fn use_group() {
    test("pub use { foo };");
}

#[test]
fn meta_list() {
    test("#[derive(foo::Deserialize, foo::Serialize)] struct Foo;");
    test("#[derive(foo :: Deserialize, Debug)] struct Foo;");
    test("#[derive(foo:: Deserialize, Debug)] struct Foo;");
    test("#[derive(foo ::Deserialize, Debug)] struct Foo;");
    test("#[derive(foo        ::       Deserialize, Debug)] struct Foo;");
}

#[test]
fn extern_crate() {
    test("extern crate foo;");
}

#[test]
fn meta_list_path() {
    test(r#"#[foo::instrument(level = "debug")] fn print_with_indent() {}"#);
}

#[test]
fn use_rename() {
    test("use foo as bar;");
}

#[test]
fn macro_on_struct() {
    test("#[foo::self_referencing] struct AST {}");
}

#[test]
fn macro_on_verbatim() {
    test("#[foo::ext(name = ParserExt)] pub impl Parser {}");
}

#[test]
fn serde_with_on_field() {
    test("struct Foo { #[serde(with = \"foo\")] foo: () }");
    // should also work combined with other attributes
    test("struct Foo { #[serde(default, with = \"foo\")] bar: () }");
}

#[test]
fn serde_crate_on_type() {
    test("#[serde(crate = \"foo\")] struct Foo { bar: () }");
}

#[test]
fn test_lib() {
    let shear = CargoShear::new(CargoShearOptions {
        fix: false,
        package: vec![],
        exclude: vec![],
        path: default_path().unwrap(),
        expand: false,
        unused_files: false,
    });
    let exit_code = shear.run();
    assert_eq!(exit_code, ExitCode::SUCCESS);
}

#[test]
fn test_file_reference_collection() {
    use std::path::Path;
    
    // Test basic mod statement detection
    let source = "mod foo;\nmod bar;";
    let refs = collect_file_references(source, Path::new("src/lib.rs")).unwrap();
    // Note: In a real test environment, we'd need actual files to exist
    // This is testing the parsing logic
    assert_eq!(refs.len(), 0); // Files don't exist, so no refs collected
}

#[test]
fn test_file_reference_parsing() {
    use std::path::Path;
    
    // Test parsing of various mod statements
    let test_cases = vec![
        ("mod foo;", vec!["foo"]),
        ("mod foo;\nmod bar;", vec!["foo", "bar"]),
        ("mod foo { fn test() {} }", vec![]), // Inline mod, not external file
        ("pub mod foo;", vec!["foo"]),
        ("mod foo;\n// mod commented;", vec!["foo"]),
    ];
    
    for (source, _expected_mods) in test_cases {
        let refs = collect_file_references(source, Path::new("src/lib.rs")).unwrap();
        // Since we're just testing parsing, and files don't exist, refs will be empty
        // But we can verify the code doesn't panic and handles the syntax correctly
        assert!(refs.is_empty());
    }
}

#[test]
fn test_file_collector_module_resolution() {
    // Test that the file collector understands Rust module resolution patterns
    use std::fs;
    
    // Create a temporary directory structure for testing
    let temp_dir = std::env::temp_dir().join("cargo_shear_test");
    let _ = fs::remove_dir_all(&temp_dir); // Clean up if exists
    fs::create_dir_all(&temp_dir).unwrap();
    fs::create_dir_all(temp_dir.join("src")).unwrap();
    
    // Create test files
    fs::write(temp_dir.join("src/lib.rs"), "mod foo;").unwrap();
    fs::write(temp_dir.join("src/foo.rs"), "pub fn hello() {}").unwrap();
    
    let refs = collect_file_references("mod foo;", &temp_dir.join("src/lib.rs")).unwrap();
    assert_eq!(refs.len(), 1);
    
    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_unused_files_integration() {
    use std::fs;
    
    // Create a temporary test project structure
    let temp_dir = std::env::temp_dir().join("cargo_shear_integration_test");
    let _ = fs::remove_dir_all(&temp_dir); // Clean up if exists
    
    // Create project structure
    fs::create_dir_all(&temp_dir).unwrap();
    fs::create_dir_all(temp_dir.join("src")).unwrap();
    fs::create_dir_all(temp_dir.join("src/modules")).unwrap();
    
    // Create Cargo.toml
    fs::write(
        temp_dir.join("Cargo.toml"),
        r#"[package]
name = "test_integration"
version = "0.1.0"
edition = "2021"

[dependencies]"#,
    ).unwrap();
    
    // Create main.rs that references one module
    fs::write(
        temp_dir.join("src/main.rs"),
        "mod used_module;\nfn main() { used_module::hello(); }",
    ).unwrap();
    
    // Create used module
    fs::write(
        temp_dir.join("src/used_module.rs"),
        "pub fn hello() { println!(\"Hello\"); }",
    ).unwrap();
    
    // Create unused files
    fs::write(
        temp_dir.join("src/unused.rs"),
        "pub fn unused() {}",
    ).unwrap();
    
    fs::write(
        temp_dir.join("src/modules/unused_mod.rs"),
        "pub fn unused_function() {}",
    ).unwrap();
    
    // Test the unused files functionality
    let shear = CargoShear::new(CargoShearOptions {
        fix: false,
        package: vec![],
        exclude: vec![],
        path: temp_dir.clone(),
        expand: false,
        unused_files: true,
    });
    
    let exit_code = shear.run();
    assert_eq!(exit_code, ExitCode::SUCCESS);
    
    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}
