// Yes, we even test the lua.
// Since we have our own json serializer and de-serializer, its important to test these.

use mlua::prelude::*;

use crate::minecraft::computercraft::lua_types::table::PairedLuaTable;


const HELPERS: &str = include_str!("helpers.lua");

#[tokio::test]
/// Attempt to serialize an array
async fn lua_array_serialization() {
    let lua = Lua::new();
    // Load in the helper functions
    let helpers: LuaTable = lua.load(HELPERS).eval().unwrap();

    // Grab the json serializers
    let json_serialize_fn: LuaFunction = helpers.get("serializeJSON").unwrap();
    let json_deserialize_fn: LuaFunction = helpers.get("deserializeJSON").unwrap();

    // Try to serialize an array style
    let array_table = lua.create_table().unwrap();
    array_table.push("one").unwrap();
    array_table.push("two").unwrap();
    array_table.push("three").unwrap();

    // Convert that table to json
    let json_from_lua: String = json_serialize_fn.call(array_table.clone()).expect("lua didn't return a string as expected.");

    // try to convert that json back into a table again in lua
    let lua_deserialized: LuaTable = json_deserialize_fn.call(json_from_lua.clone()).unwrap();

    // Check that they match
    assert_eq!(array_table, lua_deserialized); // This should be enough
    assert_eq!(lua_deserialized.pop::<String>().unwrap(), "three"); // but just in case.
    assert_eq!(lua_deserialized.pop::<String>().unwrap(), "two");
    assert_eq!(lua_deserialized.pop::<String>().unwrap(), "one");


    // Now convert that json into our Rust type
    let rust_struct: PairedLuaTable = serde_json::from_str(&json_from_lua).expect("Failed to turn lua's json into a rust type.");
    
    // Make sure it contains what we expect
    let mut rust_copy = rust_struct.clone();
    assert_eq!(rust_copy.pairs.len(), 3);
    // I'm sure this can be done with iterators some how to make it cleaner.
    let mut popped = rust_copy.pairs.pop().unwrap();
    assert!(popped.key.is_number());
    assert!(popped.value.is_string());
    assert_eq!(popped.value.as_str().unwrap(), "three");
    popped = rust_copy.pairs.pop().unwrap();
    assert!(popped.key.is_number());
    assert!(popped.value.is_string());
    assert_eq!(popped.value.as_str().unwrap(), "two");
    popped = rust_copy.pairs.pop().unwrap();
    assert!(popped.key.is_number());
    assert!(popped.value.is_string());
    assert_eq!(popped.value.as_str().unwrap(), "one");

    // Now cast the table back into json again
    let json_from_rust = serde_json::to_string(&rust_struct).expect("Failed to serialize rust type into lua json!");

    // Load it back into lua
    let reload_lua_table: LuaTable = json_deserialize_fn.call(json_from_rust).expect("Unable to deserialize into lua from rust type!");

    // Make sure they are the same
    // for i in reload_lua_table.pairs().zip(array_table.pairs::<u32, String>()) {
    //     assert_eq!(i.0.unwrap(), i.1.unwrap());
    // }

    // Make sure they are the same
    // This assumes the underlying key value pair is u32 and strings. this same pattern cannot
    // be used to compare mixed tables, or tables that contain multiple kinds of values.
    assert!(reload_lua_table.pairs().zip(array_table.pairs::<u32, String>()).all(|p| p.0.unwrap() == p.1.unwrap()));

    // Everything matches!
}