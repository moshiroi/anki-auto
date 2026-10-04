# Releasing

Releases use GitHub-hosted runners exclusively. Do not upload binaries built on a personal device.

1. Update the version in `Cargo.toml` and run `cargo check --locked` to update `Cargo.lock` if needed.
2. Open a pull request, review the build matrix and installer checks, and have the owner merge it. All five platforms must pass.
3. Tag that reviewed commit with the matching version, such as `v0.1.0`, and push the tag. The `Release binaries` workflow creates a draft release only after every build, path scan, linkage check, dry run, and installer test passes.
4. Check the draft contains five archives, per-archive checksums, `SHA256SUMS`, `install.sh`, `install.ps1`, and `basic.json`. Run the installers against the draft's downloaded assets if changes require further checking.
5. Publish the draft with `gh release edit v0.1.0 --draft=false --latest`. Confirm the README's latest-download links work.

The first release may be built from the owner-authorized open-source setup branch while its PR is pending. The tag points to the exact public source used by the build; publishing a release does not merge the PR or bypass main-branch protection.

## What the checks guarantee

`Cargo.toml` strips symbols and debug data for release builds. `configure-release.py` remaps Rust workspace/home paths and Unix C source paths. `package-release.py` checks the resulting executable for those paths (UTF-8 and UTF-16) and common absolute user-path patterns. Packaging stops if a match is found. It also runs `--version` and JSON dry runs, verifies static Linux linking, and checks that macOS links only system libraries. No check imports or syncs real Anki notes.

The packaging step copies only explicitly named public files. Unix archive owner/group names and modification times are normalized. The public workflow log remains evidence of the build runner and source commit. Compiler versions, target architecture, generic source references, and normal operating-system dependencies are not personal-device identifiers. These are targeted checks, not a claim that all metadata or every possible source path is absent.

Installers verify SHA-256 before executing or installing the downloaded binary. They reject tampered archives without replacing an existing installation. Checksums are fetched over HTTPS from the same release, so they detect corruption rather than compromise of the release account. The binaries are not Apple-notarized or Windows Authenticode-signed.

## Installer tests

The workflow tests Unix installers using a localhost download server and Windows using a mocked download function. Both use temporary install directories and skip persistent PATH changes. They check that the binary and sample are installed and that bad checksums are rejected. Normal installations add a directory to bash/zsh startup files or Windows user PATH; they never need administrator access.
