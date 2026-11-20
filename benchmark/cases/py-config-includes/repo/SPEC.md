# Config files

A file is read line by line. Leading and trailing whitespace of every line is ignored.

- An empty line, or one starting with `#` or `;`, is skipped.
- `[name]` starts a section: the keys after it are `name.key`. Keys before any section have no prefix.
- `key = value` defines a key; the value is everything after the first `=`, trimmed. A later definition of the same full key replaces an earlier one, wherever it was read.
- `%include path` reads another file at that point, as if its lines were here, with the section reset to none while it is read and restored after. `path` is relative to the directory of the file that has the `%include`. A file that does not exist raises `ConfigError("missing include: <path as written>")`. A file that includes itself, directly or through others, raises `ConfigError("include cycle: a.cfg -> b.cfg -> a.cfg")`, listing the file names (no directories) from the first file of the cycle back to it.

## Interpolation

After every file is read, values are resolved:

- `${key}` is replaced by the resolved value of the full key `key`.
- `${env:NAME}` is replaced by `env[NAME]` (the `env` argument, a dict; `None` means an empty one).
- `$$` is a literal `$`; what follows it is not interpolated.
- An unknown key raises `ConfigError("undefined: key")`; an unknown variable `ConfigError("undefined: env:NAME")`.
- A key whose value needs itself, directly or through others, raises `ConfigError("interpolation cycle: a -> b -> a")`: the keys in the order they were followed, starting and ending with the first key of the cycle reached, where keys are resolved in the order they were first defined.

`load` returns a `dict` of full keys to resolved strings.
