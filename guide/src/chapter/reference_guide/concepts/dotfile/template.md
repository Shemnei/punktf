# Template

## Default system environment variables

`punktf` injects some default system environment variables if they are not present. These variable are set during compilation of `punktf` so you get the values of the compiling system. In most cases the values should be the same when running a compiled executable on another system.

The following are implemented (can be overwritten by defining a environment variable with the same name):

- `PUNKTF_TARGET_ARCH`: Architecture of the system which compiled `punktf`
- `PUNKTF_TARGET_OS`: Operating system of the system which compiled `punktf`
- `PUNKTF_TARGET_FAMILY`: Family of the operating system which compiled `punktf`

The following variables are accessible for templates **and** hooks (can **not** be overwritten by defining a environment variable with the same name):

- `PUNKTF_CURRENT_SOURCE`: `punktf` source directory used for the current operation
- `PUNKTF_CURRENT_TARGET`: `punktf` target directory used for the current operation
- `PUNKTF_CURRENT_PROFILE`: `punktf` profile used for the current operation

The values for these variables are available at <https://doc.rust-lang.org/reference/conditional-compilation.html>.

## Syntax

The syntax is heavily inspired by <https://handlebarsjs.com/>.

### Comment blocks

Document template blocks. Will not be copied over to final output.

#### Syntax

`{{!-- This is a comment --}}`

### Escape blocks

Everything inside will be copied over as is. It can be used to  copied over `{{` or `}}` without it being interpreted as a template block.

#### Syntax

`{{{ This will be copied over {{ as is }} even with the "{{" inside }}}`

### Variable blocks

Define a variable which will be inserted instead of the block. The value of the variable can be gotten from three different environments which can be defined by specifying a prefix:

1) `$`: System environment
2) `#`: Profile-variables defined in the `profile`
2) `&`: Dotfile-variables defined in the `dotfile` section of a profile

To search in more than one environment, these prefixes can be combined. The order they appear in is important, as they will be searched in order of appearance. If one environment does not have a value set for the variable, the next one is searched.

If no prefixes are defined, it will default to `&#`.

Valid symbols/characters for a variable name are: `(a..z|A..Z|0-9|_)`

#### Syntax

`{{$&#OS}}`

### Print blocks

Print blocks will simply print everything contained within the block to the command line. The content of the print block **won't** be resolved, meaning it will be printed 1 to 1 (e.g. no variables are resolved).

#### Syntax

`{{@print Hello World}}`

## If blocks

Supported are `if`, `elif`, `else` and `fi`. Each `if` block must have a `fi` block as a final closing block.
In between the `if` and `fi` block can be zero or multiple `elif` blocks with a final optional `else` block.
Each if related block must be prefixed with `{{@` and end with `}}`.

Currently the only supported if syntax is:

- Check if the value of a variable is (not) equal to the literal given: `{{VAR}} (==|!=) "LITERAL"`
- Check if a value for a variable (not) exists: `(!){{VAR}}`

Other blocks can be nested inside the `if`, `elif` and `else` bodies.

#### Syntax

```text
{{@if {{OS}}}}
        {{@if {{&OS}} != "windows"}}
	        print("OS is not windows")
        {{@elif {{OS}} == "windows"}}
	        {{{!-- This is a nested comment. Below it is a nested variable block. --}}}
	        print("OS is {{OS}}")
        {{@else}}
	        {{{!-- This is a nested comment. --}}}
	        print("Can never get here. {{{ {{OS}} is neither `windows` nor not `windows`. }}}")
        {{@fi}}
{{@else}}
	print("No value for variable `OS` set")
{{@fi}}

{{@if !{{OS}}}}
    {{!-- Run when variable `OS` does not exist/is not set --}}
{{@fi}}
```
