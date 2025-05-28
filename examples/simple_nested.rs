use struct_convert::Convert;

#[derive(Debug, PartialEq, Default)]
struct Inner {
    name: String,
}

#[derive(Debug, PartialEq, Default)]
struct Outer {
    id: u64,
    inner: Inner,
}

#[derive(Debug, Convert, PartialEq)]
#[convert(into = "Outer")]
struct Flat {
    id: u64,

    #[convert_field(nested_field = "inner")]
    name: String,
}

fn main() {
    let flat = Flat {
        id: 1,
        name: "test".to_string(),
    };

    let outer: Outer = flat.into();

    println!("Converted: {:#?}", outer);

    assert_eq!(outer.id, 1);
    assert_eq!(outer.inner.name, "test");
}
