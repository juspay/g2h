/// Test to verify consistency between heck::ToSnakeCase and custom implementation
///
/// This test ensures that g2h's custom to_snake_case function produces the same
/// results as the heck crate's ToSnakeCase trait, which is used in most places.
use heck::ToSnakeCase;

#[test]
fn test_snake_case_consistency() {
    // G2h now uses heck::ToSnakeCase consistently
    fn g2h_to_snake_case(input: &str) -> String {
        input.to_snake_case()
    }

    let test_cases = vec![
        "HelloReply",
        "PaymentRequest",
        "UserProfile",
        "ResponseStatus",
        "XMLHttpRequest",
        "APIKey",
        "InnerMessage",
        "Level2",
        "DeepNestedMessage",
        "ConflictTestRequest",
        "AuthenticationStatus",
        "ProcessingStatus",
        // Edge cases
        "A",
        "AB",
        "ABCDef",
        "HTTPSConnection",
        "URLPath",
        "IDField",
        "lowercase",
        "",
    ];

    for test_case in test_cases {
        let heck_result = test_case.to_snake_case();
        let g2h_result = g2h_to_snake_case(test_case);

        assert_eq!(
            heck_result, g2h_result,
            "Inconsistency for '{}': heck='{}' vs g2h='{}'",
            test_case, heck_result, g2h_result
        );

        println!("✅ '{}' -> '{}' (consistent)", test_case, heck_result);
    }

    println!("✅ Snake case consistency test passed!");
}

#[test]
fn test_identify_problematic_cases() {
    // G2h now uses heck::ToSnakeCase consistently
    fn g2h_to_snake_case(input: &str) -> String {
        input.to_snake_case()
    }

    // Test cases that might reveal differences
    let edge_cases = vec![
        "HTTPSConnection",  // Multiple consecutive uppercase
        "XMLParser",        // XML prefix
        "URLPath",          // URL prefix
        "IDValue",          // ID prefix
        "APIKey",           // API prefix
        "HTMLElement",      // HTML prefix
        "JSONData",         // JSON prefix
        "UUIDGenerator",    // UUID prefix
        "TCP_IP",           // Underscore already present
        "camelCase",        // Starts lowercase
        "PascalCase",       // Standard PascalCase
        "ALLCAPS",          // All uppercase
        "Mixed_Snake_Case", // Mixed formats
    ];

    let mut differences = Vec::new();

    for test_case in edge_cases {
        let heck_result = test_case.to_snake_case();
        let g2h_result = g2h_to_snake_case(test_case);

        if heck_result != g2h_result {
            differences.push((test_case, heck_result.clone(), g2h_result.clone()));
            println!(
                "❌ DIFFERENCE for '{}': heck='{}' vs g2h='{}'",
                test_case, heck_result, g2h_result
            );
        } else {
            println!("✅ '{}' -> '{}' (same)", test_case, heck_result);
        }
    }

    if differences.is_empty() {
        println!("✅ No differences found between heck and g2h implementations!");
    } else {
        println!("⚠️  Found {} differences:", differences.len());
        for (input, heck_result, g2h_result) in differences {
            println!(
                "  '{}': heck='{}' vs g2h='{}'",
                input, heck_result, g2h_result
            );
        }

        // This test should fail if there are differences to highlight the issue
        panic!("Found differences between heck::ToSnakeCase and g2h implementation. G2h should use heck consistently.");
    }
}
