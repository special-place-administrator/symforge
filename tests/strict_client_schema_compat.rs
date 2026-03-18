use serde_json::{Map, Value};
use symforge::protocol::SymForgeServer;

fn visit_schema(node: &Value, path: &str) {
    match node {
        Value::Object(object) => visit_schema_object(object, path),
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                visit_schema(item, &format!("{path}[{index}]"));
            }
        }
        _ => {}
    }
}

fn visit_schema_object(object: &Map<String, Value>, path: &str) {
    if object.get("type") == Some(&Value::String("array".to_string())) {
        assert!(
            object.contains_key("items"),
            "strict client compatibility requires `items` for array schema at {path}: {object:?}"
        );
    }

    for (key, value) in object {
        visit_schema(value, &format!("{path}.{key}"));
    }
}

#[test]
fn test_symforge_tool_schemas_are_strict_client_compatible() {
    for tool in SymForgeServer::tool_definitions() {
        let schema = Value::Object(tool.input_schema.as_ref().clone());
        visit_schema(&schema, tool.name.as_ref());
    }
}
