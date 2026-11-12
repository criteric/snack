# Insert Timestamp

An example **Lua Snack plugin**. Press `Ctrl+P` inside Snack, enter `criteric.insert-timestamp`, and confirm first execution. It inserts the current local time into the open file at the cursor.

Requires Lua (`sudo apt install lua5.4` on Zorin OS). This demo executes with the user's normal permissions; approval is not a sandbox.

Build from source: `snackctl compiler plugins/criteric.insert-timestamp` (preserves a timestamped source backup); the output is in `plugins/criteric.insert-timestamp/dist/`.
