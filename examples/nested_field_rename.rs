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
    
    // Rename "given_name" to "first_name" in the nested struct
    #[convert_field(nested_field = "info", nested_type = "PersonInfo", rename = "first_name")]
    given_name: String,
    
    // Rename "family_name" to "last_name" in the nested struct
    #[convert_field(nested_field = "info", nested_type = "PersonInfo", rename = "last_name")]
    family_name: String,
    
    #[convert_field(nested_field = "info", nested_type = "PersonInfo")]
    age: u32,
    
    // Rename "email_address" to "email" in the nested struct
    #[convert_field(nested_field = "contact", nested_type = "ContactInfo", rename = "email")]
    email_address: String,
    
    // Rename "phone_number" to "phone" in the nested struct
    #[convert_field(nested_field = "contact", nested_type = "ContactInfo", rename = "phone")]
    phone_number: String,
    
    active: bool,
}

fn main() {
    let flat = FlatPerson {
        id: 1,
        given_name: "John".to_string(),
        family_name: "Doe".to_string(),
        age: 30,
        email_address: "john.doe@example.com".to_string(),
        phone_number: "+1234567890".to_string(),
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
fn test_nested_field_rename() {
    let flat = FlatPerson {
        id: 42,
        given_name: "Jane".to_string(),
        family_name: "Smith".to_string(),
        age: 25,
        email_address: "jane.smith@example.com".to_string(),
        phone_number: "+0987654321".to_string(),
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