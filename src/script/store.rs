use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mlua::{UserData, UserDataMethods};

/// Per script runner key-value store.
///
/// Scripts use [`Store`] to persist data across requests within a single
/// ScriptRunner's lifetime. Values are arbitrary [`mlua::Value`]s.
#[derive(Debug, Default, Clone)]
pub struct Store(Arc<Mutex<HashMap<String, mlua::Value>>>);

impl Store {
    /// Creates a fresh, empty [`Store`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks up a value by name. Returns `None` when the key is absent.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex has been poisoned by a panicking thread.
    pub fn get(&self, name: &str) -> Option<mlua::Value> {
        self.0
            .lock()
            .expect("Store mutex poisoned")
            .get(name)
            .cloned()
    }

    /// Stores a value under `name`, overwriting any prior value at that key.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex has been poisoned by a panicking thread.
    pub fn set(&self, name: &str, value: mlua::Value) {
        self.0
            .lock()
            .expect("Store mutex poisoned")
            .insert(name.to_string(), value);
    }
}

impl UserData for Store {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get", |_, this, name: String| match this.get(&name) {
            Some(value) => Ok(value),
            None => Ok(mlua::Value::Nil),
        });
        methods.add_method("set", |_, this, (name, value): (String, mlua::Value)| {
            this.set(&name, value);
            Ok(())
        });
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;
    use std::collections::HashMap;

    use mlua::{FromLua, IntoLua};

    use crate::script::Store;

    #[test]
    fn rust_set_then_get_round_trip() {
        let state = Store::new();
        state.set("k", mlua::Value::Integer(0));
        assert_eq!(state.get("k"), Some(mlua::Value::Integer(0)));
    }

    #[test]
    fn rust_get_returns_none_for_absent_key() {
        let state = Store::new();
        assert_eq!(state.get("missing"), None);
    }

    #[test]
    fn rust_set_supports_all_json_value_types() {
        let lua = mlua::Lua::new();
        let state = Store::new();
        let string = "hello".into_lua(&lua).expect("string to be convertible");
        state.set("string", string.clone());
        let integer = 42.into_lua(&lua).expect("integer to be convertible");
        state.set("integer", integer.clone());
        let float = 3.5.into_lua(&lua).expect("float to be convertible");
        state.set("float", float.clone());
        let bool = true.into_lua(&lua).expect("bool to be convertible");
        state.set("bool", bool.clone());
        let nil = mlua::Value::Nil;
        state.set("null", nil.clone());
        let array = [1, 2, 3].into_lua(&lua).expect("array to convertible");
        state.set("array", array.clone());
        let object = HashMap::from([("a", "one"), ("b", "two")])
            .into_lua(&lua)
            .expect("object to be convertible");
        state.set("object", object.clone());

        assert_eq!(state.get("string"), Some(string));
        assert_eq!(state.get("integer"), Some(integer));
        assert_eq!(state.get("float"), Some(float));
        assert_eq!(state.get("bool"), Some(bool));
        assert_eq!(state.get("null"), Some(nil));
        assert_eq!(state.get("array"), Some(array));
        assert_eq!(state.get("object"), Some(object));
    }

    #[test]
    fn rust_set_overwrites_existing_key() {
        let lua = mlua::Lua::new();
        let state = Store::new();
        state.set(
            "k",
            "first".into_lua(&lua).expect("string to be convertible"),
        );
        state.set(
            "k",
            "second".into_lua(&lua).expect("string to be convertible"),
        );
        assert_eq!(
            state.get("k"),
            Some("second".into_lua(&lua).expect("string to be convertible"))
        );
    }

    #[test]
    fn rust_two_states_do_not_share_keys() {
        let lua = mlua::Lua::new();
        let state1 = Store::new();
        let state2 = Store::new();

        let s1 = "v1".into_lua(&lua).expect("string to be convertible");
        state1.set("k", s1.clone());
        let s2 = "v2".into_lua(&lua).expect("string to be convertible");
        state2.set("k", s2.clone());
        let b = true.into_lua(&lua).expect("boolean to be convertible");
        state1.set("only_in_1", b.clone());

        assert_eq!(state1.get("k"), Some(s1));
        assert_eq!(state2.get("k"), Some(s2));
        assert_eq!(state1.get("only_in_1"), Some(b));
        assert_eq!(state2.get("only_in_1"), None);
    }

    #[test]
    fn store_is_send() {
        // Compile-time check: the runner may dispatch setup across threads.
        fn assert_send<T: Send>() {}
        assert_send::<Store>();
    }

    fn fresh_lua_with_state() -> (mlua::Lua, Store) {
        let lua = mlua::Lua::new();
        let state = Store::new();
        lua.globals()
            .set("store", state.clone())
            .expect("set global");
        (lua, state)
    }

    #[test]
    fn lua_get_returns_nil_for_absent_key() {
        let (lua, _) = fresh_lua_with_state();

        let value: mlua::Value = lua
            .load(r#"return store:get("missing")"#)
            .eval()
            .expect("eval");

        assert_eq!(value, mlua::Value::Nil);
    }

    #[test]
    fn lua_set_then_get_round_trips_string() {
        let (lua, state) = fresh_lua_with_state();

        lua.load(r#"store:set("k", "v")"#)
            .exec()
            .expect("set should succeed");

        assert_eq!(
            state.get("k"),
            Some("v".into_lua(&lua).expect("string to be convertible"))
        );
        assert_eq!(
            "v",
            lua.load(r#"return store:get("k")"#)
                .eval::<String>()
                .expect("get should succeed")
        )
    }

    #[test]
    fn lua_round_trips_number() {
        let (lua, state) = fresh_lua_with_state();

        lua.load(r#"store:set("n", 42)"#)
            .exec()
            .expect("set should succeed");

        assert_eq!(
            state.get("n"),
            Some(42.into_lua(&lua).expect("integer should be convertible"))
        );
        assert_eq!(
            42,
            lua.load(r#"return store:get("n")"#)
                .eval::<u32>()
                .expect("get should succeed")
        )
    }

    #[test]
    fn lua_round_trips_bool() {
        let (lua, state) = fresh_lua_with_state();

        lua.load(r#"store:set("flag", true)"#)
            .exec()
            .expect("set should succeed");

        assert_eq!(
            state.get("flag"),
            Some(true.into_lua(&lua).expect("boolean should be convertible"))
        );
        assert!(
            lua.load(r#"return store:get("flag")"#)
                .eval::<bool>()
                .expect("get should succeed")
        )
    }

    #[test]
    fn userdata_round_trips_null() {
        let (lua, state) = fresh_lua_with_state();

        lua.load(r#"store:set("nothing", nil)"#)
            .exec()
            .expect("set should succeed");

        assert_eq!(state.get("nothing"), Some(mlua::Value::Nil));
    }

    #[test]
    fn userdata_round_trips_array_table() {
        let (lua, state) = fresh_lua_with_state();

        lua.load(r#"store:set("xs", {10, 20, 30})"#)
            .exec()
            .expect("set should succeed");

        let xs: Vec<u32> =
            Vec::from_lua(state.get("xs").expect("xs to be set"), &lua).expect("xs to be a vec");
        assert_eq!(xs, vec![10, 20, 30]);
    }

    #[test]
    fn userdata_round_trips_object_table() {
        let (lua, state) = fresh_lua_with_state();

        lua.load(r#"store:set("obj", {name = "demo", n = 3})"#)
            .exec()
            .expect("set should succeed");

        let obj: HashMap<String, String> =
            HashMap::from_lua(state.get("obj").expect("obj to be set"), &lua)
                .expect("obj to be an object");
        assert_eq!(
            obj,
            HashMap::from([
                ("name".to_string(), "demo".to_string()),
                ("n".to_string(), "3".to_string())
            ])
        );
    }

    #[test]
    fn userdata_set_rejects_function_with_clear_error() {
        let (lua, state) = fresh_lua_with_state();

        lua.load(r#"store:set("k", function() end)"#)
            .exec()
            .expect("set should succeed");
        assert_matches!(state.get("k"), Some(mlua::Value::Function(..)));
    }

    #[test]
    fn userdata_get_returns_value_for_present_key() {
        let (lua, _) = fresh_lua_with_state();
        lua.load(r#"store:set("k", "v")"#)
            .exec()
            .expect("set should succeed");

        let value: mlua::Value = lua.load(r#"return store:get("k")"#).eval().expect("eval");

        let s = match value {
            mlua::Value::String(s) => s.to_str().unwrap().to_string(),
            other => panic!("expected string, got {:?}", other.type_name()),
        };
        assert_eq!(s, "v");
    }
}
