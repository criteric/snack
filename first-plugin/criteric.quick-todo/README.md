# Quick TODO

A minimal Snack Lua extension. Inserts `TODO [YYYY-MM-DD]: ` at the editor cursor.

## Build

From the Snack project directory:

```bash
./target/release/snackctl build first-plugin/criteric.quick-todo
```

## Install

```bash
./target/release/snackctl install first-plugin/criteric.quick-todo/dist/criteric.quick-todo-0.1.0.yummy
```

If you use the prepackaged archive included in `first-plugin/`, you can install it directly:

```bash
./target/release/snackctl install first-plugin/criteric.quick-todo-0.1.0.yummy
```

In Snack press `Ctrl+P`, type `criteric.quick-todo`, press Enter, then type `YES` once to authorize the package. Lua 5.4 is required at runtime.

`snackctl publish` can upload it to `criteric/snack-plugins` only after you have reviewed and tested the code, and have write access to that repository. Do not publish the same version twice.
