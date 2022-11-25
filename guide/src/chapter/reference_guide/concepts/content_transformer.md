# Content Transformer

Transforms run once for each defined dotfile during the deploy process.

They can either be specified for a whole profile, in which case each dotfile is transformed by them or they can be attached to a specific dotfile.

A transform takes the contents of a dotfile, processes it and returns a new version of the content.

The contents are either a resolved template or a non-template dotfile.

## Order of execution

1) Content transformers specified in the profile
2) Content transformers specified in the specific dotfile

## Available Transformers

### LineTerminator

The `LineTerminator` transformer replaces all occurrences of one kind of line terminator with a other one.

#### Options

- `LF` will replace all `\r\n` with a single `\n` (windows style -> unix style)
- `CRLF` will replace all `\n` with a `\r\n` (unix style -> windows style)

#### Usage

Define the following in either the root of the profile or a specific dotfile:

```yaml
transformers:
  - LineTerminator: CRLF
```
