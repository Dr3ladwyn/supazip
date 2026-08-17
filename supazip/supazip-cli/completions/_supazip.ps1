
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'supazip' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'supazip'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'supazip' {
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('-V', '-V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', '--version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List the contents of an archive')
            [CompletionResult]::new('extract', 'extract', [CompletionResultType]::ParameterValue, 'Extract an archive to a directory')
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create a new archive from one or more files')
            [CompletionResult]::new('test', 'test', [CompletionResultType]::ParameterValue, 'Verify the integrity of an archive')
            [CompletionResult]::new('completions', 'completions', [CompletionResultType]::ParameterValue, 'Generate shell completion scripts (bash, zsh, fish, powershell, elvish)')
            [CompletionResult]::new('man', 'man', [CompletionResultType]::ParameterValue, 'Generate man page')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'supazip;list' {
            [CompletionResult]::new('--password', '--password', [CompletionResultType]::ParameterName, 'Password for encrypted archives')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'supazip;extract' {
            [CompletionResult]::new('--out', '--out', [CompletionResultType]::ParameterName, 'Output directory. Defaults to the current directory')
            [CompletionResult]::new('--password', '--password', [CompletionResultType]::ParameterName, 'Password for encrypted archives')
            [CompletionResult]::new('--entry', '--entry', [CompletionResultType]::ParameterName, 'Extract only the named entries. May be repeated')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
            [CompletionResult]::new('--all', '--all', [CompletionResultType]::ParameterName, 'Extract every entry (the default)')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'supazip;create' {
            [CompletionResult]::new('--format', '--format', [CompletionResultType]::ParameterName, 'Force a specific archive format')
            [CompletionResult]::new('--password', '--password', [CompletionResultType]::ParameterName, 'Password for encrypted archives (only supported for 7z right now)')
            [CompletionResult]::new('--compression', '--compression', [CompletionResultType]::ParameterName, 'Compression method (`store`, `deflate`, `bzip2`, `zstd` for ZIP; ignored by 7z, which uses LZMA2 / AES-256 depending on password)')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'supazip;test' {
            [CompletionResult]::new('--password', '--password', [CompletionResultType]::ParameterName, 'Password for encrypted archives')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'supazip;completions' {
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'supazip;man' {
            [CompletionResult]::new('--out-dir', '--out-dir', [CompletionResultType]::ParameterName, 'out-dir')
            [CompletionResult]::new('--output', '--output', [CompletionResultType]::ParameterName, 'output')
            [CompletionResult]::new('-h', '-h', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            [CompletionResult]::new('--help', '--help', [CompletionResultType]::ParameterName, 'Print help (see more with ''--help'')')
            break
        }
        'supazip;help' {
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List the contents of an archive')
            [CompletionResult]::new('extract', 'extract', [CompletionResultType]::ParameterValue, 'Extract an archive to a directory')
            [CompletionResult]::new('create', 'create', [CompletionResultType]::ParameterValue, 'Create a new archive from one or more files')
            [CompletionResult]::new('test', 'test', [CompletionResultType]::ParameterValue, 'Verify the integrity of an archive')
            [CompletionResult]::new('completions', 'completions', [CompletionResultType]::ParameterValue, 'Generate shell completion scripts (bash, zsh, fish, powershell, elvish)')
            [CompletionResult]::new('man', 'man', [CompletionResultType]::ParameterValue, 'Generate man page')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'supazip;help;list' {
            break
        }
        'supazip;help;extract' {
            break
        }
        'supazip;help;create' {
            break
        }
        'supazip;help;test' {
            break
        }
        'supazip;help;completions' {
            break
        }
        'supazip;help;man' {
            break
        }
        'supazip;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
