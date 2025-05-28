use struct_convert::Convert;

#[derive(Debug, PartialEq, Default)]
struct Level3 {
    value: String,
    number: u32,
}

#[derive(Debug, PartialEq, Default)]
struct Level2 {
    name: String,
    level3: Level3,
}

#[derive(Debug, PartialEq, Default)]
struct Level1 {
    id: u64,
    level2: Level2,
}

#[derive(Debug, PartialEq, Default)]
struct Root {
    root_field: String,
    level1: Level1,
}

#[derive(Debug, Convert, PartialEq)]
#[convert(into = "Root")]
struct FlatStruct {
    root_field: String,

    // Level 1 nesting
    #[convert_field(nested_field = "level1", nested_type = "Level1")]
    id: u64,

    // Level 2 nesting (level1.level2)
    #[convert_field(nested_field = "level1.level2", nested_type = "Level1")]
    name: String,

    // Level 3 nesting (level1.level2.level3)
    #[convert_field(nested_field = "level1.level2.level3", nested_type = "Level1")]
    value: String,

    #[convert_field(nested_field = "level1.level2.level3", nested_type = "Level1")]
    number: u32,
}

// Example with explicit type annotations at each level
#[derive(Debug, Convert, PartialEq)]
#[convert(into = "Root")]
struct FlatStructWithTypes {
    root_field: String,

    // Level 1 nesting with explicit type
    #[convert_field(nested_field = "level1:Level1")]
    id: u64,

    // Level 2 nesting with explicit types
    #[convert_field(nested_field = "level1:Level1.level2:Level2")]
    name: String,

    // Level 3 nesting with explicit types
    #[convert_field(nested_field = "level1:Level1.level2:Level2.level3:Level3")]
    value: String,

    #[convert_field(nested_field = "level1:Level1.level2:Level2.level3:Level3")]
    number: u32,
}

fn main() {
    // Test basic deep nesting
    let flat = FlatStruct {
        root_field: "root".to_string(),
        id: 42,
        name: "test".to_string(),
        value: "deep_value".to_string(),
        number: 123,
    };

    let root: Root = flat.into();
    println!("Converted root: {:#?}", root);

    assert_eq!(root.root_field, "root");
    assert_eq!(root.level1.id, 42);
    assert_eq!(root.level1.level2.name, "test");
    assert_eq!(root.level1.level2.level3.value, "deep_value");
    assert_eq!(root.level1.level2.level3.number, 123);

    // Test with explicit types
    let flat_typed = FlatStructWithTypes {
        root_field: "root_typed".to_string(),
        id: 99,
        name: "typed_test".to_string(),
        value: "typed_deep_value".to_string(),
        number: 456,
    };

    let root_typed: Root = flat_typed.into();
    println!("Converted root with types: {:#?}", root_typed);

    assert_eq!(root_typed.root_field, "root_typed");
    assert_eq!(root_typed.level1.id, 99);
    assert_eq!(root_typed.level1.level2.name, "typed_test");
    assert_eq!(root_typed.level1.level2.level3.value, "typed_deep_value");
    assert_eq!(root_typed.level1.level2.level3.number, 456);
}

#[test]
fn test_deep_nested_field() {
    let flat = FlatStruct {
        root_field: "test_root".to_string(),
        id: 1,
        name: "nested_name".to_string(),
        value: "nested_value".to_string(),
        number: 789,
    };

    let root: Root = flat.into();

    assert_eq!(
        root,
        Root {
            root_field: "test_root".to_string(),
            level1: Level1 {
                id: 1,
                level2: Level2 {
                    name: "nested_name".to_string(),
                    level3: Level3 {
                        value: "nested_value".to_string(),
                        number: 789,
                    },
                },
            },
        }
    );
}

#[test]
fn test_deep_nested_field_with_types() {
    let flat = FlatStructWithTypes {
        root_field: "test_root_typed".to_string(),
        id: 2,
        name: "typed_nested_name".to_string(),
        value: "typed_nested_value".to_string(),
        number: 987,
    };

    let root: Root = flat.into();

    assert_eq!(
        root,
        Root {
            root_field: "test_root_typed".to_string(),
            level1: Level1 {
                id: 2,
                level2: Level2 {
                    name: "typed_nested_name".to_string(),
                    level3: Level3 {
                        value: "typed_nested_value".to_string(),
                        number: 987,
                    },
                },
            },
        }
    );
}
