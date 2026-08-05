# Release

Before creating a release, check that everything can be published to crates.io.

1. The C SDK version is locked to the crate version (the build fails if they differ), so for every release update these files with the ones distributed in the C SDK:
  - [checksum.txt](aic-sdk-sys/checksum.txt)
  - [aic.h](aic-sdk-sys/include/aic.h)
  - [NOTICE.libaic.txt](aic-sdk-sys/NOTICE.libaic.txt), copied from the C SDK's `NOTICE.txt`

2. Bump the version in the top-level [Cargo.toml](Cargo.toml):
  - `version` under `[workspace.package]` (inherited by all member crates)
  - the `aic-sdk-sys` and `aic-model-downloader` version requirements under `[workspace.dependencies]`, to the same number

3. Regenerate the top-level [NOTICE.txt](NOTICE.txt). Install the generator with `cargo install cargo-about --features=cli` if needed, then run `./scripts/generate-notice.sh`.

4. Update [changelog](CHANGELOG.md)

5. Run `cargo check --features download-lib` (to update `Cargo.lock`, if necessary)

6. Create a new release on the GitHub main branch with a tag that matches the version number
