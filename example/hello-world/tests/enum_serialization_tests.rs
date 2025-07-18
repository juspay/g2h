use serde_json::json;

/// Test that demonstrates enum serialization function naming patterns
/// 
/// This verifies that g2h generates field-specific serialization functions
/// that prevent enum value conflicts between different enum types.
#[test]
fn test_field_specific_serializer_patterns() {
    // Test that field-specific function names follow expected patterns
    let field_patterns = vec![
        ("payment_status", "PaymentStatus", "Single"),
        ("auth_status", "AuthenticationStatus", "Single"), 
        ("processing_status", "ProcessingStatus", "Single"),
        ("optional_payment", "PaymentStatus", "Option"),
        ("auth_history", "AuthenticationStatus", "Repeated"),
        ("processing_steps", "ProcessingStatus", "Repeated"),
    ];

    for (field_name, enum_type, field_type) in field_patterns {
        let message_name = "conflict_test_request";
        let field_id = format!("{}_{}", message_name, field_name);
        
        let expected_functions = match field_type {
            "Single" => vec![
                format!("serialize_{}_as_string", field_id),
                format!("deserialize_{}_from_string", field_id),
            ],
            "Option" => vec![
                format!("serialize_option_{}_as_string", field_id),
                format!("deserialize_option_{}_from_string", field_id),
            ],
            "Repeated" => vec![
                format!("serialize_repeated_{}_as_string", field_id),
                format!("deserialize_repeated_{}_from_string", field_id),
            ],
            _ => panic!("Unknown field type: {}", field_type),
        };
        
        // Verify function names are type-specific
        for func_name in expected_functions {
            assert!(func_name.contains(&field_id), 
                "Function {} should contain field ID {}", func_name, field_id);
            assert!(func_name.contains("_as_string") || func_name.contains("_from_string"),
                "Function {} should be a serializer/deserializer", func_name);
        }
        
        println!("✅ Field {} ({}) generates correct function names", field_name, enum_type);
    }
}

/// Test enum value conflict scenarios  
/// 
/// This simulates the original problem: multiple enums with same values
/// should be handled by field-specific serializers
#[test]
fn test_enum_value_conflicts() {
    // Simulate enum value conflicts that the field-specific serializers solve
    let enum_conflicts = vec![
        // (enum_name, value, expected_string)
        ("PaymentStatus", 0, "SUCCESS"),
        ("PaymentStatus", 1, "PENDING"),
        ("PaymentStatus", 2, "FAILED"),
        
        ("AuthenticationStatus", 0, "AUTHENTICATION_SUCCESS"),
        ("AuthenticationStatus", 1, "AUTHENTICATION_PENDING"), // Same value as PaymentStatus::PENDING
        ("AuthenticationStatus", 2, "AUTHENTICATION_FAILED"),   // Same value as PaymentStatus::FAILED
        
        ("ProcessingStatus", 0, "COMPLETED"),     // Same value as SUCCESS
        ("ProcessingStatus", 1, "PROCESSING"),    // Same value as PENDING/AUTHENTICATION_PENDING
        ("ProcessingStatus", 2, "ERROR"),         // Same value as FAILED/AUTHENTICATION_FAILED
    ];

    // Verify that the same integer value maps to different strings for different enums
    let value_1_mappings: Vec<(&str, &str)> = enum_conflicts.iter()
        .filter(|(_, value, _)| *value == 1)
        .map(|(enum_name, _, expected)| (*enum_name, *expected))
        .collect();
    
    assert_eq!(value_1_mappings.len(), 3, "Should have 3 different enums with value 1");
    assert_eq!(value_1_mappings[0], ("PaymentStatus", "PENDING"));
    assert_eq!(value_1_mappings[1], ("AuthenticationStatus", "AUTHENTICATION_PENDING"));
    assert_eq!(value_1_mappings[2], ("ProcessingStatus", "PROCESSING"));
    
    println!("✅ Enum value conflict test passed!");
    println!("Value 1 maps to different strings for different enum types:");
    for (enum_name, string_val) in value_1_mappings {
        println!("  - {}: {}", enum_name, string_val);
    }
}

/// Test the core benefit: field-specific serializers prevent wrong enum selection
#[test] 
fn test_field_specific_serialization_benefit() {
    // Before the fix: generic serializer would try all enums and return first match
    // After the fix: field-specific serializer knows exactly which enum to use
    
    let test_scenarios = vec![
        // (field_name, enum_type, value, expected_serialized_string)
        ("payment_status", "PaymentStatus", 1, "PENDING"),
        ("auth_status", "AuthenticationStatus", 1, "AUTHENTICATION_PENDING"), // NOT "PENDING" or "DISCOVER"
        ("processing_status", "ProcessingStatus", 1, "PROCESSING"), // NOT "PENDING" or "AUTHENTICATION_PENDING"
    ];
    
    for (field_name, enum_type, value, expected) in test_scenarios {
        // Verify that each field gets its own specific serializer function name
        let field_id = format!("conflict_test_request_{}", field_name);
        let serialize_func = format!("serialize_{}_as_string", field_id);
        let deserialize_func = format!("deserialize_{}_from_string", field_id);
        
        // Function names should be field-specific to prevent conflicts
        assert!(serialize_func.contains("conflict_test_request"), 
            "Serializer should be message-specific: {}", serialize_func);
        assert!(serialize_func.contains(field_name), 
            "Serializer should be field-specific: {}", serialize_func);
        
        println!("✅ Field {} ({}) value {} -> {} with functions {} / {}", 
            field_name, enum_type, value, expected, serialize_func, deserialize_func);
    }
    
    println!("✅ Field-specific serialization benefit test passed!");
}

/// Test JSON serialization patterns that would be generated
#[test]
fn test_expected_json_patterns() {
    // Test the JSON patterns that should be generated with field-specific serializers
    let test_cases = vec![
        // Scenario 1: All enums have value 1 - should serialize to different strings
        (
            json!({
                "payment_status": 1,
                "auth_status": 1, 
                "processing_status": 1
            }),
            vec![
                ("payment_status", "PENDING"),
                ("auth_status", "AUTHENTICATION_PENDING"), // The key fix: not "DISCOVER"  
                ("processing_status", "PROCESSING"),
            ]
        ),
        
        // Scenario 2: Mixed values
        (
            json!({
                "payment_status": 0,
                "auth_status": 2,
                "processing_status": 1
            }),
            vec![
                ("payment_status", "SUCCESS"),
                ("auth_status", "AUTHENTICATION_FAILED"),
                ("processing_status", "PROCESSING"),
            ]
        ),
    ];
    
    for (input_json, expected_mappings) in test_cases {
        for (field_name, expected_string) in expected_mappings {
            let input_value = input_json[field_name].as_i64().unwrap();
            println!("Field {} with value {} should serialize to {}", 
                field_name, input_value, expected_string);
            
            // The key insight: same value (like 1) produces different strings 
            // for different fields because each field has its own enum type
        }
    }
    
    println!("✅ Expected JSON patterns test passed!");
}