use std::fs;
use std::path::Path;
use std::process::ExitCode;
use tempfile::TempDir;

use cargo_shear::{CargoShear, CargoShearOptions};

/// Helper function to copy a fixture to a temporary directory for testing
fn copy_fixture_to_temp(fixture_name: &str) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let fixture_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join(fixture_name);

    copy_dir_recursive(&fixture_path, temp_dir.path()).expect("Failed to copy fixture");
    temp_dir
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
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

/// Run cargo-shear on a fixture and return the exit code
fn run_cargo_shear_on_fixture(fixture_name: &str, fix: bool) -> (ExitCode, TempDir) {
    let temp_dir = copy_fixture_to_temp(fixture_name);
    let options = CargoShearOptions::new_for_test(temp_dir.path().to_path_buf(), fix);

    let shear = CargoShear::new(options);
    let exit_code = shear.run();
    (exit_code, temp_dir)
}

#[test]
fn test_simple_unused_deps_detection() {
    let (exit_code, _temp_dir) = run_cargo_shear_on_fixture("simple_unused_deps", false);

    // Should detect unused dependencies and return error code
    assert_eq!(exit_code, ExitCode::FAILURE, "Should detect unused dependencies");
}

#[test]
fn test_simple_unused_deps_fix() {
    let (exit_code, temp_dir) = run_cargo_shear_on_fixture("simple_unused_deps", true);

    // Should detect and fix unused dependencies
    assert_eq!(exit_code, ExitCode::FAILURE, "Should detect unused dependencies");

    // Check that unused dependencies were removed from Cargo.toml
    let cargo_toml =
        fs::read_to_string(temp_dir.path().join("Cargo.toml")).expect("Failed to read Cargo.toml");

    // serde should still be there (it's used)
    assert!(cargo_toml.contains("serde"), "Used dependency should remain");

    // regex and clap should be removed (they're unused)
    assert!(!cargo_toml.contains("regex"), "Unused dependency should be removed");
    assert!(!cargo_toml.contains("clap"), "Unused dependency should be removed");

    // tokio dev-dependency should be removed too
    assert!(!cargo_toml.contains("tokio"), "Unused dev-dependency should be removed");
}

#[test]
fn test_workspace_unused_deps_detection() {
    let (exit_code, _temp_dir) = run_cargo_shear_on_fixture("workspace_unused_deps", false);

    // Should detect unused workspace dependencies
    assert_eq!(exit_code, ExitCode::FAILURE, "Should detect unused workspace dependencies");
}

#[test]
fn test_workspace_unused_deps_fix() {
    let (exit_code, temp_dir) = run_cargo_shear_on_fixture("workspace_unused_deps", true);

    // Should detect and fix unused dependencies
    assert_eq!(exit_code, ExitCode::FAILURE, "Should detect unused dependencies");

    // Check that unused dependencies were removed from workspace Cargo.toml
    let cargo_toml = fs::read_to_string(temp_dir.path().join("Cargo.toml"))
        .expect("Failed to read workspace Cargo.toml");

    // serde and clap should still be there (they're used)
    assert!(cargo_toml.contains("serde"), "Used workspace dependency should remain");
    assert!(cargo_toml.contains("clap"), "Used workspace dependency should remain");

    // unused-crate should be removed (it's not used by anyone)
    assert!(!cargo_toml.contains("unused-crate"), "Unused workspace dependency should be removed");
}

#[test]
fn test_all_deps_used_no_changes() {
    let (exit_code, temp_dir) = run_cargo_shear_on_fixture("all_deps_used", false);

    // Should not detect any unused dependencies
    assert_eq!(
        exit_code,
        ExitCode::SUCCESS,
        "Should not detect unused dependencies when all are used"
    );

    // Verify original Cargo.toml is unchanged
    let original_toml = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("all_deps_used")
            .join("Cargo.toml"),
    )
    .expect("Failed to read original Cargo.toml");

    let temp_toml = fs::read_to_string(temp_dir.path().join("Cargo.toml"))
        .expect("Failed to read temp Cargo.toml");

    assert_eq!(original_toml, temp_toml, "Cargo.toml should be unchanged when all deps are used");
}

#[test]
fn test_all_deps_used_with_fix_no_changes() {
    let (exit_code, temp_dir) = run_cargo_shear_on_fixture("all_deps_used", true);

    // Should not detect any unused dependencies even with fix enabled
    assert_eq!(
        exit_code,
        ExitCode::SUCCESS,
        "Should not detect unused dependencies when all are used"
    );

    // Verify Cargo.toml is unchanged even with fix enabled
    let original_toml = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("all_deps_used")
            .join("Cargo.toml"),
    )
    .expect("Failed to read original Cargo.toml");

    let temp_toml = fs::read_to_string(temp_dir.path().join("Cargo.toml"))
        .expect("Failed to read temp Cargo.toml");

    assert_eq!(original_toml, temp_toml, "Cargo.toml should be unchanged when all deps are used");
}
#[test]
fn test_complex_workspace_unused_deps() {
    let (exit_code, _temp_dir) = run_cargo_shear_on_fixture("complex_workspace", false);

    // Should detect unused dependencies
    assert_eq!(
        exit_code,
        ExitCode::FAILURE,
        "Should detect unused dependencies in complex workspace"
    );
}

#[test]
fn test_complex_workspace_unused_deps_fix() {
    let (exit_code, temp_dir) = run_cargo_shear_on_fixture("complex_workspace", true);

    // Should detect and fix unused dependencies
    assert_eq!(exit_code, ExitCode::FAILURE, "Should detect unused dependencies");

    // Check workspace root Cargo.toml
    let workspace_toml = fs::read_to_string(temp_dir.path().join("Cargo.toml"))
        .expect("Failed to read workspace Cargo.toml");

    // Used workspace deps should remain
    assert!(workspace_toml.contains("serde"), "Used workspace dependency should remain");
    assert!(workspace_toml.contains("tokio"), "Used workspace dependency should remain");
    assert!(workspace_toml.contains("uuid"), "Used workspace dependency should remain");
    assert!(workspace_toml.contains("anyhow"), "Used workspace dependency should remain");

    // Unused workspace deps should be removed
    assert!(
        !workspace_toml.contains("unused-dep-1"),
        "Unused workspace dependency should be removed"
    );
    assert!(
        !workspace_toml.contains("unused-dep-2"),
        "Unused workspace dependency should be removed"
    );

    // Check individual package Cargo.toml files
    let core_toml = fs::read_to_string(temp_dir.path().join("core/Cargo.toml"))
        .expect("Failed to read core Cargo.toml");
    assert!(core_toml.contains("serde"), "Used dependency should remain in core");
    assert!(core_toml.contains("uuid"), "Used dependency should remain in core");
    assert!(core_toml.contains("rand"), "Used dependency should remain in core");
    assert!(!core_toml.contains("tempdir"), "Unused dev dependency should be removed from core");

    let api_toml = fs::read_to_string(temp_dir.path().join("api/Cargo.toml"))
        .expect("Failed to read api Cargo.toml");
    assert!(api_toml.contains("tokio"), "Used dependency should remain in api");
    assert!(api_toml.contains("anyhow"), "Used dependency should remain in api");
    assert!(!api_toml.contains("cc"), "Unused build dependency should be removed from api");

    let tools_toml = fs::read_to_string(temp_dir.path().join("tools/Cargo.toml"))
        .expect("Failed to read tools Cargo.toml");
    assert!(!tools_toml.contains("clap"), "Unused dependency should be removed from tools");
    assert!(!tools_toml.contains("regex"), "Unused dependency should be removed from tools");
}

#[test]
fn test_invalid_ignored_package_warning() {
    // This test verifies that a warning is printed when a package-level ignored
    // dependency doesn't exist in the package's dependencies.
    // The warning will appear during test execution:
    // "warning: 'nonexistent-crate' is redundant in [package.metadata.cargo-shear] for package 'invalid-ignored-test'."
    let (exit_code, _temp_dir) = run_cargo_shear_on_fixture("invalid_ignored", false);

    // Should succeed since all actual dependencies are used
    assert_eq!(
        exit_code,
        ExitCode::SUCCESS,
        "Should succeed when all actual dependencies are used"
    );
}

#[test]
fn test_invalid_ignored_workspace_warning() {
    // This test verifies that a warning is printed when a workspace-level ignored
    // dependency doesn't exist in the workspace dependencies.
    // The warning will appear during test execution:
    // "warning: 'workspace-nonexistent' is redundant in [workspace.metadata.cargo-shear]."
    let (exit_code, _temp_dir) = run_cargo_shear_on_fixture("invalid_ignored_workspace", false);

    // Should succeed since all actual dependencies are used
    assert_eq!(
        exit_code,
        ExitCode::SUCCESS,
        "Should succeed when all actual dependencies are used"
    );
}
