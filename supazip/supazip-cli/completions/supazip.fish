# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_supazip_global_optspecs
	string join \n output= h/help V/version
end

function __fish_supazip_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_supazip_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_supazip_using_subcommand
	set -l cmd (__fish_supazip_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c supazip -n "__fish_supazip_needs_command" -l output -r -f -a "text\t'Human-readable table (the legacy default)'
json\t'JSON'
yaml\t'YAML'"
complete -c supazip -n "__fish_supazip_needs_command" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c supazip -n "__fish_supazip_needs_command" -s V -l version -d 'Print version'
complete -c supazip -n "__fish_supazip_needs_command" -f -a "list" -d 'List the contents of an archive'
complete -c supazip -n "__fish_supazip_needs_command" -f -a "extract" -d 'Extract an archive to a directory'
complete -c supazip -n "__fish_supazip_needs_command" -f -a "create" -d 'Create a new archive from one or more files'
complete -c supazip -n "__fish_supazip_needs_command" -f -a "test" -d 'Verify the integrity of an archive'
complete -c supazip -n "__fish_supazip_needs_command" -f -a "completions" -d 'Generate shell completion scripts (bash, zsh, fish, powershell, elvish)'
complete -c supazip -n "__fish_supazip_needs_command" -f -a "man" -d 'Generate man page'
complete -c supazip -n "__fish_supazip_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c supazip -n "__fish_supazip_using_subcommand list" -l password -d 'Password for encrypted archives' -r
complete -c supazip -n "__fish_supazip_using_subcommand list" -l output -r -f -a "text\t'Human-readable table (the legacy default)'
json\t'JSON'
yaml\t'YAML'"
complete -c supazip -n "__fish_supazip_using_subcommand list" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c supazip -n "__fish_supazip_using_subcommand extract" -l out -d 'Output directory. Defaults to the current directory' -r -F
complete -c supazip -n "__fish_supazip_using_subcommand extract" -l password -d 'Password for encrypted archives' -r
complete -c supazip -n "__fish_supazip_using_subcommand extract" -l entry -d 'Extract only the named entries. May be repeated' -r
complete -c supazip -n "__fish_supazip_using_subcommand extract" -l output -r -f -a "text\t'Human-readable table (the legacy default)'
json\t'JSON'
yaml\t'YAML'"
complete -c supazip -n "__fish_supazip_using_subcommand extract" -l all -d 'Extract every entry (the default)'
complete -c supazip -n "__fish_supazip_using_subcommand extract" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c supazip -n "__fish_supazip_using_subcommand create" -l format -d 'Force a specific archive format' -r -f -a "zip\t''
7z\t''"
complete -c supazip -n "__fish_supazip_using_subcommand create" -l password -d 'Password for encrypted archives (only supported for 7z right now)' -r
complete -c supazip -n "__fish_supazip_using_subcommand create" -l compression -d 'Compression method (`store`, `deflate`, `bzip2`, `zstd` for ZIP; ignored by 7z, which uses LZMA2 / AES-256 depending on password)' -r
complete -c supazip -n "__fish_supazip_using_subcommand create" -l output -r -f -a "text\t'Human-readable table (the legacy default)'
json\t'JSON'
yaml\t'YAML'"
complete -c supazip -n "__fish_supazip_using_subcommand create" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c supazip -n "__fish_supazip_using_subcommand test" -l password -d 'Password for encrypted archives' -r
complete -c supazip -n "__fish_supazip_using_subcommand test" -l output -r -f -a "text\t'Human-readable table (the legacy default)'
json\t'JSON'
yaml\t'YAML'"
complete -c supazip -n "__fish_supazip_using_subcommand test" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c supazip -n "__fish_supazip_using_subcommand completions" -l output -r -f -a "text\t'Human-readable table (the legacy default)'
json\t'JSON'
yaml\t'YAML'"
complete -c supazip -n "__fish_supazip_using_subcommand completions" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c supazip -n "__fish_supazip_using_subcommand man" -l out-dir -r -F
complete -c supazip -n "__fish_supazip_using_subcommand man" -l output -r -f -a "text\t'Human-readable table (the legacy default)'
json\t'JSON'
yaml\t'YAML'"
complete -c supazip -n "__fish_supazip_using_subcommand man" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c supazip -n "__fish_supazip_using_subcommand help; and not __fish_seen_subcommand_from list extract create test completions man help" -f -a "list" -d 'List the contents of an archive'
complete -c supazip -n "__fish_supazip_using_subcommand help; and not __fish_seen_subcommand_from list extract create test completions man help" -f -a "extract" -d 'Extract an archive to a directory'
complete -c supazip -n "__fish_supazip_using_subcommand help; and not __fish_seen_subcommand_from list extract create test completions man help" -f -a "create" -d 'Create a new archive from one or more files'
complete -c supazip -n "__fish_supazip_using_subcommand help; and not __fish_seen_subcommand_from list extract create test completions man help" -f -a "test" -d 'Verify the integrity of an archive'
complete -c supazip -n "__fish_supazip_using_subcommand help; and not __fish_seen_subcommand_from list extract create test completions man help" -f -a "completions" -d 'Generate shell completion scripts (bash, zsh, fish, powershell, elvish)'
complete -c supazip -n "__fish_supazip_using_subcommand help; and not __fish_seen_subcommand_from list extract create test completions man help" -f -a "man" -d 'Generate man page'
complete -c supazip -n "__fish_supazip_using_subcommand help; and not __fish_seen_subcommand_from list extract create test completions man help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
