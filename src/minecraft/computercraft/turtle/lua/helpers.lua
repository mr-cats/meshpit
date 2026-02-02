-- Weirdly, lua doesn't have some methods it really should.
local helpers = {}

--- Deep-copy a table, a lot of the time we do NOT want to take tables
--- by reference. So we need to copy it.
---@param table table
---@return table table a copy of the table
function helpers.deepCopy(table)
    local copy = {}
    for key, val in pairs(table) do
        if type(val) == "table" then
            -- recurse
            copy[key] = helpers.deepCopy(val)
        else
            copy[key] = val
        end
    end
    return copy
end

--- Turn any incoming type into a type we can turn into json.
--- Takes in values or keys, but do note that it does not take in both at the same time.
--- TODO: I'm worried this may cause computers to not yield for a while if the table is large. There needs to
--- be some yield in here.
---@param value any
---@return any any
local function pack(value)
    -- Keep track of tables we've seen, avoids recursion.
    PACK_SEEN = PACK_SEEN or {}

    local t = type(value)

    -- We can directly pass back primitive types.
    if t == "number" or t == "string" or t == "boolean" then
        return value
    end

    -- If this is a nil, we need to use the special nil type.
    if t == "nil" or t == nil then
        ---@diagnostic disable-next-line: undefined-global
        return textutils.json_null
    end

    -- If it's a function, we just put the name of the function
    if t == "function" then
        return "lua function: " .. debug.getinfo (value, "n")
    end

    -- We don't care at all about threads or userdata, discard them entirely.
    -- Note on userdata: Its just a type that lets you store C/C++ values in
    -- ...somewhere? But AFAIK we do not use them.
    if t == "thread" or t == "userdata" then
        -- This is different from a json null, the serializer will completely ignore this value.
        return nil
    end
    
    -- The only remaining type is a table. Thus we will recurse into the table to clean it up.
    -- Unless we have already seen this table, in which case, we just mark it as a duplicate.
    if PACK_SEEN[value] then
        -- Just put in a string
        return "Duplicate table."
    end

    -- Mark the current value as seen, then start turning it into an object of key value pairs.
    PACK_SEEN[value] = true

    local struct = {
        pairs = {}
    }

    for key, value in pairs(value) do
        -- Recuse into keys and values to pack them.
        local packed_key = pack(key)
        local packed_value = pack(value)

        -- Now if either the key or the value is nil, we have no need to store it.
        if packed_key == nil or packed_value == nil then
            goto continue
        end

        -- Otherwise, this pair is now in a serializable format.
        table.insert(struct.pairs, {
            key = packed_key,
            value = packed_value
        })

        ::continue::
    end

    -- Cleanup the global seen list
    PACK_SEEN = {}
    
    -- Return the cleaned table
    return struct
end

--- Unpack our custom json table format back into a normal lua table.
---@param packed any
---@return any unpacked
function unpack(packed)
    -- Only need special logic for tables
    if type(packed) ~= "table" then
        return packed
    end

    -- Check if this is actually a packed table
    if not packed.pairs or type(packed.pairs) ~= "table" then
        -- Not our packed format. Cannot unpack.
        -- Should already be in the correct format then.
        return packed
    end

    local new_table = {}

    -- Unpack our table!
    -- We do not care about the keys from the originating table, as
    -- the keys we want are packed into the pair.
    for _, pair in ipairs(packed) do
        -- If anything is nil (which it should never be) we skip the pair.
        if pair.key == nil or pair.value == nil then
            goto continue
        end

        -- Unpack the inner keys and values, then put them into our new table.
        local unpacked_key = unpack(pair.key)
        local unpacked_value = unpack(pair.value)
        new_table[unpacked_key] = unpacked_value

        ::continue::
    end

    -- All done!
    return new_table
end


--- The built-in json serializer is not good enough.
--- 
--- The textutils.serialiseJSON() method does not work with:
--- - Mixed tables
--- - Non-integer keys into tables
--- - Recursion within tables
--- 
--- Thus we have our own that pre-cleans the data.
---@param input any
---@return string json the outgoing json string.
function helpers.serializeJSON(input)
    -- Make a deep copy of the input as to not alter the incoming table, and
    -- clean it up for export.
    local copy = helpers.deepCopy(input)
    copy = pack(copy)

    -- Serialize that with the standard serializer, now that it's in a
    -- format that it likes.
    local ok, result = pcall(textutils.serializeJSON, copy)
    if not ok then
        -- Well crap!
        panic.panic(tostring(result))
    end

    return result
end

--- Deserialize our custom json format back into tables.
---@param json string
---@return any anything the result of deserialization. You should hopefully know the type.
function helpers.deserializeJSON(json)
    local ok, result = pcall(textutils.deserializeJSON, json)
    if not ok then
        -- Well crap!
        panic.panic(tostring(result))
    end
    -- unpack the data as needed
    -- we can pass by value here.
    return unpack(ok)
end

return helpers