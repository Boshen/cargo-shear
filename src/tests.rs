#![expect(clippy::unwrap_used, reason = "This is a test module, panicking is fine")]
use std::{collections::HashSet, process::ExitCode};

use crate::{CargoShear, CargoShearOptions, default_path};

use super::collect_imports;

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
    });
    let exit_code = shear.run();
    assert_eq!(exit_code, ExitCode::SUCCESS);
}

// Additional import collection tests for edge cases

#[test]
fn empty_source() {
    let deps = collect_imports("").unwrap();
    assert!(deps.is_empty());
}

#[test]
fn comment_only() {
    let deps = collect_imports("// this is a comment").unwrap();
    assert!(deps.is_empty());
}

#[test]
fn std_imports_not_collected() {
    let deps = collect_imports("use std::collections::HashMap;").unwrap();
    assert!(deps.is_empty());
}

#[test]
fn self_super_crate_not_collected() {
    let deps = collect_imports("use self::module;").unwrap();
    assert!(deps.is_empty());

    let deps = collect_imports("use super::module;").unwrap();
    assert!(deps.is_empty());

    let deps = collect_imports("use crate::module;").unwrap();
    assert!(deps.is_empty());
}

#[test]
fn multiple_imports_same_crate() {
    let source = r#"
        use foo::bar;
        use foo::baz;
        fn main() {
            foo::qux();
        }
    "#;
    let deps = collect_imports(source).unwrap();
    let expected = HashSet::from_iter(["foo".to_owned()]);
    assert_eq!(deps, expected);
}

#[test]
fn complex_path_expressions() {
    test("fn main() { let x = foo::bar::baz(); }");
    test("fn main() { foo::BAR::baz(); }"); // Should not collect uppercase paths
}

#[test]
fn generic_types_with_paths() {
    test("fn main() { let x: Vec<foo::Bar> = Vec::new(); }");
    test("fn main() { let x: HashMap<String, foo::Value> = HashMap::new(); }");
}

#[test]
fn nested_module_usage() {
    test("mod inner { use foo::bar; }");
    test("mod inner { pub use foo::bar; }");
}

#[test]
fn conditional_compilation() {
    test("#[cfg(feature = \"test\")] use foo::bar;");
    test("#[cfg(target_os = \"linux\")] use foo::bar;");
}

#[test]
fn function_calls_with_paths() {
    test("fn main() { foo::bar::create_something(); }");
    test("fn main() { let result = foo::utils::helper(); }");
}

#[test]
fn struct_initialization() {
    test("fn main() { let s = foo::Struct { field: 1 }; }");
    test("fn main() { let s = foo::Struct::new(); }");
}

#[test]
fn trait_implementations() {
    test("impl foo::Trait for MyStruct {}");
    test("impl<T> foo::Trait<T> for MyStruct<T> {}");
}

#[test]
fn constant_references() {
    test("fn main() { println!(\"{}\", foo::CONSTANT); }");
    test("static MY_CONST: i32 = foo::VALUE;");
}

#[test]
fn closure_with_paths() {
    test("fn main() { let f = || foo::bar(); }");
    test("fn main() { let f = |x| foo::process(x); }");
}

#[test]
fn match_patterns() {
    test("fn main() { match x { foo::Variant => {}, _ => {} } }");
    test("fn main() { if let foo::Some(val) = x { } }");
}

#[test]
fn complex_serde_attributes() {
    test("struct Foo { #[serde(with = \"foo::serializer\")] field: String }");
    test("struct Foo { #[serde(deserialize_with = \"foo::deserialize\")] field: String }");
    // Test serde crate attribute - this should work based on the serde_crate_on_type test
    test("#[serde(crate = \"foo\")] struct Foo {}");
}

#[test]
fn proc_macro_attributes() {
    test("#[foo::derive(Debug)] struct MyStruct {}");
    test("#[foo::custom_attr] fn my_function() {}");
}

#[test]
fn async_syntax() {
    test("async fn test() { foo::async_function().await; }");
    test("fn main() { let future = foo::create_future(); }");
}

#[test]
fn error_handling() {
    test("fn main() -> Result<(), foo::Error> { Ok(()) }");
    test("fn main() { foo::Result::from(value)?; }");
}

#[test]
fn lifetimes_and_generics() {
    test("fn process<'a>(data: &'a foo::Data<'a>) {}");
    test("struct Container<T: foo::Trait> { inner: T }");
}

// Tests for edge cases that might cause parsing errors

#[test]
fn malformed_syntax_recovery() {
    // Test that we can handle some malformed syntax gracefully
    let result = collect_imports("use foo::;"); // Incomplete use statement
    // Should either parse successfully or return an error, but not panic
    match result {
        Ok(deps) => {
            // If it parses, foo should be collected
            assert!(deps.contains("foo") || deps.is_empty());
        }
        Err(_) => {
            // Parsing error is acceptable for malformed syntax
        }
    }
}

#[test]
fn very_long_path() {
    let long_path = "foo::".repeat(100) + "bar";
    let source = format!("use {};", long_path);
    let deps = collect_imports(&source).unwrap();
    let expected = HashSet::from_iter(["foo".to_owned()]);
    assert_eq!(deps, expected);
}

#[test]
fn unicode_identifiers() {
    let deps = collect_imports("use foo::数据;").unwrap();
    let expected = HashSet::from_iter(["foo".to_owned()]);
    assert_eq!(deps, expected);
}

#[test]
fn macro_rules() {
    test("macro_rules! my_macro { () => { foo::bar() }; }");
    test("foo::my_macro!();");
}

#[test]
fn raw_string_inside_macro() {
    test(r##"fn main() { foo::my_macro!(r#"this mentions foo::bar inside a raw string"#); }"##);
    test(
        r###"fn main() { foo::my_macro!(r##"this mentions foo::baz inside a double-hash raw string"##); }"###,
    );
}

// Tests for different import patterns

#[test]
fn glob_imports() {
    let deps = collect_imports("use foo::*;").unwrap();
    let expected = HashSet::from_iter(["foo".to_owned()]);
    assert_eq!(deps, expected);
}

#[test]
fn nested_use_groups() {
    test("use foo::{bar, baz::{qux, quux}};");
    test("use foo::{self, bar::*};");
}

#[test]
fn pub_use_reexports() {
    test("pub use foo::bar;");
    test("pub(crate) use foo::internal;");
    test("pub(super) use foo::parent;");
}

// Integration tests for the full cargo-shear workflow

#[test]
fn cargo_shear_with_different_options() {
    // Test with fix=true but no unused dependencies (should not change anything)
    let shear = CargoShear::new(CargoShearOptions {
        fix: true,
        package: vec![],
        exclude: vec![],
        path: default_path().unwrap(),
        expand: false,
    });
    let exit_code = shear.run();
    assert_eq!(exit_code, ExitCode::SUCCESS);
}

#[test]
fn cargo_shear_with_package_filter() {
    // Test with specific package filtering
    let shear = CargoShear::new(CargoShearOptions {
        fix: false,
        package: vec!["cargo-shear".to_string()],
        exclude: vec![],
        path: default_path().unwrap(),
        expand: false,
    });
    let exit_code = shear.run();
    assert_eq!(exit_code, ExitCode::SUCCESS);
}

#[test]
fn cargo_shear_with_exclude_filter() {
    // Test with package exclusion
    let shear = CargoShear::new(CargoShearOptions {
        fix: false,
        package: vec![],
        exclude: vec!["some-package".to_string()],
        path: default_path().unwrap(),
        expand: false,
    });
    let exit_code = shear.run();
    assert_eq!(exit_code, ExitCode::SUCCESS);
}

#[test]
fn cargo_shear_options_creation() {
    // Test that we can create options with all combinations
    let options = CargoShearOptions {
        fix: true,
        package: vec!["test1".to_string(), "test2".to_string()],
        exclude: vec!["exclude1".to_string()],
        path: std::path::PathBuf::from("/tmp"),
        expand: true,
    };

    let shear = CargoShear::new(options.clone());
    // Verify the shear instance was created successfully
    assert_eq!(format!("{:?}", shear.options.fix), format!("{:?}", options.fix));
}

// Error handling and edge case tests

#[test]
fn invalid_rust_syntax() {
    let result = collect_imports("this is not rust code ^^^");
    assert!(result.is_err(), "Should fail to parse invalid Rust syntax");
}

#[test]
fn empty_file_handling() {
    let deps = collect_imports("").unwrap();
    assert!(deps.is_empty(), "Empty file should result in no dependencies");
}

#[test]
fn whitespace_only() {
    let deps = collect_imports("   \n  \t  \n  ").unwrap();
    assert!(deps.is_empty(), "Whitespace-only file should result in no dependencies");
}

#[test]
fn mixed_valid_invalid_imports() {
    // Test a file with some valid and some edge-case imports
    let source = r#"
        use foo::bar;  // valid
        use std::collections::HashMap;  // should be ignored (std)
        use self::local;  // should be ignored (self)
        use foo::baz::qux;  // valid, same crate as first
    "#;
    let deps = collect_imports(source).unwrap();
    let expected = HashSet::from_iter(["foo".to_owned()]);
    assert_eq!(deps, expected);
}

// Test helper function variations

#[track_caller]
fn test_no_deps(source_text: &str) {
    let deps = collect_imports(source_text).unwrap();
    assert!(deps.is_empty(), "Expected no dependencies for: {source_text}");
}

#[track_caller]
fn test_multiple_deps(source_text: &str, expected_deps: &[&str]) {
    let deps = collect_imports(source_text).unwrap();
    let expected = HashSet::from_iter(expected_deps.iter().map(|s| s.to_string()));
    assert_eq!(deps, expected, "Dependencies mismatch for: {source_text}");
}

#[test]
fn test_multiple_crates() {
    test_multiple_deps("use foo::bar; use baz::qux;", &["foo", "baz"]);

    test_multiple_deps("fn main() { foo::func(); bar::other(); }", &["foo", "bar"]);
}

#[test]
fn test_no_false_positives() {
    test_no_deps("use std::vec::Vec;");
    test_no_deps("use self::module;");
    test_no_deps("use super::parent;");
    test_no_deps("use crate::internal;");
    test_no_deps("// just a comment");
    test_no_deps("fn main() {} // empty function");
}

// Performance and stress tests

#[test]
fn large_file_simulation() {
    // Create a moderately large source file to test performance
    let mut source = String::with_capacity(10000);
    for i in 0..100 {
        source.push_str(&format!("use foo::module{};\n", i));
        source.push_str(&format!("fn func{}() {{ foo::call{}(); }}\n", i, i));
    }

    let deps = collect_imports(&source).unwrap();
    let expected = HashSet::from_iter(["foo".to_owned()]);
    assert_eq!(deps, expected);
}

#[test]
fn deeply_nested_paths() {
    let nested_path = (0..20).map(|i| format!("level{}", i)).collect::<Vec<_>>().join("::");
    let source = format!("use foo::{};", nested_path);
    let deps = collect_imports(&source).unwrap();
    let expected = HashSet::from_iter(["foo".to_owned()]);
    assert_eq!(deps, expected);
}

// Tests for specific Rust language features

#[test]
fn const_generics() {
    test("fn process<const N: usize>() { foo::array::<N>(); }");
    test("struct Array<T, const N: usize> { data: foo::Data<T, N> }");
}

#[test]
fn associated_types() {
    test("fn process<T: foo::Iterator>() where T::Item: Clone {}");
    test("type Output = foo::Result<Self::Item>;");
}

#[test]
fn higher_ranked_trait_bounds() {
    test("fn process<F>() where F: for<'a> foo::Fn(&'a str) {}");
}

#[test]
fn raw_identifiers() {
    test("use foo::r#type;");
    test("fn main() { foo::r#match(); }");
}

#[test]
fn raw_identifier_crate_name() {
    let deps = collect_imports("use r#continue::thing;").unwrap();
    assert!(deps.contains("continue"));
}

#[test]
fn visibility_modifiers() {
    test("pub use foo::public;");
    test("pub(crate) use foo::crate_public;");
    test("pub(in crate::module) use foo::module_public;");
}

#[test]
fn extern_blocks() {
    test("extern \"C\" { fn foo_function() -> foo::Type; }");
    test("extern { static FOO_STATIC: foo::Type; }");
}

// File system and temporary project tests

#[test]
fn test_default_path() {
    let path = default_path().unwrap();
    assert!(path.exists(), "Default path should exist");
}

#[test]
fn test_cargo_shear_new() {
    let options = CargoShearOptions {
        fix: false,
        package: vec![],
        exclude: vec![],
        path: default_path().unwrap(),
        expand: false,
    };
    let _shear = CargoShear::new(options);
    // Just verify it can be created without panicking
}

// Comprehensive syntax coverage tests

#[test]
fn advanced_trait_bounds() {
    test("fn complex<T>() where T: foo::Clone + foo::Debug + Send + Sync {}");
    test("impl<T: foo::Display> foo::fmt::Display for Wrapper<T> {}");
}

#[test]
fn phantom_data_usage() {
    test("struct Marker<T> { _phantom: foo::PhantomData<T> }");
}

#[test]
fn dyn_trait_objects() {
    test("fn process(obj: &dyn foo::Trait) {}");
    test("fn factory() -> Box<dyn foo::Factory> { todo!() }");
}

#[test]
fn impl_trait_syntax() {
    test("fn create() -> impl foo::Iterator<Item = String> { todo!() }");
    test("fn process(iter: impl foo::IntoIterator) {}");
}

#[test]
fn never_type() {
    test("fn panic_handler() -> foo::Never { panic!() }");
}

#[test]
fn question_mark_operator() {
    test("fn fallible() -> foo::Result<()> { foo::might_fail()?; Ok(()) }");
}

#[test]
fn range_syntax() {
    test("fn main() { for i in foo::range(0..10) {} }");
    test("fn slice_range(data: &[i32]) -> &[i32] { &data[foo::start()..foo::end()] }");
}

#[test]
fn try_blocks() {
    test("fn main() { let result: foo::Result<()> = try { foo::operation()?; }; }");
}

#[test]
fn destructuring_patterns() {
    test("fn main() { let foo::Struct { field1, field2 } = data; }");
    test("fn main() { if let foo::Enum::Variant(x, y) = value {} }");
}

#[test]
fn tuple_struct_patterns() {
    test("fn main() { let foo::Point(x, y) = point; }");
    test("fn process(foo::Wrapper(inner): foo::Wrapper<T>) {}");
}

#[test]
fn reference_patterns() {
    test("fn main() { match &value { foo::Ref(x) => {} } }");
}

#[test]
fn slice_patterns() {
    test("fn main() { match slice { [foo::first, rest @ ..] => {} } }");
}

// Complex real-world scenarios

#[test]
fn web_framework_style() {
    test("#[foo::get(\"/api/users\")] async fn get_users() -> foo::Response {}");
    test("#[foo::derive(Serialize, Deserialize)] struct User { name: String }");
}

#[test]
fn database_integration() {
    test(
        "async fn query() -> foo::Result<Vec<User>> { foo::query(\"SELECT * FROM users\").await }",
    );
    test("#[foo::table_name = \"users\"] struct User { id: i32 }");
}

#[test]
fn logging_and_instrumentation() {
    test("#[foo::instrument] async fn process_data() {}");
    test("fn main() { foo::info!(\"Processing {}\", count); }");
}

#[test]
fn configuration_management() {
    test("#[foo::derive(Config)] struct AppConfig { database_url: String }");
    test("fn load_config() -> foo::Result<AppConfig> { foo::from_env() }");
}

#[test]
fn dependency_injection() {
    test("fn handler(service: foo::Arc<dyn foo::Service>) -> foo::Response {}");
    test("#[foo::inject] fn process(#[foo::service] svc: &foo::MyService) {}");
}

// Error condition and edge case tests

#[test]
fn incomplete_statements() {
    // These should parse or fail gracefully without panicking
    let _ = collect_imports("use foo::");
    let _ = collect_imports("fn main() { foo:: }");
    let _ = collect_imports("struct S { field: foo:: }");
}

#[test]
fn deeply_nested_generics() {
    test("type Complex = foo::Outer<foo::Middle<foo::Inner<foo::Deep<String>>>>;");
}

#[test]
fn recursive_type_definitions() {
    test("enum List { Cons(i32, foo::Box<List>), Nil }");
}

#[test]
fn higher_kinded_types() {
    test("trait Functor<F> { type Applied<T>: foo::IntoIterator<Item = T>; }");
}

#[test]
fn complex_where_clauses() {
    let source = r#"
    fn complex<T, U, V>()
    where
        T: foo::Clone + foo::Debug,
        U: foo::Into<T> + foo::Send,
        V: for<'a> foo::Fn(&'a T) -> U
    {}
    "#;
    test(source);
}

// Performance edge cases

#[test]
fn many_small_imports() {
    let mut source = String::new();
    for i in 0..1000 {
        source.push_str(&format!("use foo::item{};\n", i));
    }
    let deps = collect_imports(&source).unwrap();
    let expected = HashSet::from_iter(["foo".to_owned()]);
    assert_eq!(deps, expected);
}

#[test]
fn deeply_nested_modules() {
    let mut source = String::new();
    for i in 0..100 {
        source.push_str(&format!("mod level{} {{ use foo::item{}; }}\n", i, i));
    }
    let deps = collect_imports(&source).unwrap();
    let expected = HashSet::from_iter(["foo".to_owned()]);
    assert_eq!(deps, expected);
}
