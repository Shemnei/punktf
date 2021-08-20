# punktf lib

This is the library crate which powers punktf. This crate on it's own is just a
library and is used by [punktf-cli](../punktf-cli) to form `punktf`.

The main components are:

- [Profile](src/profile.rs)
- [Templating](src/template/mod.rs)
- [Hooks](src/hook.rs)
