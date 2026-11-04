# Contributing

All Git commits must be signed. Use your existing signing setup; this project does not require sharing a private key or generating a new one. Configure your local workflow to require signing, create signed commits, and verify each signature before pushing. A failed signature must stop the commit workflow; do not bypass it with `--no-gpg-sign` or disabled signing configuration.

Release tags must also be signed annotated tags. Do not publish an unsigned replacement when signing is unavailable. CI should validate source signatures before publishing, but repository branch protection and verified signatures remain an owner's responsibility.

Run `cargo fmt --check`, `cargo test --locked`, and `cargo build --release --locked` for Rust changes. Run distribution fixture tests and shell syntax checks for installer/update changes. Windows and macOS native execution belongs in their respective CI runners; identify local cross-target checks separately in reports.

Preserve cache data and user configuration. Tests use isolated temporary fixtures, never real cache deletion. Keep README version/date, affected references, and CHANGELOG synchronized. Changes to safety policy need review independently of implementation.

No commit or release is implied by creating source files. Publication requires a confirmed repository, working signing, explicit destination authorization, successful applicable checks, and a reviewed release.
