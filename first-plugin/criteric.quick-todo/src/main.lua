-- Quick TODO: official Snack Lua output protocol (0.6 series).
-- First execution requires the editor's user consent.
local date = os.date("%Y-%m-%d")
print("SNACK_INSERT\tTODO [" .. date .. "]: ")
print("SNACK_MESSAGE\tQuick TODO inserted")
