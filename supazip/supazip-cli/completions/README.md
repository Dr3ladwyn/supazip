# Shell completions

Regenerate after adding new CLI flags:

```bash
cargo run -p supazip-cli -- completions bash > supazip.bash
cargo run -p supazip-cli -- completions zsh > _supazip
cargo run -p supazip-cli -- completions fish > supazip.fish
cargo run -p supazip-cli -- completions powershell > _supazip.ps1
cargo run -p supazip-cli -- completions elvish > supazip.elv
```

## Installation

- **bash**: `source supazip.bash` or copy to `/etc/bash_completion.d/`
- **zsh**: copy `_supazip` to a directory in `$fpath` (e.g. `~/.zfunc/`)
- **fish**: copy `supazip.fish` to `~/.config/fish/completions/`
- **powershell**: `. _supazip.ps1` in your `$PROFILE`
- **elvish**: `eval (slurp < supazip.elv)`
