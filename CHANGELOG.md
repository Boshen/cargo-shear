# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.7.2](https://github.com/Boshen/cargo-shear/compare/v1.7.1...v1.7.2) - 2025-12-09

### <!-- 4 -->⚡ Performance
- Use simdutf8 for faster file reading ([#359](https://github.com/Boshen/cargo-shear/pull/359)) (by @Boshen)

### <!-- 9 -->💼 Other
- Show cargo build output when expanding ([#358](https://github.com/Boshen/cargo-shear/pull/358)) (by @CathalMullan)
- Show cargo metadata output to users ([#356](https://github.com/Boshen/cargo-shear/pull/356)) (by @Boshen)
- Fix feature ref diagnostic to include plain dep enablements ([#355](https://github.com/Boshen/cargo-shear/pull/355)) (by @CathalMullan)
- Rewrite source parser to use `pulldown-cmark` for comment parsing ([#353](https://github.com/Boshen/cargo-shear/pull/353)) (by @CathalMullan)
- Add OpenAI Codex to trophy case (by @Boshen)
- Collect imports inside macros ([#346](https://github.com/Boshen/cargo-shear/pull/346)) (by @CathalMullan)

### Contributors

* @Boshen
* @CathalMullan
* @renovate[bot]

## [1.7.1](https://github.com/Boshen/cargo-shear/compare/v1.7.0...v1.7.1) - 2025-12-03

### <!-- 9 -->💼 Other
- Skip workspace analysis when package/exclude is specified ([#343](https://github.com/Boshen/cargo-shear/pull/343)) (by @CathalMullan)
- Switch to `miette` for output, add new warnings ([#342](https://github.com/Boshen/cargo-shear/pull/342)) (by @CathalMullan)

### Contributors

* @CathalMullan
* @renovate[bot]
* @Boshen

## [1.7.0](https://github.com/Boshen/cargo-shear/compare/v1.6.6...v1.7.0) - 2025-11-30

### Added

- improve redundant ignore warning messages ([#333](https://github.com/Boshen/cargo-shear/pull/333))

### Other

- Replace `cargo_toml` with custom spanned structs  ([#338](https://github.com/Boshen/cargo-shear/pull/338))
- Track feature references for dependencies, split out optionals  ([#335](https://github.com/Boshen/cargo-shear/pull/335))
- Improve tracking of dependencies, and precision of TOML updates ([#334](https://github.com/Boshen/cargo-shear/pull/334))
- collect import syntax tokens not string ([#328](https://github.com/Boshen/cargo-shear/pull/328))
- Process packages in parallel using `rayon` ([#329](https://github.com/Boshen/cargo-shear/pull/329))
- *(deps)* update crate-ci/typos action to v1.40.0 ([#330](https://github.com/Boshen/cargo-shear/pull/330))

## [1.6.6](https://github.com/Boshen/cargo-shear/compare/v1.6.5...v1.6.6) - 2025-11-26

### Fixed

- fix incorrect `--version` output ([#326](https://github.com/Boshen/cargo-shear/pull/326))

### Other

- Fix detection of redundant workspace ignores ([#324](https://github.com/Boshen/cargo-shear/pull/324))
- Switch from `syn` to `ra_ap_syntax` for parsing ([#322](https://github.com/Boshen/cargo-shear/pull/322))

## [1.6.5](https://github.com/Boshen/cargo-shear/compare/v1.6.4...v1.6.5) - 2025-11-24

### Other

- Improve error reporting for misplaced deps, and mention fix flag if not used ([#318](https://github.com/Boshen/cargo-shear/pull/318))
- *(deps)* update rust crate syn to v2.0.111 ([#320](https://github.com/Boshen/cargo-shear/pull/320))
- *(deps)* update github-actions ([#319](https://github.com/Boshen/cargo-shear/pull/319))
- Detect and fix misplaced dev dependencies ([#316](https://github.com/Boshen/cargo-shear/pull/316))

## [1.6.4](https://github.com/Boshen/cargo-shear/compare/v1.6.3...v1.6.4) - 2025-11-22

### Other

- Refactor dependency analysis to properly handle code imports, dependency keys, and package names ([#315](https://github.com/Boshen/cargo-shear/pull/315))
- Refactor tests, add snapshot testing, improve edge case coverage ([#314](https://github.com/Boshen/cargo-shear/pull/314))
- *(deps)* update rust crates ([#312](https://github.com/Boshen/cargo-shear/pull/312))
- *(deps)* update github-actions ([#311](https://github.com/Boshen/cargo-shear/pull/311))

## [1.6.3](https://github.com/Boshen/cargo-shear/compare/v1.6.2...v1.6.3) - 2025-11-14

### Other

- Add locked/offline/frozen flags like cargo ([#310](https://github.com/Boshen/cargo-shear/pull/310))
- *(deps)* update crate-ci/typos action to v1.39.2 ([#309](https://github.com/Boshen/cargo-shear/pull/309))
- *(deps)* update crate-ci/typos action to v1.39.1 ([#308](https://github.com/Boshen/cargo-shear/pull/308))
- *(deps)* update dependency rust to v1.91.1 ([#306](https://github.com/Boshen/cargo-shear/pull/306))
- *(deps)* lock file maintenance ([#304](https://github.com/Boshen/cargo-shear/pull/304))
- *(deps)* update rust crate syn to v2.0.109 ([#302](https://github.com/Boshen/cargo-shear/pull/302))
- *(deps)* update taiki-e/install-action action to v2.62.49 ([#303](https://github.com/Boshen/cargo-shear/pull/303))

## [1.6.2](https://github.com/Boshen/cargo-shear/compare/v1.6.1...v1.6.2) - 2025-11-04

### Fixed

- preserve attributes in doc blocks normalizer ([#301](https://github.com/Boshen/cargo-shear/pull/301))

### Other

- *(deps)* lock file maintenance rust crates ([#300](https://github.com/Boshen/cargo-shear/pull/300))
- *(deps)* update github-actions ([#299](https://github.com/Boshen/cargo-shear/pull/299))
- *(deps)* update crate-ci/typos action to v1.39.0 ([#298](https://github.com/Boshen/cargo-shear/pull/298))
- *(deps)* update dependency rust to v1.91.0 ([#296](https://github.com/Boshen/cargo-shear/pull/296))

## [1.6.1](https://github.com/Boshen/cargo-shear/compare/v1.6.0...v1.6.1) - 2025-10-27

### Fixed

- treat ignored packages as used on the workspace level ([#295](https://github.com/Boshen/cargo-shear/pull/295))

### Other

- *(deps)* lock file maintenance rust crates ([#294](https://github.com/Boshen/cargo-shear/pull/294))
- *(deps)* update github-actions ([#293](https://github.com/Boshen/cargo-shear/pull/293))
- *(deps)* lock file maintenance rust crates ([#291](https://github.com/Boshen/cargo-shear/pull/291))
- *(deps)* update github-actions ([#290](https://github.com/Boshen/cargo-shear/pull/290))

## [1.6.0](https://github.com/Boshen/cargo-shear/compare/v1.5.2...v1.6.0) - 2025-10-15

### Added

- collect imports from documentation code blocks ([#289](https://github.com/Boshen/cargo-shear/pull/289))
- warn about redundant ignored dependencies in metadata ([#284](https://github.com/Boshen/cargo-shear/pull/284))

### Other

- *(deps)* update github/codeql-action action to v4 ([#287](https://github.com/Boshen/cargo-shear/pull/287))
- *(deps)* lock file maintenance rust crates ([#288](https://github.com/Boshen/cargo-shear/pull/288))
- *(deps)* update github-actions ([#286](https://github.com/Boshen/cargo-shear/pull/286))
- *(deps)* update crate-ci/typos action to v1.38.1 ([#285](https://github.com/Boshen/cargo-shear/pull/285))
- *(deps)* update crate-ci/typos action to v1.38.0 ([#283](https://github.com/Boshen/cargo-shear/pull/283))
- *(deps)* lock file maintenance rust crates ([#282](https://github.com/Boshen/cargo-shear/pull/282))
- *(deps)* update github-actions ([#281](https://github.com/Boshen/cargo-shear/pull/281))
- *(deps)* update crate-ci/typos action to v1.37.2 ([#280](https://github.com/Boshen/cargo-shear/pull/280))
- *(deps)* update crate-ci/typos action to v1.37.1 ([#279](https://github.com/Boshen/cargo-shear/pull/279))
- *(deps)* update crate-ci/typos action to v1.37.0 ([#278](https://github.com/Boshen/cargo-shear/pull/278))
- *(deps)* update github-actions ([#277](https://github.com/Boshen/cargo-shear/pull/277))
- *(deps)* update crate-ci/typos action to v1.36.3 ([#275](https://github.com/Boshen/cargo-shear/pull/275))

## [1.5.2](https://github.com/Boshen/cargo-shear/compare/v1.5.1...v1.5.2) - 2025-09-22

### Other

- optimize collect_tokens by replacing regex with string operations and iterators ([#274](https://github.com/Boshen/cargo-shear/pull/274))
- use rustc-hash for better performance ([#273](https://github.com/Boshen/cargo-shear/pull/273))
- add CLAUDE.md
- modularize codebase for better extensibility ([#271](https://github.com/Boshen/cargo-shear/pull/271))
- *(deps)* lock file maintenance rust crates ([#270](https://github.com/Boshen/cargo-shear/pull/270))
- *(deps)* update taiki-e/install-action action to v2.62.0 ([#269](https://github.com/Boshen/cargo-shear/pull/269))
- *(deps)* update dependency rust to v1.90.0 ([#268](https://github.com/Boshen/cargo-shear/pull/268))
- *(deps)* update github-actions ([#267](https://github.com/Boshen/cargo-shear/pull/267))
- renovate ignore tests
- *(deps)* lock file maintenance ([#266](https://github.com/Boshen/cargo-shear/pull/266))
- *(deps)* lock file maintenance rust crates ([#265](https://github.com/Boshen/cargo-shear/pull/265))
- *(deps)* lock file maintenance rust crates ([#264](https://github.com/Boshen/cargo-shear/pull/264))
- *(deps)* update github-actions ([#263](https://github.com/Boshen/cargo-shear/pull/263))
- *(deps)* lock file maintenance rust crates ([#262](https://github.com/Boshen/cargo-shear/pull/262))
- *(deps)* update github-actions ([#261](https://github.com/Boshen/cargo-shear/pull/261))
- *(deps)* update crate-ci/typos action to v1.36.2 ([#260](https://github.com/Boshen/cargo-shear/pull/260))
- *(deps)* update crate-ci/typos action to v1.36.1 ([#259](https://github.com/Boshen/cargo-shear/pull/259))
- *(deps)* update crate-ci/typos action to v1.36.0 ([#258](https://github.com/Boshen/cargo-shear/pull/258))
- *(deps)* update crate-ci/typos action to v1.35.8 ([#257](https://github.com/Boshen/cargo-shear/pull/257))
- *(deps)* lock file maintenance rust crates ([#256](https://github.com/Boshen/cargo-shear/pull/256))
- *(deps)* update github-actions ([#255](https://github.com/Boshen/cargo-shear/pull/255))
- *(deps)* update crate-ci/typos action to v1.35.7 ([#254](https://github.com/Boshen/cargo-shear/pull/254))
- Add integration tests with real Rust workspace fixtures ([#248](https://github.com/Boshen/cargo-shear/pull/248))
- *(deps)* lock file maintenance rust crates ([#252](https://github.com/Boshen/cargo-shear/pull/252))
- *(deps)* update github-actions ([#251](https://github.com/Boshen/cargo-shear/pull/251))

## [1.5.1](https://github.com/Boshen/cargo-shear/compare/v1.5.0...v1.5.1) - 2025-08-21

### Fixed

- imports with r# ([#250](https://github.com/Boshen/cargo-shear/pull/250))

### Other

- Add comprehensive test suite with 84 tests covering all aspects of cargo-shear ([#247](https://github.com/Boshen/cargo-shear/pull/247))
- *(deps)* update crate-ci/typos action to v1.35.5 ([#246](https://github.com/Boshen/cargo-shear/pull/246))
- *(deps)* lock file maintenance rust crates ([#245](https://github.com/Boshen/cargo-shear/pull/245))
- *(deps)* update github-actions ([#244](https://github.com/Boshen/cargo-shear/pull/244))
- *(deps)* update crate-ci/typos action to v1.35.4 ([#241](https://github.com/Boshen/cargo-shear/pull/241))

## [1.5.0](https://github.com/Boshen/cargo-shear/compare/v1.4.1...v1.5.0) - 2025-08-11

### Added

- improve logging for file read and parse ([#240](https://github.com/Boshen/cargo-shear/pull/240))

### Other

- *(deps)* lock file maintenance rust crates ([#239](https://github.com/Boshen/cargo-shear/pull/239))
- *(deps)* lock file maintenance rust crates ([#238](https://github.com/Boshen/cargo-shear/pull/238))
- *(deps)* update github-actions ([#237](https://github.com/Boshen/cargo-shear/pull/237))
- *(deps)* update crate-ci/typos action to v1.35.3 ([#235](https://github.com/Boshen/cargo-shear/pull/235))
- *(deps)* update crate-ci/typos action to v1.35.2 ([#234](https://github.com/Boshen/cargo-shear/pull/234))
- *(deps)* update dependency rust to v1.89.0 ([#233](https://github.com/Boshen/cargo-shear/pull/233))
- *(deps)* update crate-ci/typos action to v1.35.1 ([#232](https://github.com/Boshen/cargo-shear/pull/232))
- *(deps)* lock file maintenance rust crates ([#230](https://github.com/Boshen/cargo-shear/pull/230))
- *(deps)* update github-actions ([#229](https://github.com/Boshen/cargo-shear/pull/229))

## [1.4.1](https://github.com/Boshen/cargo-shear/compare/v1.4.0...v1.4.1) - 2025-07-28

### Other

- *(release)* enable trusted publishing
- *(deps)* lock file maintenance rust crates ([#226](https://github.com/Boshen/cargo-shear/pull/226))
- *(deps)* update github-actions ([#225](https://github.com/Boshen/cargo-shear/pull/225))
- *(deps)* lock file maintenance rust crates ([#223](https://github.com/Boshen/cargo-shear/pull/223))
- *(deps)* update github-actions ([#222](https://github.com/Boshen/cargo-shear/pull/222))
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.4.0](https://github.com/Boshen/cargo-shear/compare/v1.3.3...v1.4.0) - 2025-07-14

### Added

- print backtrace when resulting errors ([#215](https://github.com/Boshen/cargo-shear/pull/215))

### Other

- *(deps)* lock file maintenance rust crates ([#221](https://github.com/Boshen/cargo-shear/pull/221))
- *(deps)* update taiki-e/install-action action to v2.56.13 ([#220](https://github.com/Boshen/cargo-shear/pull/220))
- *(deps)* lock file maintenance ([#218](https://github.com/Boshen/cargo-shear/pull/218))
- *(deps)* update github-actions ([#217](https://github.com/Boshen/cargo-shear/pull/217))

## [1.3.3](https://github.com/Boshen/cargo-shear/compare/v1.3.2...v1.3.3) - 2025-07-04

### Fixed

- add file type check ([#214](https://github.com/Boshen/cargo-shear/pull/214))

### Other

- *(deps)* update crate-ci/typos action to v1.34.0 ([#212](https://github.com/Boshen/cargo-shear/pull/212))

## [1.3.2](https://github.com/Boshen/cargo-shear/compare/v1.3.1...v1.3.2) - 2025-06-29

### Other

- *(deps)* lock file maintenance rust crates ([#210](https://github.com/Boshen/cargo-shear/pull/210))
- *(deps)* update github-actions ([#209](https://github.com/Boshen/cargo-shear/pull/209))
- *(deps)* update dependency rust to v1.88.0 ([#208](https://github.com/Boshen/cargo-shear/pull/208))
- *(deps)* update taiki-e/install-action action to v2.54.0 ([#207](https://github.com/Boshen/cargo-shear/pull/207))
- *(deps)* lock file maintenance rust crates ([#205](https://github.com/Boshen/cargo-shear/pull/205))
- *(deps)* update github-actions ([#204](https://github.com/Boshen/cargo-shear/pull/204))
- *(deps)* lock file maintenance rust crates ([#203](https://github.com/Boshen/cargo-shear/pull/203))
- *(deps)* update github-actions ([#202](https://github.com/Boshen/cargo-shear/pull/202))
- *(deps)* lock file maintenance rust crates ([#200](https://github.com/Boshen/cargo-shear/pull/200))
- *(deps)* update github-actions ([#199](https://github.com/Boshen/cargo-shear/pull/199))
- *(README)* add turbopack to trophy case
- *(deps)* update crate-ci/typos action to v1.33.1 ([#198](https://github.com/Boshen/cargo-shear/pull/198))
- *(deps)* update crate-ci/typos action to v1.33.0 ([#196](https://github.com/Boshen/cargo-shear/pull/196))

## [1.3.1](https://github.com/Boshen/cargo-shear/compare/v1.3.0...v1.3.1) - 2025-06-02

### Other

- *(deps)* lock file maintenance rust crates ([#195](https://github.com/Boshen/cargo-shear/pull/195))
- *(deps)* update taiki-e/install-action action to v2.52.4 ([#194](https://github.com/Boshen/cargo-shear/pull/194))
- *(deps)* lock file maintenance rust crates ([#193](https://github.com/Boshen/cargo-shear/pull/193))
- *(deps)* update taiki-e/install-action action to v2.52.1 ([#192](https://github.com/Boshen/cargo-shear/pull/192))
- add fix example

## [1.3.0](https://github.com/Boshen/cargo-shear/compare/v1.2.8...v1.3.0) - 2025-05-23

### Added

- return exit code when `--fix` ([#189](https://github.com/Boshen/cargo-shear/pull/189))

## [1.2.8](https://github.com/Boshen/cargo-shear/compare/v1.2.7...v1.2.8) - 2025-05-21

### Other

- improve ignore hint ([#188](https://github.com/Boshen/cargo-shear/pull/188))
- *(deps)* update github-actions ([#185](https://github.com/Boshen/cargo-shear/pull/185))
- *(deps)* lock file maintenance rust crates ([#186](https://github.com/Boshen/cargo-shear/pull/186))
- *(deps)* update dependency rust to v1.87.0 ([#183](https://github.com/Boshen/cargo-shear/pull/183))
- *(deps)* update github-actions ([#180](https://github.com/Boshen/cargo-shear/pull/180))

## [1.2.7](https://github.com/Boshen/cargo-shear/compare/v1.2.6...v1.2.7) - 2025-05-08

### Fixed

- handle whitespace in derive macro path `thiserror :: Error` ([#177](https://github.com/Boshen/cargo-shear/pull/177))

### Other

- *(deps)* lock file maintenance rust crates ([#179](https://github.com/Boshen/cargo-shear/pull/179))
- *(deps)* update crate-ci/typos action to v1.32.0 ([#176](https://github.com/Boshen/cargo-shear/pull/176))
- *(deps)* update crate-ci/typos action to v1.31.2 ([#174](https://github.com/Boshen/cargo-shear/pull/174))
- *(deps)* update github-actions ([#173](https://github.com/Boshen/cargo-shear/pull/173))

## [1.2.6](https://github.com/Boshen/cargo-shear/compare/v1.2.5...v1.2.6) - 2025-04-25

### Other

- use `ubuntu-22.04` instead of `ubuntu-latest` to build
- Reapply "perf: Use mimalloc on pre-built binaries. " ([#171](https://github.com/Boshen/cargo-shear/pull/171))

## [1.2.5](https://github.com/Boshen/cargo-shear/compare/v1.2.4...v1.2.5) - 2025-04-25

### Other

- Revert "perf: Use mimalloc on pre-built binaries. " ([#171](https://github.com/Boshen/cargo-shear/pull/171))
- update README
- *(README.md)* document how to install cargo-shear using homebrew ([#168](https://github.com/Boshen/cargo-shear/pull/168))
- *(deps)* update github-actions ([#166](https://github.com/Boshen/cargo-shear/pull/166))

## [1.2.4](https://github.com/Boshen/cargo-shear/compare/v1.2.3...v1.2.4) - 2025-04-20

### Other

- fix windows build

## [1.2.3](https://github.com/Boshen/cargo-shear/compare/v1.2.2...v1.2.3) - 2025-04-20

### Other

- pin crate-ci/typos

## [1.2.2](https://github.com/Boshen/cargo-shear/compare/v1.2.1...v1.2.2) - 2025-04-20

### Other

- use latest containers to build binaries

## [1.2.1](https://github.com/Boshen/cargo-shear/compare/v1.2.0...v1.2.1) - 2025-04-20

### Other

- Use mimalloc on pre-built binaries. 1.2x times faster! ([#162](https://github.com/Boshen/cargo-shear/pull/162))
- Add `--expand` to expand macros to find any hidden dependencies ([#159](https://github.com/Boshen/cargo-shear/pull/159))

## [1.2.0](https://github.com/Boshen/cargo-shear/compare/v1.1.14...v1.2.0) - 2025-04-18

### Added

- `--fix` remove unused dependencies from [features] section ([#157](https://github.com/Boshen/cargo-shear/pull/157))

## [1.1.14](https://github.com/Boshen/cargo-shear/compare/v1.1.13...v1.1.14) - 2025-04-16

### Other

- build on ubuntu-latest. ubuntu-20.04 is retired ([#153](https://github.com/Boshen/cargo-shear/pull/153))

## [1.1.13](https://github.com/Boshen/cargo-shear/compare/v1.1.12...v1.1.13) - 2025-04-14

### Other

- *(deps)* update github-actions ([#151](https://github.com/Boshen/cargo-shear/pull/151))
- *(deps)* update github-actions ([#148](https://github.com/Boshen/cargo-shear/pull/148))
- *(deps)* lock file maintenance rust crates ([#149](https://github.com/Boshen/cargo-shear/pull/149))

## [1.1.12](https://github.com/Boshen/cargo-shear/compare/v1.1.11...v1.1.12) - 2025-04-04

### Other

- use regex-lite
- `cargo upgrade -i`
- *(deps)* update dependency rust to v1.86.0 ([#146](https://github.com/Boshen/cargo-shear/pull/146))
- *(deps)* update github-actions ([#144](https://github.com/Boshen/cargo-shear/pull/144))
- *(deps)* lock file maintenance ([#145](https://github.com/Boshen/cargo-shear/pull/145))
- *(deps)* update github-actions ([#142](https://github.com/Boshen/cargo-shear/pull/142))
- *(deps)* update dependency rust to v1.85.1 ([#140](https://github.com/Boshen/cargo-shear/pull/140))

## [1.1.11](https://github.com/Boshen/cargo-shear/compare/v1.1.10...v1.1.11) - 2025-03-18

### Other

- Add support for serde macro attributes ([#134](https://github.com/Boshen/cargo-shear/pull/134))
- *(deps)* update github-actions ([#137](https://github.com/Boshen/cargo-shear/pull/137))
- trigger CI when workflow file changes
- *(deps)* lock file maintenance rust crates ([#138](https://github.com/Boshen/cargo-shear/pull/138))
- *(deps)* update github-actions ([#135](https://github.com/Boshen/cargo-shear/pull/135))
- *(deps)* lock file maintenance rust crates ([#136](https://github.com/Boshen/cargo-shear/pull/136))
- *(deps)* update github-actions ([#131](https://github.com/Boshen/cargo-shear/pull/131))
- *(deps)* lock file maintenance rust crates ([#132](https://github.com/Boshen/cargo-shear/pull/132))
- improve tests & add more lints ([#130](https://github.com/Boshen/cargo-shear/pull/130))
- *(deps)* update github-actions ([#127](https://github.com/Boshen/cargo-shear/pull/127))
- update trigger

## [1.1.10](https://github.com/Boshen/cargo-shear/compare/v1.1.9...v1.1.10) - 2025-02-21

### Other

- Rust 2024
- *(deps)* update dependency rust to v1.85.0 (#126)
- *(deps)* update taiki-e/install-action action to v2.48.15 (#125)
- *(deps)* update taiki-e/install-action action to v2.48.14 (#124)
- *(deps)* update github-actions (#123)
- use oxc-project/setup-rust
- pinGitHubActionDigestsToSemver

## [1.1.9](https://github.com/Boshen/cargo-shear/compare/v1.1.8...v1.1.9) - 2025-02-11

### Other

- persist-credentials: true
- change token
- persist-credentials: false
- *(deps)* pin dependencies (#120)
- add components
- fix overly broad permissions
- update
- add zizmor
- update justfile
- update renovate.json
- *(deps)* lock file maintenance rust crates (#118)
- Update README.md
- *(deps)* update rust crates (#116)
- *(deps)* update dependency rust to v1.84.1 (#115)
- *(deps)* update rust crate bpaf to 0.9.16 (#114)
- *(deps)* update rust crate serde_json to 1.0.137 (#113)
- *(deps)* update rust crate serde_json to 1.0.136 (#112)
- *(deps)* update rust crates (#110)

## [1.1.8](https://github.com/Boshen/cargo-shear/compare/v1.1.7...v1.1.8) - 2025-01-10

### Other

- macos-12 (deprecated) -> macos-13

## [1.1.7](https://github.com/Boshen/cargo-shear/compare/v1.1.6...v1.1.7) - 2025-01-10

### Other

- update rustfmt

## [1.1.6](https://github.com/Boshen/cargo-shear/compare/v1.1.5...v1.1.6) - 2025-01-10

### Other

- chore: remove `resolver = "3"` from `Cargo.toml`

## [1.1.5](https://github.com/Boshen/cargo-shear/compare/v1.1.4...v1.1.5) - 2025-01-10

### Other

- `cargo update` - support `resolver = "3"`
- *(deps)* update dependency rust to v1.84.0 (#107)
- *(deps)* update rust crate syn to 2.0.95 (#106)
- *(deps)* update rust crate syn to 2.0.93 (#105)
- *(deps)* update rust crates (#104)
- *(deps)* update rust crates (#103)
- *(deps)* update rust crates (#102)
- *(deps)* update dependency rust to v1.83.0 (#100)

## [1.1.4](https://github.com/Boshen/cargo-shear/compare/v1.1.3...v1.1.4) - 2024-11-25

### Other

- *(deps)* update rust crates ([#99](https://github.com/Boshen/cargo-shear/pull/99))
- *(deps)* update rust crate serde_json to 1.0.133 ([#98](https://github.com/Boshen/cargo-shear/pull/98))
- *(deps)* update rust crate anyhow to 1.0.93 ([#97](https://github.com/Boshen/cargo-shear/pull/97))
- *(deps)* update rust crates ([#96](https://github.com/Boshen/cargo-shear/pull/96))
- *(deps)* update rust crates ([#94](https://github.com/Boshen/cargo-shear/pull/94))
- *(deps)* update rust crates ([#92](https://github.com/Boshen/cargo-shear/pull/92))
- *(deps)* update dependency rust to v1.82.0 ([#91](https://github.com/Boshen/cargo-shear/pull/91))
- *(deps)* update rust crates ([#90](https://github.com/Boshen/cargo-shear/pull/90))
- *(deps)* update rust crates ([#88](https://github.com/Boshen/cargo-shear/pull/88))

## [1.1.3](https://github.com/Boshen/cargo-shear/compare/v1.1.2...v1.1.3) - 2024-09-23

### Fixed

- search for tokens in `Verbatim` which are not interpreted by syn. ([#87](https://github.com/Boshen/cargo-shear/pull/87))

### Other

- *(renovate)* bump versions
- *(deps)* update rust crates ([#86](https://github.com/Boshen/cargo-shear/pull/86))
- *(deps)* update rust crates ([#84](https://github.com/Boshen/cargo-shear/pull/84))
- *(deps)* update dependency rust to v1.81.0 ([#83](https://github.com/Boshen/cargo-shear/pull/83))
- *(deps)* update dependency rust to v1.80.1 ([#82](https://github.com/Boshen/cargo-shear/pull/82))
- Update README.md
- Add trophy cases for reqsign ([#80](https://github.com/Boshen/cargo-shear/pull/80))
- *(deps)* update rust crates ([#79](https://github.com/Boshen/cargo-shear/pull/79))
- *(README)* mention rustc and clippy

## [1.1.2](https://github.com/Boshen/cargo-shear/compare/v1.1.1...v1.1.2) - 2024-08-18

### Other
- Add package filtering options ([#75](https://github.com/Boshen/cargo-shear/pull/75))
- *(deps)* update rust crates ([#74](https://github.com/Boshen/cargo-shear/pull/74))
- *(deps)* update rust crates ([#73](https://github.com/Boshen/cargo-shear/pull/73))
- *(deps)* update rust crate serde_json to v1.0.121 ([#72](https://github.com/Boshen/cargo-shear/pull/72))
- *(deps)* update rust crates ([#70](https://github.com/Boshen/cargo-shear/pull/70))

## [1.1.1](https://github.com/Boshen/cargo-shear/compare/v1.1.0...v1.1.1) - 2024-07-25

### Other
- *(deps)* update dependency rust to v1.80.0 ([#69](https://github.com/Boshen/cargo-shear/pull/69))

## [1.1.0](https://github.com/Boshen/cargo-shear/compare/v1.0.1...v1.1.0) - 2024-07-10

### Added
- inherit package level ignore from workspace level ignore ([#64](https://github.com/Boshen/cargo-shear/pull/64))

## [1.0.1](https://github.com/Boshen/cargo-shear/compare/v1.0.0...v1.0.1) - 2024-07-07

### Other
- macos-12

## [1.0.0](https://github.com/Boshen/cargo-shear/compare/v1.0.0...v1.0.0) - 2024-07-05

Release v1.0.0.

Consider `cargo-shear` as stable after using for a few months so we pin version in CI and introduce breaking changes in the future.

## [0.0.26](https://github.com/Boshen/cargo-shear/compare/v0.0.25...v0.0.26) - 2024-05-29

### Added
- exit code is 0 when performing fix ([#52](https://github.com/Boshen/cargo-shear/pull/52))

## [0.0.25](https://github.com/Boshen/cargo-shear/compare/v0.0.24...v0.0.25) - 2024-05-02

### Other
- *(deps)* update dependency rust to v1.78.0 ([#40](https://github.com/Boshen/cargo-shear/pull/40))
- *(renovate)* add rust-toolchain
- *(deps)* update rust crate cargo-util-schemas to 0.3.0 ([#39](https://github.com/Boshen/cargo-shear/pull/39))
- *(deps)* update rust crates ([#38](https://github.com/Boshen/cargo-shear/pull/38))
- *(deps)* update rust crate bpaf to 0.9.12 ([#37](https://github.com/Boshen/cargo-shear/pull/37))
- *(deps)* update rust crate cargo_toml to 0.20.2 ([#36](https://github.com/Boshen/cargo-shear/pull/36))
- *(deps)* update rust crate cargo_toml to 0.20.1 ([#35](https://github.com/Boshen/cargo-shear/pull/35))
- *(deps)* update rust crates ([#34](https://github.com/Boshen/cargo-shear/pull/34))
- *(deps)* update rust crate toml_edit to 0.22.11 ([#33](https://github.com/Boshen/cargo-shear/pull/33))
- *(deps)* update rust crate toml_edit to 0.22.10 ([#32](https://github.com/Boshen/cargo-shear/pull/32))
- *(deps)* update rust crate serde_json to 1.0.116 ([#31](https://github.com/Boshen/cargo-shear/pull/31))
- *(deps)* update rust crate anyhow to 1.0.82 ([#30](https://github.com/Boshen/cargo-shear/pull/30))
- mention `[workspace.metadata.cargo-shear]`

## [0.0.24](https://github.com/Boshen/cargo-shear/compare/v0.0.23...v0.0.24) - 2024-04-09

### Added
- handle package rename in workspace dependencies
- add ignore with [workspace.metadata.cargo-shear]

### Other
- space out printing

## [0.0.23](https://github.com/Boshen/cargo-shear/compare/v0.0.22...v0.0.23) - 2024-04-03

### Fixed
- collect import from all use declarations

### Other
- use [lints.clippy]

## [0.0.22](https://github.com/Boshen/cargo-shear/compare/v0.0.21...v0.0.22) - 2024-04-03

### Fixed
- rust v1.77.0 has a different package id representation

## [0.0.21](https://github.com/Boshen/cargo-shear/compare/v0.0.20...v0.0.21) - 2024-04-03

### Other
- fix github.ref read

## [0.0.20](https://github.com/Boshen/cargo-shear/compare/v0.0.19...v0.0.20) - 2024-04-03

### Added
- add --version

### Other
- simplify code around hashset union
- analyze packages in sequence, make debugging easier
- setup rust with moonrepo

## [0.0.19](https://github.com/Boshen/cargo-shear/compare/v0.0.18...v0.0.19) - 2024-04-02

### Fixed
- use `--all-features` to get all deps

### Other
- update README

## [0.0.18](https://github.com/Boshen/cargo-shear/compare/v0.0.17...v0.0.18) - 2024-04-02

### Added
- use cargo metadata module resolution to get module names instead of package names
- add `profile.release` to Cargo.toml

### Other
- small tweaks

## [0.0.17](https://github.com/Boshen/cargo-shear/compare/v0.0.16...v0.0.17) - 2024-04-01

### Fixed
- ignored packages by package name instead of normalized name

### Other
- fix broken ci
- make `shear_package` the more readable
- minor tweak
- add `--no-deps` to `cargo metadata`
- add `just ready`
- run shear on this repo

## [0.0.16](https://github.com/Boshen/cargo-shear/compare/v0.0.15...v0.0.16) - 2024-03-29

### Added
- better output messages

### Other
- update README about ignoring false positives

## [0.0.15](https://github.com/Boshen/cargo-shear/compare/v0.0.14...v0.0.15) - 2024-03-26

### Other
- fix release

## [0.0.14](https://github.com/Boshen/cargo-shear/compare/v0.0.13...v0.0.14) - 2024-03-26

### Other
- fix release-binaries

## [0.0.13](https://github.com/Boshen/cargo-shear/compare/v0.0.12...v0.0.13) - 2024-03-26

### Fixed
- binary release

### Other
- Rust v1.77.0

## [0.0.12](https://github.com/Boshen/cargo-shear/compare/v0.0.11...v0.0.12) - 2024-03-26

### Other
- add binary after release

## [0.0.11](https://github.com/Boshen/cargo-shear/compare/v0.0.10...v0.0.11) - 2024-03-26

### Other
- add release-plz
- add typos
- add `cargo publish`

## v0.0.10 - 2024-03-25

### Fixed

* Return exit code 0 when there are no unused dependencies, 1 when there are unused dependencies.

## v0.0.9 - 2024-03-25

### Added

* Ignore crate by `[package.metadata.cargo-shear] ignored = ["crate"]`
