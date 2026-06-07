# Code Coverage

SupaZip uses [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) for
source-based code coverage. The CI workflow
(`.github/workflows/coverage.yml`) runs on every push and PR targeting
`master`, uploads an LCOV report to Codecov, and enforces an **80 %
minimum**.

## Running locally

```bash
# One-time install (requires Rust nightly or stable with llvm-tools):
cargo install cargo-llvm-cov

# Run coverage and open the HTML report:
cargo llvm-cov --workspace --open
```

The `--open` flag generates an HTML report and opens it in your default
browser.  Without it, a summary is printed to stdout.

## Updating the threshold

Edit the number `80` in `.github/workflows/coverage.yml` — both in the
step name and in the `bc` comparison:

```yaml
- name: Check threshold (80%)
  run: |
    COV=$(...)
    if [ "$(echo "$COV < 80" | bc)" -eq 1 ]; then
      #                                     ^^ change this
```

## Ignoring files

To exclude test helpers, benches, or fuzz harnesses from the report, pass
`--ignore-filename-regex`:

```bash
cargo llvm-cov --workspace --open \
  --ignore-filename-regex '(tests|benches|fuzz)/'
```

In CI the flag can be added to the `cargo llvm-cov` invocation in
`coverage.yml`.

## Badge

The README badge links to the Codecov dashboard. Replace `<owner>` in the
URL with your GitHub user or organisation name.
