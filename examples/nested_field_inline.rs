use struct_convert::Convert;

#[derive(Debug, PartialEq, Default)]
struct PersonInfo {
    first_name: String,
    last_name: String,
    age: u32,
}

#[derive(Debug, PartialEq, Default)]
struct ContactInfo {
    email: String,
    phone: String,
}

#[derive(Debug, PartialEq, Default)]
struct Person {
    id: u64,
    info: PersonInfo,
    contact: ContactInfo,
    active: bool,
}

#[derive(Debug, Convert, PartialEq)]
#[convert(into = "Person")]
struct FlatPerson {
    id: u64,
    
    #[convert_field(nested_field = "info:PersonInfo")]
    first_name: String,
    
    #[convert_field(nested_field = "info:PersonInfo")]
    last_name: String,
    
    #[convert_field(nested_field = "info:PersonInfo")]
    age: u32,
    
    #[convert_field(nested_field = "contact:ContactInfo")]
    email: String,
    
    #[convert_field(nested_field = "contact:ContactInfo")]
    phone: String,
    
    active: bool,
}

fn main() {
    let flat = FlatPerson {
        id: 1,
        first_name: "John".to_string(),
        last_name: "Doe".to_string(),
        age: 30,
        email: "john.doe@example.com".to_string(),
        phone: "+1234567890".to_string(),
        active: true,
    };
    
    let person: Person = flat.into();
    
    println!("Converted person: {:#?}", person);
    
    assert_eq!(person.id, 1);
    assert_eq!(person.info.first_name, "John");
    assert_eq!(person.info.last_name, "Doe");
    assert_eq!(person.info.age, 30);
    assert_eq!(person.contact.email, "john.doe@example.com");
    assert_eq!(person.contact.phone, "+1234567890");
    assert_eq!(person.active, true);
}

#[test]
fn test_nested_field_inline() {
    let flat = FlatPerson {
        id: 42,
        first_name: "Jane".to_string(),
        last_name: "Smith".to_string(),
        age: 25,
        email: "jane.smith@example.com".to_string(),
        phone: "+0987654321".to_string(),
        active: false,
    };
    
    let person: Person = flat.into();
    
    assert_eq!(
        person,
        Person {
            id: 42,
            info: PersonInfo {
                first_name: "Jane".to_string(),
                last_name: "Smith".to_string(),
                age: 25,
            },
            contact: ContactInfo {
                email: "jane.smith@example.com".to_string(),
                phone: "+0987654321".to_string(),
            },
            active: false,
        }
    );
} 