#![expect(clippy::panic_in_result_fn, reason = "This is a test module, panicking is fine")]

use std::{error::Error, fs, io, path::Path, process::ExitCode};

use cargo_shear::{CargoShear, CargoShearOptions};
use cargo_toml::Manifest;
use tempfile::TempDir;

/// Helper function to copy a fixture to a temporary directory for testing
fn copy_fixture_to_temp(fixture_path: &str) -> io::Result<TempDir> {
    let full_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(fixture_path);

    let temp_dir = TempDir::new()?;
    copy_dir_recursive(&full_path, temp_dir.path())?;

    Ok(temp_dir)
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    if src.is_dir() {
        fs::create_dir_all(dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            copy_dir_recursive(&src_path, &dst_path)?;
        }
    } else {
        fs::copy(src, dst)?;
    }

    Ok(())
}

/// Run cargo-shear on a fixture and return the exit code, output and directory
fn run_cargo_shear(
    fixture_path: &str,
    fix: bool,
) -> Result<(ExitCode, String, TempDir), Box<dyn Error>> {
    let temp_dir = copy_fixture_to_temp(fixture_path)?;
    let options = CargoShearOptions::new_for_test(temp_dir.path().to_path_buf(), fix);

    let mut output = Vec::new();
    let shear = CargoShear::new(&mut output, options);
    let exit_code = shear.run();

    // Redact any mentions of the temp dir, for stable snapshots.
    let mut output = String::from_utf8(output)?;
    let path = temp_dir.path().to_string_lossy();
    output = output.replace(&*path, ".");

    Ok((exit_code, output, temp_dir))
}

// `anyhow` is declared and used, so nothing should be flagged.
#[test]
fn clean_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("clean", false)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    Analyzing .

    No unused dependencies!
    ");

    Ok(())
}

// All dependencies are used, so none should be removed when fixing.
#[test]
fn clean_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("clean", true)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dependencies.contains_key("anyhow"));
    assert!(manifest.dependencies.contains_key("rustc-hash"));
    assert!(manifest.dependencies.contains_key("serde_json"));
    assert!(manifest.dependencies.contains_key("smallvec-v1"));

    Ok(())
}

// Workspace dependency `anyhow` is inherited and used by a workspace member.
#[test]
fn clean_workspace_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("clean_workspace", false)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    Analyzing .

    No unused dependencies!
    ");

    Ok(())
}

// All workspace dependencies are in use and should not be removed.
#[test]
fn clean_workspace_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("clean_workspace", true)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let workspace = &manifest.workspace.as_ref().unwrap().dependencies;
    assert!(workspace.contains_key("anyhow"));
    assert!(workspace.contains_key("rustc-hash"));
    assert!(workspace.contains_key("serde_json"));
    assert!(workspace.contains_key("smallvec-v1"));

    Ok(())
}

// `anyhow` is unused but suppressed via package ignore config.
#[test]
fn ignored() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("ignored", false)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    Analyzing .

    No unused dependencies!
    ");

    Ok(())
}

// `anywho` is in the ignored list but doesn't exist as a dependency.
#[test]
fn ignored_invalid() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("ignored_invalid", false)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    Analyzing .

    warning: 'anywho' is redundant in [package.metadata.cargo-shear] for package 'ignored_invalid'.

    No unused dependencies!
    ");

    Ok(())
}

// `anyhow` is in the ignored list but is actually being used.
#[test]
fn ignored_redundant() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("ignored_redundant", false)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    Analyzing .

    warning: 'anyhow' is redundant in [package.metadata.cargo-shear] for package 'ignored_redundant'.

    No unused dependencies!
    ");

    Ok(())
}

// `anyhow` is unused but suppressed via workspace ignore config.
#[test]
fn ignored_workspace() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("ignored_workspace", false)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    Analyzing .

    No unused dependencies!
    ");

    Ok(())
}

// Both `anyhow` (workspace ignore) and `thiserror` (package ignore) are unused, but ignored.
#[test]
fn ignored_workspace_merged() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("ignored_workspace_merged", false)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    Analyzing .

    No unused dependencies!
    ");

    Ok(())
}

// `anyhow` is only used in tests but declared in `dependencies` instead of `dev-dependencies`.
#[test]
#[ignore = "Unimplemented: #47"]
fn misplaced_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("misplaced", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @"");

    Ok(())
}

// `anyhow` should be moved from `dependencies` to `dev-dependencies`.
#[test]
#[ignore = "Unimplemented: #47"]
fn misplaced_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("misplaced", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dev_dependencies.contains_key("anyhow"));
    assert!(!manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// Optional `anyhow` is only used in tests but declared in `dependencies`.
#[test]
#[ignore = "Unimplemented: #47"]
fn misplaced_optional_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("misplaced_optional", false)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @"");

    Ok(())
}

// Optional `anyhow` can't be moved to `dev-dependencies` since they don't support `optional = true`.
#[test]
#[ignore = "Unimplemented: #47"]
fn misplaced_optional_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("misplaced_optional", true)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dependencies.contains_key("anyhow"));
    assert!(!manifest.dev_dependencies.contains_key("anyhow"));

    Ok(())
}

// Renamed dependency `anyhow_v1` is only used in tests but declared in `dependencies`.
#[test]
#[ignore = "Unimplemented: #47"]
fn misplaced_renamed_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("misplaced_renamed", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @"");

    Ok(())
}

// Renamed `anyhow_v1` should be moved to `dev-dependencies` while maintaining package details.
#[test]
#[ignore = "Unimplemented: #47"]
fn misplaced_renamed_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("misplaced_renamed", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dev_dependencies.contains_key("anyhow_v1"));
    assert!(!manifest.dependencies.contains_key("anyhow_v1"));

    let anyhow = manifest.dev_dependencies.get("anyhow_v1").expect("anyhow_v1 in dev");
    let anyhow_details = anyhow.detail().expect("anyhow_v1 has details");
    assert_eq!(anyhow_details.package.as_deref(), Some("anyhow"));

    Ok(())
}

// Table syntax `anyhow` is only used in tests but declared in `dependencies`.
#[test]
#[ignore = "Unimplemented: #47"]
fn misplaced_table_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("misplaced_table", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @"");

    Ok(())
}

// Table syntax `anyhow` should be moved to `dev-dependencies` while maintaining package details.
#[test]
#[ignore = "Unimplemented: #47"]
fn misplaced_table_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("misplaced_table", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dev_dependencies.contains_key("anyhow"));
    assert!(!manifest.dependencies.contains_key("anyhow"));

    let anyhow = manifest.dev_dependencies.get("anyhow").expect("anyhow in dev");
    let anyhow_details = anyhow.detail().expect("anyhow has details");
    assert!(!anyhow_details.default_features);
    assert_eq!(anyhow_details.features, vec!["std"]);

    Ok(())
}

// `anyhow` is unused.
#[test]
fn unused_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    Analyzing .

    unused -- Cargo.toml:
      anyhow


    cargo-shear may have detected unused dependencies incorrectly due to its limitations.
    They can be ignored by adding the crate name to the package's Cargo.toml:

    [package.metadata.cargo-shear]
    ignored = ["crate-name"]

    or in the workspace Cargo.toml:

    [workspace.metadata.cargo-shear]
    ignored = ["crate-name"]
    "#);

    Ok(())
}

// Unused `anyhow` should be removed from `dependencies`.
#[test]
fn unused_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// `anyhow` is unused in build scripts.
#[test]
fn unused_build_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_build", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    Analyzing .

    unused_build -- Cargo.toml:
      anyhow


    cargo-shear may have detected unused dependencies incorrectly due to its limitations.
    They can be ignored by adding the crate name to the package's Cargo.toml:

    [package.metadata.cargo-shear]
    ignored = ["crate-name"]

    or in the workspace Cargo.toml:

    [workspace.metadata.cargo-shear]
    ignored = ["crate-name"]
    "#);

    Ok(())
}

// Unused `anyhow` should be removed from `build-dependencies`.
#[test]
fn unused_build_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_build", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.build_dependencies.contains_key("anyhow"));

    Ok(())
}

// `anyhow` is unused in dev targets.
#[test]
fn unused_dev_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_dev", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    Analyzing .

    unused_dev -- Cargo.toml:
      anyhow


    cargo-shear may have detected unused dependencies incorrectly due to its limitations.
    They can be ignored by adding the crate name to the package's Cargo.toml:

    [package.metadata.cargo-shear]
    ignored = ["crate-name"]

    or in the workspace Cargo.toml:

    [workspace.metadata.cargo-shear]
    ignored = ["crate-name"]
    "#);

    Ok(())
}

// Unused `anyhow` should be removed from `dev-dependencies`.
#[test]
fn unused_dev_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_dev", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dev_dependencies.contains_key("anyhow"));

    Ok(())
}

// `anyhow` is unused in code but referenced in a feature, so it can't be safely removed.
#[test]
fn unused_feature_detect() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_feature", false)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    Analyzing .

    No unused dependencies!
    ");

    Ok(())
}

// `anyhow` should remain since it's referenced in a feature, even though unused in code.
#[test]
fn unused_feature_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_feature", true)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// `anyhow` is unused in code but referenced in a weak feature, so it can't be safely removed.
#[test]
fn unused_feature_weak_detect() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_feature_weak", false)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    Analyzing .

    No unused dependencies!
    ");

    Ok(())
}

// `anyhow` should remain since it's referenced in a weak feature, even though unused in code.
#[test]
fn unused_feature_weak_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_feature_weak", true)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// `serde_json` (import `serde_json`) is not used in code.
#[test]
fn unused_naming_hyphen_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_naming_hyphen", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    Analyzing .

    unused_naming_hyphen -- Cargo.toml:
      serde_json


    cargo-shear may have detected unused dependencies incorrectly due to its limitations.
    They can be ignored by adding the crate name to the package's Cargo.toml:

    [package.metadata.cargo-shear]
    ignored = ["crate-name"]

    or in the workspace Cargo.toml:

    [workspace.metadata.cargo-shear]
    ignored = ["crate-name"]
    "#);

    Ok(())
}

// `serde_json` should be removed when fixing.
#[test]
fn unused_naming_hyphen_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_naming_hyphen", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dependencies.contains_key("serde_json"));

    Ok(())
}

// `rustc-hash` (import `rustc_hash`) is not used in code.
#[test]
fn unused_naming_underscore_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_naming_underscore", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    Analyzing .

    unused_naming_underscore -- Cargo.toml:
      rustc-hash


    cargo-shear may have detected unused dependencies incorrectly due to its limitations.
    They can be ignored by adding the crate name to the package's Cargo.toml:

    [package.metadata.cargo-shear]
    ignored = ["crate-name"]

    or in the workspace Cargo.toml:

    [workspace.metadata.cargo-shear]
    ignored = ["crate-name"]
    "#);

    Ok(())
}

// `rustc-hash` should be removed when fixing.
#[test]
fn unused_naming_underscore_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_naming_underscore", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dependencies.contains_key("rustc-hash"));

    Ok(())
}

// Optional `anyhow` enabled via `dep:anyhow` is unused but can't be removed without breaking the feature.
#[test]
fn unused_optional_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_optional", false)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    Analyzing .

    No unused dependencies!
    ");

    Ok(())
}

// Unused optional `anyhow` enabled via `dep:anyhow` can't be removed since that would break the feature.
#[test]
fn unused_optional_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_optional", true)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dependencies.contains_key("anyhow"));
    assert_eq!(
        manifest.features.get("anyhow").expect("anyhow feature should exist"),
        &vec!["dep:anyhow"]
    );

    Ok(())
}

// Optional `anyhow` with implicit feature is unused but can't be removed without removing the feature.
#[test]
fn unused_optional_implicit_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_optional_implicit", false)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    Analyzing .

    No unused dependencies!
    ");

    Ok(())
}

// Unused optional `anyhow` with implicit feature can't be removed since that would remove the feature.
#[test]
fn unused_optional_implicit_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_optional_implicit", true)?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// Target specific `anyhow` is unused.
#[test]
fn unused_platform_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_platform", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    Analyzing .

    unused_platform -- Cargo.toml:
      anyhow


    cargo-shear may have detected unused dependencies incorrectly due to its limitations.
    They can be ignored by adding the crate name to the package's Cargo.toml:

    [package.metadata.cargo-shear]
    ignored = ["crate-name"]

    or in the workspace Cargo.toml:

    [workspace.metadata.cargo-shear]
    ignored = ["crate-name"]
    "#);

    Ok(())
}

// Unused target specific `anyhow` should be removed.
#[test]
fn unused_platform_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_platform", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let windows = manifest.target.get("cfg(windows)");
    assert!(!windows.is_some_and(|table| table.dependencies.contains_key("anyhow")));

    Ok(())
}

// Renamed `anyhow_v1` is unused.
#[test]
fn unused_renamed_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_renamed", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    Analyzing .

    unused_renamed -- Cargo.toml:
      anyhow_v1


    cargo-shear may have detected unused dependencies incorrectly due to its limitations.
    They can be ignored by adding the crate name to the package's Cargo.toml:

    [package.metadata.cargo-shear]
    ignored = ["crate-name"]

    or in the workspace Cargo.toml:

    [workspace.metadata.cargo-shear]
    ignored = ["crate-name"]
    "#);

    Ok(())
}

// Unused renamed `anyhow_v1` should be removed.
#[test]
fn unused_renamed_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_renamed", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dependencies.contains_key("anyhow_v1"));

    Ok(())
}

// Table syntax `anyhow` is unused.
#[test]
fn unused_table_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_table", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    Analyzing .

    unused_table -- Cargo.toml:
      anyhow


    cargo-shear may have detected unused dependencies incorrectly due to its limitations.
    They can be ignored by adding the crate name to the package's Cargo.toml:

    [package.metadata.cargo-shear]
    ignored = ["crate-name"]

    or in the workspace Cargo.toml:

    [workspace.metadata.cargo-shear]
    ignored = ["crate-name"]
    "#);

    Ok(())
}

// Unused table syntax `anyhow` should be removed.
#[test]
fn unused_table_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_table", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// Workspace dependency `anyhow` is not inherited by any workspace member.
#[test]
fn unused_workspace_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_workspace", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    Analyzing .

    root -- ./Cargo.toml:
      anyhow


    cargo-shear may have detected unused dependencies incorrectly due to its limitations.
    They can be ignored by adding the crate name to the package's Cargo.toml:

    [package.metadata.cargo-shear]
    ignored = ["crate-name"]

    or in the workspace Cargo.toml:

    [workspace.metadata.cargo-shear]
    ignored = ["crate-name"]
    "#);

    Ok(())
}

// Unused workspace dependency `anyhow` should be removed.
#[test]
fn unused_workspace_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_workspace", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let workspace = &manifest.workspace.as_ref().unwrap().dependencies;
    assert!(!workspace.contains_key("anyhow"));

    Ok(())
}

// Renamed workspace dependency `anyhow_v1` is unused.
#[test]
fn unused_workspace_renamed_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _) = run_cargo_shear("unused_workspace_renamed", false)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    Analyzing .

    root -- ./Cargo.toml:
      anyhow_v1


    cargo-shear may have detected unused dependencies incorrectly due to its limitations.
    They can be ignored by adding the crate name to the package's Cargo.toml:

    [package.metadata.cargo-shear]
    ignored = ["crate-name"]

    or in the workspace Cargo.toml:

    [workspace.metadata.cargo-shear]
    ignored = ["crate-name"]
    "#);

    Ok(())
}

// Unused renamed workspace dependency `anyhow_v1` should be removed.
#[test]
fn unused_workspace_renamed_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _, temp_dir) = run_cargo_shear("unused_workspace_renamed", true)?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let workspace = &manifest.workspace.as_ref().unwrap().dependencies;
    assert!(!workspace.contains_key("anyhow_v1"));

    Ok(())
}
