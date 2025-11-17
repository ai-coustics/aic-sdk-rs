# Release

Before creating a release, check that everything can be published to crates.io.

1. If the C SDK changed:
  - Update expected C SDK version number in [downloader.rs](aic-sdk-sys/build-utils/downloader.rs)
  - Update SHAs of library artifacts in [downloader.rs](aic-sdk-sys/build-utils/downloader.rs)

2. If there were changes in `aic-sdk-sys`:
  - Increase version number in [aic-sdk-sys/Cargo.toml](aic-sdk-sys/Cargo.toml)
  - Set `aic-sdk-sys` dependency version number in top-level [Cargo.toml](Cargo.toml) to the newest version

3. Update the version number of `aic-sdk` in the top level [Cargo.toml](Cargo.toml)

4. Check that the right version number is reflected in [README.md](README.md)

5. Update [changelog](CHANGELOG.md)

5. Create a new release on the GitHub main branch with a tag that has the same version number as the main crate
