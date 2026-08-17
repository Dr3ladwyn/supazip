# Shell completions

These files are generated from the live `clap` command via `clap_complete`.
Regenerate after adding or changing CLI flags (from `supazip/`):

```bash
cargo run -p supazip-cli --ignore-rust-version -- completions bash > supazip-cli/completions/supazip.bash
cargo run -p supazip-cli --ignore-rust-version -- completions zsh > supazip-cli/completions/_supazip
cargo run -p supazip-cli --ignore-rust-version -- completions fish > supazip-cli/completions/supazip.fish
cargo run -p supazip-cli --ignore-rust-version -- completions powershell > supazip-cli/completions/_supazip.ps1
cargo run -p supazip-cli --ignore-rust-version -- completions elvish > supazip-cli/completions/supazip.elv
```

On rustc >= 1.92 the `--ignore-rust-version` flag is unnecessary.

Man page:

```bash
cargo run -p supazip-cli --ignore-rust-version -- man --out-dir supazip-cli/man
```

## Installation

- **bash**: `source supazip.bash` or copy to `/etc/bash_completion.d/`
- **zsh**: copy `_supazip` to a directory in `$fpath` (e.g. `~/.zfunc/`)
- **fish**: copy `supazip.fish` to `~/.config/fish/completions/`
- **powershell**: `. _supazip.ps1` in your `$PROFILE`
- **elvish**: `eval (slurp < supazip.elv)`
