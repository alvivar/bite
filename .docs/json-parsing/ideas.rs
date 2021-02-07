pub fn compact(mut dst_json: &mut Value, src_json: &Value) {
    match src_json {
        Value::Object(map) => {
            for (k, v) in map {
                if k == "0" {
                    println!("{}:{}\n", k, v);
                }

                let mut approved = false;
                let mut list: Vec<Value> = Vec::new();

                match v {
                    Value::Object(map2) => {
                        for (k2, v2) in map2 {
                            list.push(v2.clone());
                            if k2 == "0" {
                                approved = true;
                                println!("Child has 0");
                            }
                        }
                    }
                    _ => {}
                }

                let jlist = json!(list);
                if approved {
                    dst_json[k] = jlist.clone();
                    compact(&mut dst_json[k], &jlist);
                } else {
                    dst_json[k] = v.clone();
                    compact(&mut dst_json[k], v);
                }
            }
        }
        _ => {}
    }
}

pub fn deep_keys(value: &Value, current_path: Vec<String>, output: &mut Vec<Vec<String>>) {
    if current_path.len() > 0 {
        output.push(current_path.clone());
    }

    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let mut new_path = current_path.clone();
                new_path.push(k.to_owned());
                deep_keys(v, new_path, output);
            }
        }
        Value::Array(array) => {
            for (i, v) in array.iter().enumerate() {
                let mut new_path = current_path.clone();
                new_path.push(i.to_string().to_owned());
                deep_keys(v, new_path, output);
            }
        }
        _ => (),
    }
}
