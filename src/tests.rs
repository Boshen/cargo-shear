#![expect(clippy::unwrap_used, reason = "This is a test module, panicking is fine")]
use std::{collections::HashSet, process::ExitCode};

use crate::{CargoShear, CargoShearOptions, default_path};

use super::collect_imports;

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
fn test_lib() {
    let shear = CargoShear::new(CargoShearOptions {
        fix: false,
        package: vec![],
        exclude: vec![],
        path: default_path().unwrap(),
    });
    let exit_code = shear.run();
    assert_eq!(exit_code, ExitCode::SUCCESS);
}
