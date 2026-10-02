# Contributing

Fork this repository, create a branch, and open a pull request against `main`.
The repository owner reviews contributions before merging. Direct pushes to
`main`, force pushes, and branch deletion are blocked. New commits dismiss
previous approvals so the final changes are reviewed.

Enter `nix develop` for the development tools, or use your installed Rust toolchain:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Keep tests independent of a live Anki collection. Mock AnkiConnect or test pure
logic. Use `--dry-run` for CLI checks; never import into or sync a real collection
for development. See `AGENTS.md` for the agent workflow and collection safety rules.

The CLI handles deterministic JSON validation, duplicate checking, deck targeting,
import, and sync. Source discovery and card generation stay with the agent; do not
add an AI API dependency to the binary.

The project is licensed under MIT; contributions use that same license.
