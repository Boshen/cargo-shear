#![expect(clippy::panic_in_result_fn, reason = "This is a test module, panicking is fine")]

use std::{error::Error, fs, io, path::Path, process::ExitCode};

use cargo_shear::{CargoShear, CargoShearOptions, ColorMode};
use cargo_toml::Manifest;
use tempfile::TempDir;

/// Test runner for `cargo-shear`.
struct CargoShearRunner {
    fixture: String,
    options_fn: Box<dyn FnOnce(CargoShearOptions) -> CargoShearOptions>,
}

impl CargoShearRunner {
    fn new(fixture: &str) -> Self {
        Self { fixture: fixture.to_owned(), options_fn: Box::new(std::convert::identity) }
    }

    fn options(self, f: impl FnOnce(CargoShearOptions) -> CargoShearOptions + 'static) -> Self {
        Self {
            fixture: self.fixture,
            options_fn: Box::new(|options| f((self.options_fn)(options))),
        }
    }

    /// Run cargo-shear and return the results.
    fn run(self) -> Result<(ExitCode, String, TempDir), Box<dyn Error>> {
        let full_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(&self.fixture);

        let temp_dir = TempDir::new()?;
        Self::copy_dir_recursive(&full_path, temp_dir.path())?;

        let options = (self.options_fn)(
            CargoShearOptions::new(temp_dir.path().to_path_buf()).with_color(ColorMode::Never),
        );

        let mut output = Vec::new();
        let shear = CargoShear::new(&mut output, options);
        let exit_code = shear.run();
        let output = String::from_utf8(output)?;

        Ok((exit_code, output, temp_dir))
    }

    fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
        if src.is_dir() {
            fs::create_dir_all(dst)?;
            for entry in fs::read_dir(src)? {
                let entry = entry?;
                Self::copy_dir_recursive(&entry.path(), &dst.join(entry.file_name()))?;
            }
        } else {
            fs::copy(src, dst)?;
        }

        Ok(())
    }
}

// `anyhow` is declared and used, so nothing should be flagged.
#[test]
fn clean_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("clean").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    shear/summary

      ✓ no issues found
    ");

    Ok(())
}

// All dependencies are used, so none should be removed when fixing.
#[test]
fn clean_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("clean").options(CargoShearOptions::with_fix).run()?;
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
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("clean_workspace").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    shear/summary

      ✓ no issues found
    ");

    Ok(())
}

// All workspace dependencies are in use and should not be removed.
#[test]
fn clean_workspace_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("clean_workspace").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let workspace = &manifest.workspace.as_ref().unwrap().dependencies;
    assert!(workspace.contains_key("anyhow"));
    assert!(workspace.contains_key("rustc-hash"));
    assert!(workspace.contains_key("serde_json"));
    assert!(workspace.contains_key("smallvec-v1"));

    Ok(())
}

// Complex fixture with one of each issue type of issue.
#[test]
#[expect(clippy::too_many_lines, reason = "Output stress test")]
fn complex_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("complex").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_dependency

      × unused dependency `bitflags_v2`
        ╭─[Cargo.toml:28:1]
     27 │ # Unused Renamed
     28 │ bitflags_v2 = { package = "bitflags", version = "2.0" }
        · ─────┬─────
        ·      ╰── not used in code
     29 │ 
        ╰────
      help: remove this dependency

    shear/unused_dependency

      × unused dependency `cfg-if`
        ╭─[Cargo.toml:64:15]
     63 │ # Unused Table
     64 │ [dependencies.cfg-if]
        ·               ───┬──
        ·                  ╰── not used in code
     65 │ version = "1.0"
        ╰────
      help: remove this dependency

    shear/unused_dependency

      × unused dependency `either`
        ╭─[Cargo.toml:16:1]
     15 │ # Unused
     16 │ either = "1.0"
        · ───┬──
        ·    ╰── not used in code
     17 │ 
        ╰────
      help: remove this dependency

    shear/unused_dependency

      × unused dependency `serde`
        ╭─[Cargo.toml:53:1]
     52 │ # Unused Dev
     53 │ serde = "1.0"
        · ──┬──
        ·   ╰── not used in code
     54 │ 
        ╰────
      help: remove this dependency

    shear/unused_dependency

      × unused dependency `version_check`
        ╭─[Cargo.toml:57:1]
     56 │ # Unused Build
     57 │ version_check = "0.9"
        · ──────┬──────
        ·       ╰── not used in code
     58 │ 
        ╰────
      help: remove this dependency

    shear/unused_dependency

      × unused dependency `winapi`
        ╭─[Cargo.toml:61:1]
     60 │ # Unused Platform
     61 │ winapi = "0.3"
        · ───┬──
        ·    ╰── not used in code
     62 │ 
        ╰────
      help: remove this dependency

    shear/unused_optional_dependency

      ⚠ unused optional dependency `itoa`
        ╭─[Cargo.toml:19:1]
     18 │ # Unused Optional
     19 │ itoa = { version = "1.0", optional = true }
        · ──┬─
        ·   ╰── not used in code
     20 │ 
        ╰────

    Advice: 
      ☞ removing an optional dependency may be a breaking change

    Advice: 
      ☞ used in feature `unused-optional`
       ╭─[Cargo.toml:7:20]
     6 │ [features]
     7 │ unused-optional = ["dep:itoa"]
       ·                    ─────┬────
       ·                         ╰── enabled here
     8 │ unused-optional-feature = ["once_cell/std"]
       ╰────

    shear/unused_optional_dependency

      ⚠ unused optional dependency `memchr`
        ╭─[Cargo.toml:25:1]
     24 │ # Unused Optional Feature Weak
     25 │ memchr = { version = "2.7", optional = true }
        · ───┬──
        ·    ╰── not used in code
     26 │ 
        ╰────

    Advice: 
      ☞ removing an optional dependency may be a breaking change

    Advice: 
      ☞ used in feature `unused-optional-weak`
        ╭─[Cargo.toml:9:25]
      8 │ unused-optional-feature = ["once_cell/std"]
      9 │ unused-optional-weak = ["memchr?/std"]
        ·                         ──────┬──────
        ·                               ╰── enabled here
     10 │ misplaced-optional = ["dep:smallvec"]
        ╰────

    shear/unused_optional_dependency

      ⚠ unused optional dependency `once_cell`
        ╭─[Cargo.toml:22:1]
     21 │ # Unused Optional Feature
     22 │ once_cell = { version = "1.0", optional = true }
        · ────┬────
        ·     ╰── not used in code
     23 │ 
        ╰────

    Advice: 
      ☞ removing an optional dependency may be a breaking change

    Advice: 
      ☞ used in feature `unused-optional-feature`
       ╭─[Cargo.toml:8:28]
     7 │ unused-optional = ["dep:itoa"]
     8 │ unused-optional-feature = ["once_cell/std"]
       ·                            ───────┬───────
       ·                                   ╰── enabled here
     9 │ unused-optional-weak = ["memchr?/std"]
       ╰────

    shear/misplaced_dependency

      × misplaced dependency `fastrand`
        ╭─[Cargo.toml:34:1]
     33 │ # Misplaced
     34 │ fastrand = "2.0"
        · ────┬───
        ·     ╰── only used in dev targets
     35 │ 
        ╰────
      help: move this dependency to `[dev-dependencies]`

    shear/misplaced_dependency

      × misplaced dependency `ryu_v1`
        ╭─[Cargo.toml:46:1]
     45 │ # Misplaced Renamed
     46 │ ryu_v1 = { package = "ryu", version = "1.0" }
        · ───┬──
        ·    ╰── only used in dev targets
     47 │ 
        ╰────
      help: move this dependency to `[dev-dependencies]`

    shear/misplaced_optional_dependency

      ⚠ misplaced optional dependency `ahash`
        ╭─[Cargo.toml:43:1]
     42 │ # Misplaced Optional Feature Weak
     43 │ ahash = { version = "0.8", optional = true }
        · ──┬──
        ·   ╰── only used in dev targets
     44 │ 
        ╰────
      help: remove the `optional` flag and move to `[dev-dependencies]`

    Advice: 
      ☞ removing an optional dependency may be a breaking change

    Advice: 
      ☞ used in feature `misplaced-optional-weak`
        ╭─[Cargo.toml:12:28]
     11 │ misplaced-optional-feature = ["hashbrown/serde"]
     12 │ misplaced-optional-weak = ["ahash?/std"]
        ·                            ──────┬─────
        ·                                  ╰── enabled here
     13 │ 
        ╰────

    shear/misplaced_optional_dependency

      ⚠ misplaced optional dependency `hashbrown`
        ╭─[Cargo.toml:40:1]
     39 │ # Misplaced Optional Feature
     40 │ hashbrown = { version = "0.15", optional = true }
        · ────┬────
        ·     ╰── only used in dev targets
     41 │ 
        ╰────
      help: remove the `optional` flag and move to `[dev-dependencies]`

    Advice: 
      ☞ removing an optional dependency may be a breaking change

    Advice: 
      ☞ used in feature `misplaced-optional-feature`
        ╭─[Cargo.toml:11:31]
     10 │ misplaced-optional = ["dep:smallvec"]
     11 │ misplaced-optional-feature = ["hashbrown/serde"]
        ·                               ────────┬────────
        ·                                       ╰── enabled here
     12 │ misplaced-optional-weak = ["ahash?/std"]
        ╰────

    shear/misplaced_optional_dependency

      ⚠ misplaced optional dependency `smallvec`
        ╭─[Cargo.toml:37:1]
     36 │ # Misplaced Optional
     37 │ smallvec = { version = "1.0", optional = true }
        · ────┬───
        ·     ╰── only used in dev targets
     38 │ 
        ╰────
      help: remove the `optional` flag and move to `[dev-dependencies]`

    Advice: 
      ☞ removing an optional dependency may be a breaking change

    Advice: 
      ☞ used in feature `misplaced-optional`
        ╭─[Cargo.toml:10:23]
      9 │ unused-optional-weak = ["memchr?/std"]
     10 │ misplaced-optional = ["dep:smallvec"]
        ·                       ───────┬──────
        ·                              ╰── enabled here
     11 │ misplaced-optional-feature = ["hashbrown/serde"]
        ╰────

    shear/unknown_ignore

      ⚠ unknown ignore `fake-crate`
        ╭─[Cargo.toml:68:36]
     67 │ [package.metadata.cargo-shear]
     68 │ ignored = ["regex-syntax", "slab", "fake-crate"]
        ·                                    ──────┬─────
        ·                                          ╰── not a dependency
        ╰────
      help: remove from ignored list

    shear/summary

      ✗ 8 errors
      ⚠ 7 warnings

    Advice:
      ☞ run with `--fix` to fix 8 issues
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Complex fixture should fix all fixable issues.
#[test]
fn complex_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("complex").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let windows = manifest.target.get("cfg(windows)");

    // Fixed
    assert!(!manifest.dependencies.contains_key("either"));
    assert!(!manifest.dependencies.contains_key("bitflags_v2"));
    assert!(!manifest.dependencies.contains_key("cfg-if"));
    assert!(!manifest.dev_dependencies.contains_key("serde"));
    assert!(!manifest.build_dependencies.contains_key("version_check"));
    assert!(!windows.is_some_and(|table| table.dependencies.contains_key("winapi")));
    assert!(manifest.dev_dependencies.contains_key("fastrand"));
    assert!(manifest.dev_dependencies.contains_key("ryu_v1"));

    // Can't Fix
    assert!(manifest.dependencies.contains_key("itoa"));
    assert!(manifest.dependencies.contains_key("once_cell"));
    assert!(manifest.dependencies.contains_key("memchr"));
    assert!(manifest.dependencies.contains_key("regex-syntax"));
    assert!(manifest.dependencies.contains_key("smallvec"));
    assert!(manifest.dependencies.contains_key("hashbrown"));
    assert!(manifest.dependencies.contains_key("ahash"));
    assert!(manifest.dependencies.contains_key("slab"));

    Ok(())
}

// When using --package, workspace analysis is skipped.
#[test]
fn filter_workspace_package_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("filter_workspace")
        .options(|options| options.with_packages(vec!["app".into()]))
        .run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_dependency

      × unused dependency `thiserror`
       ╭─[app/Cargo.toml:8:1]
     7 │ # Unused
     8 │ thiserror = "2.0"
       · ────┬────
       ·     ╰── not used in code
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// When using --package, only targeted package issues are fixed.
#[test]
fn filter_workspace_package_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) = CargoShearRunner::new("filter_workspace")
        .options(|options| options.with_fix().with_packages(vec!["app".into()]))
        .run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let app = Manifest::from_path(temp_dir.path().join("app/Cargo.toml"))?;
    assert!(!app.dependencies.contains_key("thiserror"));

    let lib = Manifest::from_path(temp_dir.path().join("lib/Cargo.toml"))?;
    assert!(lib.dependencies.contains_key("anyhow"));

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let workspace = &manifest.workspace.as_ref().unwrap().dependencies;
    assert!(workspace.contains_key("anyhow"));

    Ok(())
}

// When using --exclude, workspace analysis is skipped.
#[test]
fn filter_workspace_exclude_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("filter_workspace")
        .options(|options| options.with_excludes(vec!["lib".into()]))
        .run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_dependency

      × unused dependency `thiserror`
       ╭─[app/Cargo.toml:8:1]
     7 │ # Unused
     8 │ thiserror = "2.0"
       · ────┬────
       ·     ╰── not used in code
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// When using --exclude, only targeted package issues are fixed.
#[test]
fn filter_workspace_exclude_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) = CargoShearRunner::new("filter_workspace")
        .options(|options| options.with_fix().with_excludes(vec!["lib".into()]))
        .run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let app = Manifest::from_path(temp_dir.path().join("app/Cargo.toml"))?;
    assert!(!app.dependencies.contains_key("thiserror"));

    let lib = Manifest::from_path(temp_dir.path().join("lib/Cargo.toml"))?;
    assert!(lib.dependencies.contains_key("anyhow"));

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let workspace = &manifest.workspace.as_ref().unwrap().dependencies;
    assert!(workspace.contains_key("anyhow"));

    Ok(())
}

// `anyhow` is unused but suppressed via package ignore config.
#[test]
fn ignored() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("ignored").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    shear/summary

      ✓ no issues found
    ");

    Ok(())
}

// `anywho` is in the ignored list but doesn't exist as a dependency.
#[test]
fn ignored_invalid() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("ignored_invalid").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r#"
    shear/unknown_ignore

      ⚠ unknown ignore `anywho`
       ╭─[Cargo.toml:7:12]
     6 │ [package.metadata.cargo-shear]
     7 │ ignored = ["anywho"]
       ·            ────┬───
       ·                ╰── not a dependency
       ╰────
      help: remove from ignored list

    shear/summary

      ⚠ 1 warning
    "#);

    Ok(())
}

// `anyhow` is in the ignored list but is actually being used.
#[test]
fn ignored_redundant() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("ignored_redundant").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r#"
    shear/redundant_ignore

      ⚠ redundant ignore `anyhow`
        ╭─[Cargo.toml:10:12]
      9 │ [package.metadata.cargo-shear]
     10 │ ignored = ["anyhow"]
        ·            ────┬───
        ·                ╰── dependency is used
        ╰────
      help: remove from ignored list

    shear/summary

      ⚠ 1 warning
    "#);

    Ok(())
}

// `anyhow` is unused but suppressed via workspace ignore config.
#[test]
fn ignored_workspace() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("ignored_workspace").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    shear/summary

      ✓ no issues found
    ");

    Ok(())
}

// `anyhow` is in the workspace ignored list but is actually being used.
#[test]
fn ignored_workspace_redundant() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) =
        CargoShearRunner::new("ignored_workspace_redundant").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r#"
    shear/redundant_ignore

      ⚠ redundant ignore `anyhow`
        ╭─[Cargo.toml:10:12]
      9 │ [workspace.metadata.cargo-shear]
     10 │ ignored = ["anyhow"]
        ·            ────┬───
        ·                ╰── dependency is used
        ╰────
      help: remove from ignored list

    shear/summary

      ⚠ 1 warning
    "#);

    Ok(())
}

// Both `anyhow` (workspace ignore) and `thiserror` (package ignore) are unused, but ignored.
#[test]
fn ignored_workspace_merged() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("ignored_workspace_merged").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    shear/summary

      ✓ no issues found
    ");

    Ok(())
}

// `anyhow` is only used in tests but declared in `dependencies` instead of `dev-dependencies`.
#[test]
fn misplaced_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("misplaced").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/misplaced_dependency

      × misplaced dependency `anyhow`
       ╭─[Cargo.toml:8:1]
     7 │ # Misplaced
     8 │ anyhow = "1.0"
       · ───┬──
       ·    ╰── only used in dev targets
       ╰────
      help: move this dependency to `[dev-dependencies]`

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// `anyhow` should be moved from `dependencies` to `dev-dependencies`.
#[test]
fn misplaced_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("misplaced").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dev_dependencies.contains_key("anyhow"));
    assert!(!manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// Optional `anyhow` is only used in tests but declared in `dependencies`.
#[test]
fn misplaced_optional_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("misplaced_optional").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r#"
    shear/misplaced_optional_dependency

      ⚠ misplaced optional dependency `anyhow`
        ╭─[Cargo.toml:11:1]
     10 │ # Misplaced
     11 │ anyhow = { version = "1.0", optional = true }
        · ───┬──
        ·    ╰── only used in dev targets
        ╰────
      help: remove the `optional` flag and move to `[dev-dependencies]`

    Advice: 
      ☞ removing an optional dependency may be a breaking change

    Advice: 
      ☞ used in feature `testing`
       ╭─[Cargo.toml:7:12]
     6 │ [features]
     7 │ testing = ["dep:anyhow"]
       ·            ──────┬─────
       ·                  ╰── enabled here
     8 │ 
       ╰────

    shear/summary

      ⚠ 1 warning

    Advice:
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Optional `anyhow` can't be moved to `dev-dependencies` since they don't support `optional = true`.
#[test]
fn misplaced_optional_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("misplaced_optional").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dependencies.contains_key("anyhow"));
    assert!(!manifest.dev_dependencies.contains_key("anyhow"));

    Ok(())
}

// `anyhow` is only used in tests but declared in target specific `dependencies`.
#[test]
fn misplaced_platform_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("misplaced_platform").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/misplaced_dependency

      × misplaced dependency `anyhow`
       ╭─[Cargo.toml:8:1]
     7 │ # Misplaced
     8 │ anyhow = "1.0"
       · ───┬──
       ·    ╰── only used in dev targets
       ╰────
      help: move this dependency to `[target.'cfg(unix)'.dev-dependencies]`

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// `anyhow` should be moved from target specific `dependencies` to `dev-dependencies`.
#[test]
fn misplaced_platform_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("misplaced_platform").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let unix = manifest.target.get("cfg(unix)").expect("cfg(unix) target should exist");
    assert!(unix.dev_dependencies.contains_key("anyhow"));
    assert!(!unix.dependencies.contains_key("anyhow"));

    Ok(())
}

// Renamed dependency `anyhow_v1` is only used in tests but declared in `dependencies`.
#[test]
fn misplaced_renamed_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("misplaced_renamed").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/misplaced_dependency

      × misplaced dependency `anyhow_v1`
       ╭─[Cargo.toml:8:1]
     7 │ # Misplaced
     8 │ anyhow_v1 = { package = "anyhow", version = "1.0" }
       · ────┬────
       ·     ╰── only used in dev targets
       ╰────
      help: move this dependency to `[dev-dependencies]`

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Renamed `anyhow_v1` should be moved to `dev-dependencies` while maintaining package details.
#[test]
fn misplaced_renamed_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("misplaced_renamed").options(CargoShearOptions::with_fix).run()?;
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
fn misplaced_table_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("misplaced_table").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/misplaced_dependency

      × misplaced dependency `anyhow`
       ╭─[Cargo.toml:7:15]
     6 │ # Misplaced
     7 │ [dependencies.anyhow]
       ·               ───┬──
       ·                  ╰── only used in dev targets
     8 │ version = "1.0"
       ╰────
      help: move this dependency to `[dev-dependencies]`

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Table syntax `anyhow` should be moved to `dev-dependencies` while maintaining package details.
#[test]
fn misplaced_table_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("misplaced_table").options(CargoShearOptions::with_fix).run()?;
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

// `anyhow` is only used in unit tests but declared in `dependencies` instead of `dev-dependencies`.
#[test]
#[ignore = "Cannot detect misplaced dependencies in unit tests"]
fn misplaced_unit_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("misplaced_unit").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @"");

    Ok(())
}

// `anyhow` should be moved from `dependencies` to `dev-dependencies`.
#[test]
#[ignore = "Cannot detect misplaced dependencies in unit tests"]
fn misplaced_unit_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("misplaced_unit").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dev_dependencies.contains_key("anyhow"));
    assert!(!manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// Both `orphan.rs` and `ignored.rs` are unlinked, but `ignored.rs` is suppressed.
#[test]
fn unlinked_ignored_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unlinked_ignored").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r#"
    shear/unlinked_files

      ⚠ 1 unlinked file in `unlinked_ignored`
      │ src/orphan.rs
      help: delete this file

    shear/summary

      ⚠ 1 warning

    Advice:
      ☞ to suppress a file issue
       ╭─[Cargo.toml:2:18]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored-paths = ["tests/compile/*.rs"]
       ·                  ──────────┬─────────
       ·                            ╰── add a file pattern here
       ╰────
    "#);

    Ok(())
}

// `src/nonexistent.rs` pattern doesn't match any unlinked files.
#[test]
fn unlinked_ignored_redundant_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) =
        CargoShearRunner::new("unlinked_ignored_redundant").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r"
    shear/redundant_ignore_path

      ⚠ redundant ignored paths pattern `src/nonexistent.rs`
      help: remove from ignored paths list

    shear/summary

      ⚠ 1 warning
    ");

    Ok(())
}

// Both `app/orphan.rs` and `lib/ignored.rs` are unlinked, but `**/ignored.rs` is suppressed.
#[test]
fn unlinked_ignored_workspace_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) =
        CargoShearRunner::new("unlinked_ignored_workspace").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r#"
    shear/unlinked_files

      ⚠ 1 unlinked file in `app`
      │ src/orphan.rs
      help: delete this file

    shear/summary

      ⚠ 1 warning

    Advice:
      ☞ to suppress a file issue
       ╭─[Cargo.toml:2:18]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored-paths = ["tests/compile/*.rs"]
       ·                  ──────────┬─────────
       ·                            ╰── add a file pattern here
       ╰────
    "#);

    Ok(())
}

// Files that are empty (no items, only whitespace/comments) should be warned about.
#[test]
fn empty_files_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("empty_files").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r#"
    shear/empty_files

      ⚠ 3 empty files in `empty_files`
      │ src/comments.rs
      │ src/empty.rs
      │ src/whitespace.rs
      help: delete these files

    shear/summary

      ⚠ 1 warning

    Advice:
      ☞ to suppress a file issue
       ╭─[Cargo.toml:2:18]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored-paths = ["tests/compile/*.rs"]
       ·                  ──────────┬─────────
       ·                            ╰── add a file pattern here
       ╰────
    "#);

    Ok(())
}

// `anyhow` is unused.
#[test]
fn unused_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_dependency

      × unused dependency `anyhow`
       ╭─[Cargo.toml:8:1]
     7 │ # Unused
     8 │ anyhow = "1.0"
       · ───┬──
       ·    ╰── not used in code
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Unused `anyhow` should be removed from `dependencies`.
#[test]
fn unused_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("unused").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// `anyhow` is unused in build scripts.
#[test]
fn unused_build_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_build").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_dependency

      × unused dependency `anyhow`
       ╭─[Cargo.toml:8:1]
     7 │ # Unused
     8 │ anyhow = "1.0"
       · ───┬──
       ·    ╰── not used in code
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Unused `anyhow` should be removed from `build-dependencies`.
#[test]
fn unused_build_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("unused_build").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.build_dependencies.contains_key("anyhow"));

    Ok(())
}

// `anyhow` is unused in dev targets.
#[test]
fn unused_dev_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_dev").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_dependency

      × unused dependency `anyhow`
       ╭─[Cargo.toml:8:1]
     7 │ # Unused
     8 │ anyhow = "1.0"
       · ───┬──
       ·    ╰── not used in code
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Unused `anyhow` should be removed from `dev-dependencies`.
#[test]
fn unused_dev_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("unused_dev").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dev_dependencies.contains_key("anyhow"));

    Ok(())
}

// `anyhow` is unused in code but referenced in a feature, so it can't be safely removed.
#[test]
fn unused_feature_detect() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_feature").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r#"
    shear/unused_feature_dependency

      ⚠ dependency `anyhow` only used in features
        ╭─[Cargo.toml:11:1]
     10 │ # Unused
     11 │ anyhow = "1.0"
        · ───┬──
        ·    ╰── not used in code
        ╰────

    Advice: 
      ☞ used in feature `std`
       ╭─[Cargo.toml:7:8]
     6 │ [features]
     7 │ std = ["anyhow/std"]
       ·        ──────┬─────
       ·              ╰── enabled here
     8 │ 
       ╰────

    shear/summary

      ⚠ 1 warning

    Advice:
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// `anyhow` should remain since it's referenced in a feature, even though unused in code.
#[test]
fn unused_feature_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("unused_feature").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// `anyhow` is unused in code but referenced in a weak feature, so it can't be safely removed.
#[test]
fn unused_feature_weak_detect() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_feature_weak").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r#"
    shear/unused_optional_dependency

      ⚠ unused optional dependency `anyhow`
        ╭─[Cargo.toml:11:1]
     10 │ # Unused
     11 │ anyhow = { version = "1.0", optional = true }
        · ───┬──
        ·    ╰── not used in code
        ╰────

    Advice: 
      ☞ removing an optional dependency may be a breaking change

    Advice: 
      ☞ used in feature `std`
       ╭─[Cargo.toml:7:8]
     6 │ [features]
     7 │ std = ["anyhow?/std"]
       ·        ──────┬──────
       ·              ╰── enabled here
     8 │ 
       ╰────

    shear/summary

      ⚠ 1 warning

    Advice:
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// `anyhow` should remain since it's referenced in a weak feature, even though unused in code.
#[test]
fn unused_feature_weak_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("unused_feature_weak").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// `serde_json` (import `serde_json`) is not used in code.
#[test]
fn unused_naming_hyphen_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_naming_hyphen").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_dependency

      × unused dependency `serde_json`
       ╭─[Cargo.toml:8:1]
     7 │ # Unused
     8 │ serde_json = "1.0"
       · ─────┬────
       ·      ╰── not used in code
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// `serde_json` should be removed when fixing.
#[test]
fn unused_naming_hyphen_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("unused_naming_hyphen").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dependencies.contains_key("serde_json"));

    Ok(())
}

// `rustc-hash` (import `rustc_hash`) is not used in code.
#[test]
fn unused_naming_underscore_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_naming_underscore").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_dependency

      × unused dependency `rustc-hash`
       ╭─[Cargo.toml:8:1]
     7 │ # Unused
     8 │ rustc-hash = "2.0"
       · ─────┬────
       ·      ╰── not used in code
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// `rustc-hash` should be removed when fixing.
#[test]
fn unused_naming_underscore_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) = CargoShearRunner::new("unused_naming_underscore")
        .options(CargoShearOptions::with_fix)
        .run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dependencies.contains_key("rustc-hash"));

    Ok(())
}

// Optional `anyhow` enabled via `dep:anyhow` is unused but can't be removed without breaking the feature.
#[test]
fn unused_optional_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_optional").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r#"
    shear/unused_optional_dependency

      ⚠ unused optional dependency `anyhow`
        ╭─[Cargo.toml:11:1]
     10 │ # Unused
     11 │ anyhow = { version = "1.0", optional = true }
        · ───┬──
        ·    ╰── not used in code
        ╰────

    Advice: 
      ☞ removing an optional dependency may be a breaking change

    Advice: 
      ☞ used in feature `anyhow`
       ╭─[Cargo.toml:7:11]
     6 │ [features]
     7 │ anyhow = ["dep:anyhow"]
       ·           ──────┬─────
       ·                 ╰── enabled here
     8 │ 
       ╰────

    shear/summary

      ⚠ 1 warning

    Advice:
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Unused optional `anyhow` enabled via `dep:anyhow` can't be removed since that would break the feature.
#[test]
fn unused_optional_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("unused_optional").options(CargoShearOptions::with_fix).run()?;
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
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_optional_implicit").run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    insta::assert_snapshot!(output, @r#"
    shear/unused_optional_dependency

      ⚠ unused optional dependency `anyhow`
       ╭─[Cargo.toml:8:1]
     7 │ # Unused
     8 │ anyhow = { version = "1.0", optional = true }
       · ───┬──
       ·    ╰── not used in code
       ╰────

    Advice: 
      ☞ removing an optional dependency may be a breaking change

    shear/summary

      ⚠ 1 warning

    Advice:
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Unused optional `anyhow` with implicit feature can't be removed since that would remove the feature.
#[test]
fn unused_optional_implicit_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) = CargoShearRunner::new("unused_optional_implicit")
        .options(CargoShearOptions::with_fix)
        .run()?;
    assert_eq!(exit_code, ExitCode::SUCCESS);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// Target specific `anyhow` is unused.
#[test]
fn unused_platform_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_platform").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_dependency

      × unused dependency `anyhow`
       ╭─[Cargo.toml:8:1]
     7 │ # Unused
     8 │ anyhow = "1.0"
       · ───┬──
       ·    ╰── not used in code
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Unused target specific `anyhow` should be removed.
#[test]
fn unused_platform_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("unused_platform").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let windows = manifest.target.get("cfg(windows)");
    assert!(!windows.is_some_and(|table| table.dependencies.contains_key("anyhow")));

    Ok(())
}

// Renamed `anyhow_v1` is unused.
#[test]
fn unused_renamed_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_renamed").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_dependency

      × unused dependency `anyhow_v1`
       ╭─[Cargo.toml:8:1]
     7 │ # Unused
     8 │ anyhow_v1 = { package = "anyhow", version = "1.0" }
       · ────┬────
       ·     ╰── not used in code
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Unused renamed `anyhow_v1` should be removed.
#[test]
fn unused_renamed_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("unused_renamed").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dependencies.contains_key("anyhow_v1"));

    Ok(())
}

// Dependency `criterion2` (lib.name = "criterion") is unused.
#[test]
fn unused_libname_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_libname").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_dependency

      × unused dependency `criterion2`
       ╭─[Cargo.toml:8:1]
     7 │ # Unused
     8 │ criterion2 = { version = "3.0", default-features = false }
       · ─────┬────
       ·      ╰── not used in code
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Unused dependency `criterion2` (lib.name = "criterion") should be removed.
#[test]
fn unused_libname_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("unused_libname").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dependencies.contains_key("criterion2"));

    Ok(())
}

// Table syntax `anyhow` is unused.
#[test]
fn unused_table_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_table").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_dependency

      × unused dependency `anyhow`
       ╭─[Cargo.toml:7:15]
     6 │ # Unused
     7 │ [dependencies.anyhow]
       ·               ───┬──
       ·                  ╰── not used in code
     8 │ version = "1.0"
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Unused table syntax `anyhow` should be removed.
#[test]
fn unused_table_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("unused_table").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    assert!(!manifest.dependencies.contains_key("anyhow"));

    Ok(())
}

// Workspace dependency `anyhow` is not inherited by any workspace member.
#[test]
fn unused_workspace_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_workspace").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_workspace_dependency

      × unused workspace dependency `anyhow`
       ╭─[Cargo.toml:7:1]
     6 │ # Unused
     7 │ anyhow = "1.0"
       · ───┬──
       ·    ╰── not used by any workspace member
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Unused workspace dependency `anyhow` should be removed.
#[test]
fn unused_workspace_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) =
        CargoShearRunner::new("unused_workspace").options(CargoShearOptions::with_fix).run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let workspace = &manifest.workspace.as_ref().unwrap().dependencies;
    assert!(!workspace.contains_key("anyhow"));

    Ok(())
}

// Renamed workspace dependency `anyhow_v1` is unused.
#[test]
fn unused_workspace_renamed_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_workspace_renamed").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_workspace_dependency

      × unused workspace dependency `anyhow_v1`
       ╭─[Cargo.toml:7:1]
     6 │ # Unused
     7 │ anyhow_v1 = { package = "anyhow", version = "1.0" }
       · ────┬────
       ·     ╰── not used by any workspace member
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Unused renamed workspace dependency `anyhow_v1` should be removed.
#[test]
fn unused_workspace_renamed_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) = CargoShearRunner::new("unused_workspace_renamed")
        .options(CargoShearOptions::with_fix)
        .run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let workspace = &manifest.workspace.as_ref().unwrap().dependencies;
    assert!(!workspace.contains_key("anyhow_v1"));

    Ok(())
}

// Workspace dependency `criterion2` (lib.name = "criterion") is unused.
#[test]
fn unused_workspace_libname_detection() -> Result<(), Box<dyn Error>> {
    let (exit_code, output, _temp_dir) = CargoShearRunner::new("unused_workspace_libname").run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    insta::assert_snapshot!(output, @r#"
    shear/unused_workspace_dependency

      × unused workspace dependency `criterion2`
       ╭─[Cargo.toml:7:1]
     6 │ # Unused
     7 │ criterion2 = { version = "3.0", default-features = false }
       · ─────┬────
       ·      ╰── not used by any workspace member
       ╰────
      help: remove this dependency

    shear/summary

      ✗ 1 error

    Advice:
      ☞ run with `--fix` to fix 1 issue
      ☞ to suppress a dependency issue
       ╭─[Cargo.toml:2:12]
     1 │ [package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]
     2 │ ignored = ["crate-name"]
       ·            ──────┬─────
       ·                  ╰── add a crate name here
       ╰────
    "#);

    Ok(())
}

// Unused workspace dependency `criterion2` (lib.name = "criterion") should be removed.
#[test]
fn unused_workspace_libname_fix() -> Result<(), Box<dyn Error>> {
    let (exit_code, _output, temp_dir) = CargoShearRunner::new("unused_workspace_libname")
        .options(CargoShearOptions::with_fix)
        .run()?;
    assert_eq!(exit_code, ExitCode::FAILURE);

    let manifest = Manifest::from_path(temp_dir.path().join("Cargo.toml"))?;
    let workspace = &manifest.workspace.as_ref().unwrap().dependencies;
    assert!(!workspace.contains_key("criterion2"));

    Ok(())
}
