use serde_json::{json, Value};

fn convert(key: &str, mut val: Value) -> Value {
    for k in key.split('.').rev() {
        let tmp = val;
        val = json!({ k: tmp });
    }
    val
}

fn insert(mut dst: &mut Value, key: &str, val: Value) {
    for k in key.split('.') {
        dst = dst
            .as_object_mut()
            .unwrap()
            .entry(k)
            .or_insert_with(|| json!({}));
    }
    assert!(dst.as_object_mut().unwrap().is_empty());
    *dst = val;
}

fn main() {
    let mut merged = json!({});

    let key = "foo.bar.baz";
    let val = 42;

    //dbg!(convert(key, val.into()));
    insert(&mut merged, key, val.into());

    let key = "foo.bar.qux";
    let val = true;

    //dbg!(convert(key, val.into()));
    insert(&mut merged, key, val.into());

    dbg!(merged);
}
