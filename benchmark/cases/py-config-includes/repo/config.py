"""A small config loader: sections and key = value lines."""


class ConfigError(Exception):
    pass


def load(path, env=None):
    values = {}
    section = None
    with open(path, encoding="utf-8") as f:
        for raw in f:
            line = raw.strip()
            if not line or line[0] in "#;":
                continue
            if line.startswith("[") and line.endswith("]"):
                section = line[1:-1].strip()
                continue
            key, _, value = line.partition("=")
            key = key.strip()
            full = f"{section}.{key}" if section else key
            values[full] = value.strip()
    return values
