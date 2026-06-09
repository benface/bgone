# Agent notes

## Release workflow

bgone is distributed through three channels:

1. **GitHub Releases** — pre-built macOS binaries (aarch64 + x86_64) attached to a tagged release
2. **Homebrew** — formula in [`benface/homebrew-bgone`](https://github.com/benface/homebrew-bgone) pointing at the GitHub release tarballs
3. **crates.io** — source crate (`cargo install bgone`)

There is no GitHub Actions release workflow — releases are driven from the developer's machine. The local Homebrew tap clone lives at [`./homebrew-bgone`](./homebrew-bgone) (gitignored in this repo, has its own git remote).

### Prerequisites

- On macOS, Apple Silicon (`arm64`).
- Rust toolchain with both `aarch64-apple-darwin` and `x86_64-apple-darwin` targets installed (verify with `rustup target list --installed`; install missing ones with `rustup target add <target>`).
- Authenticated `gh` CLI (`gh auth status`).
- Authenticated `cargo` for crates.io publishing (`cargo login` if `~/.cargo/credentials.toml` is missing).
- Working tree clean, on `main`, version already bumped in `Cargo.toml`, and the bump commit pushed to `origin/main`.

### Steps

Replace `<VERSION>` with the new version (e.g. `0.6.0`). The tag is always `v<VERSION>`.

1. **Sanity-check the release commit**:

   ```bash
   git status                       # must be clean
   grep '^version' Cargo.toml       # must match <VERSION>
   git log --oneline -1             # must be the "Bump version to <VERSION>" commit
   ```

2. **Build release binaries** for both Apple architectures:

   ```bash
   cargo build --release --target aarch64-apple-darwin
   cargo build --release --target x86_64-apple-darwin
   ```

3. **Package + checksum** each binary as a tarball named `bgone-v<VERSION>-<TARGET>.tar.gz` containing just the `bgone` executable. Working in a temp dir like `dist/`:

   ```bash
   mkdir -p dist
   for TARGET in aarch64-apple-darwin x86_64-apple-darwin; do
     tar -czf "dist/bgone-v<VERSION>-${TARGET}.tar.gz" \
       -C "target/${TARGET}/release" bgone
   done
   shasum -a 256 dist/*.tar.gz
   ```

   Record the two SHA256 values — they're needed for both the release notes and the Homebrew formula.

4. **Tag and push**:

   ```bash
   git tag v<VERSION>
   git push origin v<VERSION>
   ```

5. **Create the GitHub release** with both tarballs attached. The release notes mirror the `## [<VERSION>]` section of `CHANGELOG.md`, followed by Installation and Checksums blocks (see prior releases for exact formatting). Pass the body via heredoc:

   ```bash
   gh release create v<VERSION> \
     dist/bgone-v<VERSION>-aarch64-apple-darwin.tar.gz \
     dist/bgone-v<VERSION>-x86_64-apple-darwin.tar.gz \
     --title "v<VERSION>" \
     --notes "$(cat <<'EOF'
   ## What's New

   <copy the CHANGELOG section for this version verbatim>

   ## Installation

   ### Homebrew (macOS)
   \`\`\`bash
   brew tap benface/bgone
   brew install bgone
   \`\`\`

   ### Cargo
   \`\`\`bash
   cargo install bgone
   \`\`\`

   ## Checksums (SHA256)
   - \`bgone-v<VERSION>-aarch64-apple-darwin.tar.gz\`: \`<sha>\`
   - \`bgone-v<VERSION>-x86_64-apple-darwin.tar.gz\`: \`<sha>\`
   EOF
   )"
   ```

6. **Update the Homebrew tap**. In `./homebrew-bgone`:

   ```bash
   cd homebrew-bgone
   # Edit bgone.rb: bump `version`, update both sha256 values
   git add bgone.rb
   git commit -m "Update to v<VERSION>"
   git push
   cd ..
   ```

   The formula's URLs use `https://github.com/benface/bgone/releases/download/v#{version}/bgone-v#{version}-<TARGET>.tar.gz` and reference `version`, so only the `version` line and the two `sha256` lines need to change. (Leave the unused Linux block alone — past releases haven't shipped a Linux binary.)

7. **Publish to crates.io**:

   ```bash
   cargo publish --dry-run    # verify packaging
   cargo publish              # irreversible — can only be yanked, not removed
   ```

### Verification

- `brew update && brew upgrade bgone` from a clean machine should pull the new tarball.
- `cargo install bgone` should fetch the new version from crates.io.
- `bgone --version` should print `<VERSION>`.

### Things to NOT do without explicit approval

These steps publish to shared/permanent infrastructure. Do each one only after the user has confirmed (or has standing approval) the release is ready:

- `git push origin v<VERSION>` (pushing the tag)
- `gh release create` (publishes the release page + tarballs publicly)
- pushing the Homebrew tap update (changes what `brew install bgone` does for everyone)
- `cargo publish` (irreversible — only yanking is possible afterwards)
