
use builtin;
use str;

set edit:completion:arg-completer[supazip] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'supazip'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'supazip'= {
            cand --output 'output'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
            cand -V 'Print version'
            cand --version 'Print version'
            cand list 'List the contents of an archive'
            cand extract 'Extract an archive to a directory'
            cand create 'Create a new archive from one or more files'
            cand test 'Verify the integrity of an archive'
            cand completions 'Generate shell completion scripts (bash, zsh, fish, powershell, elvish)'
            cand man 'Generate man page'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'supazip;list'= {
            cand --password 'Password for encrypted archives'
            cand --output 'output'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'supazip;extract'= {
            cand --out 'Output directory. Defaults to the current directory'
            cand --password 'Password for encrypted archives'
            cand --entry 'Extract only the named entries. May be repeated'
            cand --output 'output'
            cand --all 'Extract every entry (the default)'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'supazip;create'= {
            cand --format 'Force a specific archive format'
            cand --password 'Password for encrypted archives (only supported for 7z right now)'
            cand --compression 'Compression method (`store`, `deflate`, `bzip2`, `zstd` for ZIP; ignored by 7z, which uses LZMA2 / AES-256 depending on password)'
            cand --output 'output'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'supazip;test'= {
            cand --password 'Password for encrypted archives'
            cand --output 'output'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'supazip;completions'= {
            cand --output 'output'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'supazip;man'= {
            cand --out-dir 'out-dir'
            cand --output 'output'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'supazip;help'= {
            cand list 'List the contents of an archive'
            cand extract 'Extract an archive to a directory'
            cand create 'Create a new archive from one or more files'
            cand test 'Verify the integrity of an archive'
            cand completions 'Generate shell completion scripts (bash, zsh, fish, powershell, elvish)'
            cand man 'Generate man page'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'supazip;help;list'= {
        }
        &'supazip;help;extract'= {
        }
        &'supazip;help;create'= {
        }
        &'supazip;help;test'= {
        }
        &'supazip;help;completions'= {
        }
        &'supazip;help;man'= {
        }
        &'supazip;help;help'= {
        }
    ]
    $completions[$command]
}
