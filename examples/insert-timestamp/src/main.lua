-- Snack v0.6 plugin output protocol: one action per line.
-- SNACK_INSERT <TAB> text at the cursor; SNACK_MESSAGE <TAB> status text.
local now = os.date("%Y-%m-%d %H:%M:%S")
print("SNACK_INSERT\t" .. now)
print("SNACK_MESSAGE\tTimestamp inserted")
