# Release

Before creating a release, check that everything can be published to crates.io.

1. If there were changes in `aic-sdk-sys`:
  - Increase workspace version number
  - Test if `aic-sdk-sys` can be published `cargo publish -p aic-sdk-sys --dry-run`
  - Set `aic-sdk-sys` dependency version number in top-level `Cargo.toml` to the newest version
2. Check that the right version numbers are reflected in `README.md`.
3. Create a new release on the GitHub main branch with a tag that has the same version number as the main crate
4. Set local repository to the release tag and publish to crates.io
  - `cargo publish -p aic-sdk-sys` (if changed)
  - `cargo publish -p aic-sdk`
