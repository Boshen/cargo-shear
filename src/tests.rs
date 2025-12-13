use std::{fmt::Write, path::Path, process::ExitCode};

use rustc_hash::FxHashSet;

use crate::{
    CargoShear, CargoShearOptions, default_path,
    output::{ColorMode, OutputFormat},
    source_parser::ParsedSource,
};

#[track_caller]
fn test(source_text: &str) {
    let parsed = ParsedSource::from_str(source_text, Path::new("lib.rs"));
    let expected = FxHashSet::from_iter(["foo".to_owned()]);
    assert_eq!(parsed.imports, expected, "{source_text}");
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
fn macro_extern_crate() {
    test("my_macro! { extern crate foo; }");
    test("my_macro!( extern crate foo; );");
    test("my_macro![ extern crate foo; ];");
}

#[test]
fn macro_use() {
    test("my_macro! { use foo; }");
    test("my_macro!( use foo; );");
}

#[test]
fn macro_use_path() {
    test("my_macro! { use foo::bar; }");
    test("my_macro!( use foo::bar; );");
}

#[test]
fn macro_use_group() {
    test_multiple_deps("my_macro! { use {foo, bar}; }", &["foo", "bar"]);
    test_multiple_deps("my_macro!( use {foo, bar}; );", &["foo", "bar"]);
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
    let shear = CargoShear::new(
        std::io::sink(),
        CargoShearOptions {
            fix: false,
            locked: false,
            offline: false,
            frozen: false,
            package: vec![],
            exclude: vec![],
            path: default_path().unwrap(),
            expand: false,
            format: OutputFormat::Auto,
            color: ColorMode::Never,
        },
    );
    let exit_code = shear.run();
    assert_eq!(exit_code, ExitCode::SUCCESS);
}

// Additional import collection tests for edge cases

#[test]
fn empty_source() {
    let parsed = ParsedSource::from_str("", Path::new("lib.rs"));
    assert!(parsed.imports.is_empty());
}

#[test]
fn comment_only() {
    let parsed = ParsedSource::from_str("// this is a comment", Path::new("lib.rs"));
    assert!(parsed.imports.is_empty());
}

#[test]
fn std_imports_not_collected() {
    let parsed = ParsedSource::from_str("use std::collections::HashMap;", Path::new("lib.rs"));
    assert!(parsed.imports.is_empty());
}

#[test]
fn self_super_crate_not_collected() {
    let parsed = ParsedSource::from_str("use self::module;", Path::new("lib.rs"));
    assert!(parsed.imports.is_empty());

    let parsed = ParsedSource::from_str("use super::module;", Path::new("lib.rs"));
    assert!(parsed.imports.is_empty());

    let parsed = ParsedSource::from_str("use crate::module;", Path::new("lib.rs"));
    assert!(parsed.imports.is_empty());
}

#[test]
fn multiple_imports_same_crate() {
    let source = r"
        use foo::bar;
        use foo::baz;
        fn main() {
            foo::qux();
        }
    ";
    let parsed = ParsedSource::from_str(source, Path::new("lib.rs"));
    let expected = FxHashSet::from_iter(["foo".to_owned()]);
    assert_eq!(parsed.imports, expected);
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
    let parsed = ParsedSource::from_str("use foo::;", Path::new("lib.rs"));
    assert!(parsed.imports.contains("foo"));
}

#[test]
fn very_long_path() {
    let long_path = "foo::".repeat(100) + "bar";
    let source = format!("use {long_path};");
    let parsed = ParsedSource::from_str(&source, Path::new("lib.rs"));
    let expected = FxHashSet::from_iter(["foo".to_owned()]);
    assert_eq!(parsed.imports, expected);
}

#[test]
fn unicode_identifiers() {
    let parsed = ParsedSource::from_str("use foo::数据;", Path::new("lib.rs"));
    let expected = FxHashSet::from_iter(["foo".to_owned()]);
    assert_eq!(parsed.imports, expected);
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
    let parsed = ParsedSource::from_str("use foo::*;", Path::new("lib.rs"));
    let expected = FxHashSet::from_iter(["foo".to_owned()]);
    assert_eq!(parsed.imports, expected);
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
    let shear = CargoShear::new(
        std::io::sink(),
        CargoShearOptions {
            fix: true,
            locked: false,
            offline: false,
            frozen: false,
            package: vec![],
            exclude: vec![],
            path: default_path().unwrap(),
            expand: false,
            format: OutputFormat::Auto,
            color: ColorMode::Never,
        },
    );
    let exit_code = shear.run();
    assert_eq!(exit_code, ExitCode::SUCCESS);
}

#[test]
fn cargo_shear_with_package_filter() {
    // Test with specific package filtering
    let shear = CargoShear::new(
        std::io::sink(),
        CargoShearOptions {
            fix: false,
            locked: false,
            offline: false,
            frozen: false,
            package: vec!["cargo-shear".to_owned()],
            exclude: vec![],
            path: default_path().unwrap(),
            expand: false,
            format: OutputFormat::Auto,
            color: ColorMode::Never,
        },
    );
    let exit_code = shear.run();
    assert_eq!(exit_code, ExitCode::SUCCESS);
}

#[test]
fn cargo_shear_with_exclude_filter() {
    // Test with package exclusion
    let shear = CargoShear::new(
        std::io::sink(),
        CargoShearOptions {
            fix: false,
            locked: false,
            offline: false,
            frozen: false,
            package: vec![],
            exclude: vec!["some-package".to_owned()],
            path: default_path().unwrap(),
            expand: false,
            format: OutputFormat::Auto,
            color: ColorMode::Never,
        },
    );
    let exit_code = shear.run();
    assert_eq!(exit_code, ExitCode::SUCCESS);
}

#[test]
fn cargo_shear_options_creation() {
    // Test that we can create options with all combinations
    let options = CargoShearOptions {
        fix: true,
        locked: false,
        offline: false,
        frozen: false,
        package: vec!["test1".to_owned(), "test2".to_owned()],
        exclude: vec!["exclude1".to_owned()],
        path: std::path::PathBuf::from("/tmp"),
        expand: true,
        format: OutputFormat::Auto,
        color: ColorMode::Never,
    };

    let shear = CargoShear::new(std::io::sink(), options.clone());
    // Verify the shear instance was created successfully
    assert_eq!(format!("{:?}", shear.options.fix), format!("{:?}", options.fix));
}

// Error handling and edge case tests

#[test]
fn invalid_rust_syntax() {
    let parsed = ParsedSource::from_str("this is not rust code ^^^", Path::new("lib.rs"));
    assert!(parsed.imports.is_empty(), "Should gracefully handle invalid Rust syntax");
}

#[test]
fn empty_file_handling() {
    let parsed = ParsedSource::from_str("", Path::new("lib.rs"));
    assert!(parsed.imports.is_empty(), "Empty file should result in no dependencies");
}

#[test]
fn whitespace_only() {
    let parsed = ParsedSource::from_str("   \n  \t  \n  ", Path::new("lib.rs"));
    assert!(parsed.imports.is_empty(), "Whitespace-only file should result in no dependencies");
}

#[test]
fn mixed_valid_invalid_imports() {
    // Test a file with some valid and some edge-case imports
    let source = r"
        use foo::bar;  // valid
        use std::collections::HashMap;  // should be ignored (std)
        use self::local;  // should be ignored (self)
        use foo::baz::qux;  // valid, same crate as first
    ";
    let parsed = ParsedSource::from_str(source, Path::new("lib.rs"));
    let expected = FxHashSet::from_iter(["foo".to_owned()]);
    assert_eq!(parsed.imports, expected);
}

// Test helper function variations

#[track_caller]
fn test_no_deps(source_text: &str) {
    let parsed = ParsedSource::from_str(source_text, Path::new("lib.rs"));
    assert!(parsed.imports.is_empty(), "Expected no dependencies for: {source_text}");
}

#[track_caller]
fn test_multiple_deps(source_text: &str, expected_deps: &[&str]) {
    let parsed = ParsedSource::from_str(source_text, Path::new("lib.rs"));
    let expected = expected_deps.iter().map(|s| (*s).to_owned()).collect::<FxHashSet<_>>();
    assert_eq!(parsed.imports, expected, "Dependencies mismatch for: {source_text}");
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
        writeln!(source, "use foo::module{i};").unwrap();
        writeln!(source, "fn func{i}() {{ foo::call{i}(); }}").unwrap();
    }

    let parsed = ParsedSource::from_str(&source, Path::new("lib.rs"));
    let expected = FxHashSet::from_iter(["foo".to_owned()]);
    assert_eq!(parsed.imports, expected);
}

#[test]
fn deeply_nested_paths() {
    let nested_path = (0..20).map(|i| format!("level{i}")).collect::<Vec<_>>().join("::");
    let source = format!("use foo::{nested_path};");
    let parsed = ParsedSource::from_str(&source, Path::new("lib.rs"));
    let expected = FxHashSet::from_iter(["foo".to_owned()]);
    assert_eq!(parsed.imports, expected);
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
    let parsed = ParsedSource::from_str("use r#continue::thing;", Path::new("lib.rs"));
    assert!(parsed.imports.contains("continue"));
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
        locked: false,
        offline: false,
        frozen: false,
        package: vec![],
        exclude: vec![],
        path: default_path().unwrap(),
        expand: false,
        format: OutputFormat::Auto,
        color: ColorMode::Never,
    };
    let _shear = CargoShear::new(std::io::sink(), options);
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
    // These should parse without panicking

    let parsed = ParsedSource::from_str("use foo::", Path::new("lib.rs"));
    assert!(parsed.imports.contains("foo"));

    let parsed = ParsedSource::from_str("fn main() { foo:: }", Path::new("lib.rs"));
    assert!(parsed.imports.is_empty());

    let parsed = ParsedSource::from_str("struct S { field: foo:: }", Path::new("lib.rs"));
    assert!(parsed.imports.is_empty());
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
    let source = r"
    fn complex<T, U, V>()
    where
        T: foo::Clone + foo::Debug,
        U: foo::Into<T> + foo::Send,
        V: for<'a> foo::Fn(&'a T) -> U
    {}
    ";
    test(source);
}

// Performance edge cases

#[test]
fn many_small_imports() {
    let mut source = String::new();
    for i in 0..1000 {
        writeln!(source, "use foo::item{i};").unwrap();
    }
    let parsed = ParsedSource::from_str(&source, Path::new("lib.rs"));
    let expected = FxHashSet::from_iter(["foo".to_owned()]);
    assert_eq!(parsed.imports, expected);
}

#[test]
fn deeply_nested_modules() {
    let mut source = String::new();
    for i in 0..100 {
        writeln!(source, "mod level{i} {{ use foo::item{i}; }}").unwrap();
    }
    let parsed = ParsedSource::from_str(&source, Path::new("lib.rs"));
    let expected = FxHashSet::from_iter(["foo".to_owned()]);
    assert_eq!(parsed.imports, expected);
}

// Additional tests for edge cases and special patterns

#[test]
fn whitespace_variations_in_paths() {
    // Test with excessive whitespace between :: and identifiers
    test("fn main() { foo   ::   bar(); }");
    test("fn main() { foo\t::\tbar(); }");
    test("fn main() { foo\n::\nbar(); }");
    test("use foo\n    ::\n    bar;");

    // Mixed whitespace patterns
    test("fn main() { foo  :: bar  ::  baz(); }");
    test("fn main() { foo::\n\n\tbar(); }");

    // Whitespace in derive macros
    test("#[derive(foo   ::   Deserialize)] struct S;");
    test("#[derive(foo\t\t::\t\tSerialize)] struct S;");
}

#[test]
fn macro_invocation_complex_patterns() {
    // Nested macros with paths
    test_multiple_deps("fn main() { foo::macro1!(bar::macro2!()); }", &["foo", "bar"]);
    test_multiple_deps("fn main() { println!(\"{}\", foo::bar!(baz::qux())); }", &["foo", "baz"]);

    // Multiple :: patterns in single macro
    test_multiple_deps(r#"fn main() { println!("{} {}", foo::bar, baz::qux); }"#, &["foo", "baz"]);
    #[expect(
        clippy::literal_string_with_formatting_args,
        reason = "Testing format string handling"
    )]
    test_multiple_deps(
        r#"fn main() { format!("{:?} {:?}", foo::Debug, bar::Display); }"#,
        &["foo", "bar"],
    );

    // Macro with complex token streams
    test_multiple_deps(
        "fn main() { foo::my_macro! { struct S { field: bar::Type } } }",
        &["foo", "bar"],
    );
    test_multiple_deps(
        "fn main() { foo::define! { fn func() -> bar::Result<()> { Ok(()) } } }",
        &["foo", "bar"],
    );
}

#[test]
fn raw_identifier_combinations() {
    // Raw identifiers in both positions
    let parsed = ParsedSource::from_str("use r#foo::r#type;", Path::new("lib.rs"));
    assert!(parsed.imports.contains("foo"));

    let parsed = ParsedSource::from_str("fn main() { r#foo::r#match(); }", Path::new("lib.rs"));
    assert!(parsed.imports.contains("foo"));

    // Raw identifier only on left
    let parsed = ParsedSource::from_str("use r#async::bar;", Path::new("lib.rs"));
    assert!(parsed.imports.contains("async"));

    let parsed = ParsedSource::from_str("fn main() { r#type::regular(); }", Path::new("lib.rs"));
    assert!(parsed.imports.contains("type"));

    // Raw identifier only on right
    let parsed = ParsedSource::from_str("use regular::r#await;", Path::new("lib.rs"));
    assert!(parsed.imports.contains("regular"));
    test("fn main() { foo::r#fn(); }");

    // Raw identifiers with whitespace
    let parsed = ParsedSource::from_str("use r#foo  ::  r#bar;", Path::new("lib.rs"));
    assert!(parsed.imports.contains("foo"));

    let parsed =
        ParsedSource::from_str("fn main() { r#type\t::\tr#match(); }", Path::new("lib.rs"));
    assert!(parsed.imports.contains("type"));

    // Raw identifiers in macros
    let parsed = ParsedSource::from_str(
        r#"fn main() { println!("{}", r#foo::r#bar); }"#,
        Path::new("lib.rs"),
    );
    assert!(parsed.imports.contains("foo"));

    let parsed = ParsedSource::from_str("#[derive(r#foo::r#Trait)] struct S;", Path::new("lib.rs"));
    assert!(parsed.imports.contains("foo"));
}

#[test]
fn multiple_colon_patterns() {
    // Three or more colons (should handle gracefully)
    let parsed = ParsedSource::from_str("fn main() { foo:::bar(); }", Path::new("lib.rs"));
    assert!(parsed.imports.contains("foo"));

    let parsed = ParsedSource::from_str("fn main() { foo::::bar(); }", Path::new("lib.rs"));
    assert!(parsed.imports.contains("foo"));

    // Mixed valid and invalid patterns
    let parsed =
        ParsedSource::from_str("fn main() { foo::bar(); baz:::qux(); }", Path::new("lib.rs"));
    let expected = FxHashSet::from_iter(["foo".to_owned(), "baz".to_owned()]);
    assert_eq!(parsed.imports, expected);
}

#[test]
fn verbatim_token_stream_patterns() {
    // Complex verbatim items
    test("#[foo::ext] impl Type {}");
    test("#[foo::async_trait] impl Service for Handler {}");

    // Verbatim with nested paths
    test_multiple_deps("#[foo::config(bar::option = \"value\")] struct Config {}", &["foo", "bar"]);

    // Verbatim with multiple attributes
    test_multiple_deps("#[foo::attr1] #[bar::attr2] impl Thing {}", &["foo", "bar"]);
}

#[test]
fn special_macro_contexts() {
    // Macros in type position
    test("type Alias = foo::type_macro!();");
    test_multiple_deps("const VALUE: foo::Type = bar::value_macro!();", &["foo", "bar"]);

    // Macros in pattern position
    test("fn main() { match x { foo::pattern!() => {} } }");

    // Macros in expression position with paths
    test_multiple_deps("fn main() { let x = foo::create!(bar::Config); }", &["foo", "bar"]);
}

#[test]
fn builder_pattern_usage() {
    test("fn main() { let b = foo::Builder::new().build(); }");
    test("fn create() -> foo::Builder { foo::Builder::default() }");
    test_multiple_deps(
        "fn main() { let result = foo::Builder::new().with_bar(bar::Value).finish(); }",
        &["foo", "bar"],
    );
}

#[test]
fn factory_pattern_usage() {
    test("fn main() { let factory = foo::Factory::create(); }");
    test_multiple_deps(
        "impl foo::Factory for MyFactory { fn produce(&self) -> bar::Product {} }",
        &["foo", "bar"],
    );
}

#[test]
fn async_await_complex_patterns() {
    test("async fn complex() { foo::future.await.unwrap(); }");
    test_multiple_deps(
        "fn main() { foo::block_on(async { bar::async_fn().await }); }",
        &["foo", "bar"],
    );
    test_multiple_deps(
        "fn main() { let handle = foo::spawn(async move { bar::work().await }); }",
        &["foo", "bar"],
    );
}

#[test]
fn derive_macro_variations() {
    // Multiple derives with paths
    test_multiple_deps(
        "#[derive(foo::Trait1, bar::Trait2, baz::Trait3)] struct S;",
        &["foo", "bar", "baz"],
    );

    // Derives with module paths
    test("#[derive(foo::serde::Serialize)] struct S;");

    // Complex derive with attributes
    test("#[derive(foo::Builder)] #[foo(setter(strip_option))] struct S;");
}

#[test]
fn edge_case_path_segments() {
    // Single letter crate names
    let parsed = ParsedSource::from_str("use a::b;", Path::new("lib.rs"));
    assert!(parsed.imports.contains("a"));

    let parsed = ParsedSource::from_str("fn main() { x::y(); }", Path::new("lib.rs"));
    assert!(parsed.imports.contains("x"));

    // Underscore in paths
    let parsed = ParsedSource::from_str("use foo_bar::baz_qux;", Path::new("lib.rs"));
    assert!(parsed.imports.contains("foo_bar"));

    let parsed =
        ParsedSource::from_str("fn main() { snake_case::function_name(); }", Path::new("lib.rs"));
    assert!(parsed.imports.contains("snake_case"));

    // Numbers in identifiers
    let parsed = ParsedSource::from_str("use foo2::bar3;", Path::new("lib.rs"));
    assert!(parsed.imports.contains("foo2"));

    let parsed =
        ParsedSource::from_str("fn main() { crate1::module2::func3(); }", Path::new("lib.rs"));
    assert!(parsed.imports.contains("crate1"));
}

#[test]
fn macro_generated_paths() {
    // Paths that might be generated by macros
    test_multiple_deps("fn main() { <foo::Type as bar::Trait>::method(); }", &["foo", "bar"]);
    test("fn main() { <foo::Type>::associated_function(); }");
}

#[test]
fn qualified_paths_complex() {
    test_multiple_deps(
        "fn process<T: foo::Trait>() { <T as bar::OtherTrait>::method(); }",
        &["foo", "bar"],
    );
    test_multiple_deps("type Alias = <foo::Type as bar::Trait>::AssociatedType;", &["foo", "bar"]);
    test_multiple_deps(
        "fn main() { <<foo::Type as bar::Trait>::Output as baz::Other>::func(); }",
        &["foo", "bar", "baz"],
    );
}

#[test]
fn const_and_static_complex() {
    test_multiple_deps(
        "const ARRAY: [foo::Type; bar::SIZE] = [foo::Type::default(); bar::SIZE];",
        &["foo", "bar"],
    );
    test_multiple_deps(
        "static INSTANCE: foo::Lazy<bar::Service> = foo::Lazy::new(|| bar::Service::new());",
        &["foo", "bar"],
    );
}

#[test]
fn unsafe_code_with_paths() {
    test("unsafe fn call() { foo::unsafe_function(); }");
    test("unsafe impl foo::UnsafeTrait for Type {}");
    test_multiple_deps("fn main() { unsafe { foo::static_mut = bar::value(); } }", &["foo", "bar"]);
}

#[test]
fn macro_rules_complex() {
    test_multiple_deps(
        r"
        macro_rules! complex {
            ($t:ty) => {
                impl foo::Trait for $t {
                    fn method() -> bar::Result<()> {
                        baz::helper()
                    }
                }
            };
        }
    ",
        &["foo", "bar", "baz"],
    );
}

#[test]
fn attribute_macro_patterns() {
    test("#[foo::test] fn test_function() {}");
    test("#[foo::bench] fn benchmark() {}");
    test_multiple_deps("#[foo::ignore] #[bar::should_panic] fn test() {}", &["foo", "bar"]);
}

#[test]
fn doc_attributes_with_paths() {
    test("#[doc = foo::generate_docs!()] struct S;");
    test_no_deps("#[doc(alias = \"foo::bar\")] fn func() {}");
}

#[test]
fn cfg_attributes_with_paths() {
    test("#[cfg(feature = \"foo\")] use foo::feature_gated;");
    test("#[cfg_attr(test, foo::test_attr)] fn func() {}");
}

#[test]
fn nested_generics_extreme() {
    test_multiple_deps(
        "type Nested = foo::A<bar::B<baz::C<qux::D<E>>>>;",
        &["foo", "bar", "baz", "qux"],
    );
    test_multiple_deps("fn process<T: foo::A<bar::B<baz::C>>>() {}", &["foo", "bar", "baz"]);
}

#[test]
fn operator_overloading_paths() {
    test_multiple_deps(
        "impl foo::Add<bar::Type> for MyType { type Output = baz::Result; }",
        &["foo", "bar", "baz"],
    );
    test_multiple_deps(
        "impl foo::ops::Deref for Wrapper { type Target = bar::Inner; }",
        &["foo", "bar"],
    );
}

#[test]
fn test_helper_paths() {
    // Test test multiple deps to ensure it works
    test_multiple_deps("use foo::a; use bar::b; use baz::c;", &["foo", "bar", "baz"]);

    // Test no deps helper
    test_no_deps("// Just comments");
    test_no_deps("fn local_function() {}");
}

#[test]
fn mixed_import_styles() {
    test_multiple_deps(
        r"
        extern crate foo;
        use bar::item;
        fn main() {
            baz::function();
            qux::CONSTANT;
        }
        ",
        &["foo", "bar", "baz", "qux"],
    );
}

#[test]
fn workspace_specific_patterns() {
    // Patterns specific to workspace members
    test("use foo::workspace_member;");
    test("fn main() { foo::internal::helper(); }");
}

#[test]
fn edition_specific_patterns() {
    // 2018 edition and beyond patterns
    test("async fn modern() -> foo::Result<()> { Ok(()) }");
    test("use foo::{self, bar};");
}

#[test]
fn macro_2_0_patterns() {
    test("foo::proc_macro! { input }");
    test("use foo::proc_macro;");
    test("#[foo::proc_macro_attribute] fn func() {}");
}

#[test]
fn associated_const_patterns() {
    test("const VALUE: usize = foo::Trait::ASSOCIATED_CONST;");
    test("fn main() { let x = foo::Type::CONST; }");
}

#[test]
fn turbofish_syntax() {
    test_multiple_deps("fn main() { foo::function::<bar::Type>(); }", &["foo", "bar"]);
    test_multiple_deps(
        "fn main() { let result = foo::parse::<bar::Format>(input); }",
        &["foo", "bar"],
    );
}

#[test]
fn absolute_paths() {
    test("use ::foo;");

    test("use ::foo::bar;");
    test("use ::foo::{bar};");

    test("use ::foo::bar::baz;");
    test("use ::foo::{bar::baz};");

    test("use ::foo::{bar, baz};");

    test("use ::foo::bar::baz::{bax};");
    test("use ::foo::bar::{baz::bax};");
    test("use ::foo::{bar::baz::bax};");

    test_multiple_deps("use ::{foo, bar};", &["foo", "bar"]);
    test_multiple_deps("use ::{foo::bar, foo::baz};", &["foo"]);
}
