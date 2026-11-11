use mac_physics_core::spotlight::{evaluate_math, fuzzy_match, SpotlightCategory, SpotlightItem};

#[test]
fn test_fuzzy_match_ranking() {
    let score_prefix = fuzzy_match("term", "Terminal").expect("Should match");
    let score_sub = fuzzy_match("term", "Apple Intermediate Term").expect("Should match");
    assert!(
        score_prefix > score_sub,
        "Prefix match ({}) should score higher than substring/word match ({})",
        score_prefix,
        score_sub
    );

    // Acronym bonus
    let score_acronym = fuzzy_match("gc", "Google Chrome").expect("Should match acronym");
    assert!(score_acronym > 0);

    // No match
    assert!(fuzzy_match("xyz", "Terminal").is_none());
}

#[test]
fn test_fuzzy_match_acronyms_and_boundaries() {
    // "cmd" -> "Command Prompt"
    let cmd_score = fuzzy_match("cmd", "Command Prompt").expect("cmd should match Command Prompt");
    assert!(cmd_score > 0);

    // CamelCase boundary: "VSCode" -> "Visual Studio Code" or "fb" -> "FooBar"
    let fb_score = fuzzy_match("fb", "FooBar").expect("fb should match FooBar");
    assert!(fb_score > 0);

    // Case insensitivity: both match successfully
    let lower = fuzzy_match("safari", "Safari").expect("case insensitive");
    let upper = fuzzy_match("SAFARI", "safari").expect("case insensitive");
    assert!(lower > 0 && upper > 0);

    // Exact case match gives higher score
    let exact_case = fuzzy_match("Safari", "Safari").unwrap();
    assert!(exact_case > lower);

    // Empty query matches anything with 0
    assert_eq!(fuzzy_match("", "Anything"), Some(0));

    // Target empty but non-empty query -> None
    assert_eq!(fuzzy_match("a", ""), None);
}

#[test]
fn test_evaluate_math_expressions() {
    assert_eq!(evaluate_math("120 * 45"), Some(5400.0));
    assert_eq!(evaluate_math("(100 + 25) / 5"), Some(25.0));
    assert_eq!(evaluate_math("2 ^ 10"), Some(1024.0));
    assert_eq!(evaluate_math("10 / 0"), None); // Div by zero guard
    assert_eq!(evaluate_math("not an expression"), None);
}

#[test]
fn test_evaluate_math_advanced() {
    // Functions
    assert_eq!(evaluate_math("sqrt(16)"), Some(4.0));
    assert_eq!(evaluate_math("sqrt(2 ^ 6)"), Some(8.0));
    assert_eq!(evaluate_math("sqrt(-4)"), None); // Negative sqrt guard

    // Decimals and unary minus
    assert_eq!(evaluate_math("-5 + 10"), Some(5.0));
    assert_eq!(evaluate_math("3.5 * 2"), Some(7.0));
    assert_eq!(evaluate_math("-(3 + 2)"), Some(-5.0));

    // Operator precedence: 2 + 3 * 4 ^ 2 = 2 + 3 * 16 = 2 + 48 = 50
    assert_eq!(evaluate_math("2 + 3 * 4 ^ 2"), Some(50.0));

    // Whitespace handling
    assert_eq!(evaluate_math("   42   "), Some(42.0));
    assert_eq!(evaluate_math(""), None);
    assert_eq!(evaluate_math("   "), None);

    // Syntax errors
    assert_eq!(evaluate_math("1 + "), None);
    assert_eq!(evaluate_math("(1 + 2"), None);
    assert_eq!(evaluate_math("1 + 2)"), None);
}

#[test]
fn test_spotlight_item_and_category_serde() {
    let item = SpotlightItem::new(
        "app.terminal",
        "Terminal",
        "Applications",
        SpotlightCategory::Application,
        "/Applications/Utilities/Terminal.app",
    );

    assert_eq!(item.id, "app.terminal");
    assert_eq!(item.title, "Terminal");
    assert_eq!(item.category, SpotlightCategory::Application);

    // Serde roundtrip
    let json = serde_json::to_string(&item).expect("serialize item");
    let deserialized: SpotlightItem = serde_json::from_str(&json).expect("deserialize item");
    assert_eq!(item, deserialized);

    // Verify all category variants
    let categories = vec![
        SpotlightCategory::Application,
        SpotlightCategory::SystemSetting,
        SpotlightCategory::Calculation,
        SpotlightCategory::File,
        SpotlightCategory::Other,
    ];
    for cat in categories {
        let cat_json = serde_json::to_string(&cat).unwrap();
        let cat_back: SpotlightCategory = serde_json::from_str(&cat_json).unwrap();
        assert_eq!(cat, cat_back);
    }
}

#[test]
fn calculator_bounds_recursive_pasted_input_without_rejecting_long_flat_sums() {
    use mac_physics_core::spotlight::evaluate_math;
    assert_eq!(evaluate_math(&format!("{}1{}", "(".repeat(256), ")".repeat(256))), None);
    assert_eq!(evaluate_math(&format!("{}1", "-".repeat(256))), None);
    assert_eq!(evaluate_math(&vec!["1"; 256].join("^")), None);
    assert_eq!(evaluate_math(&vec!["1"; 256].join("+")), Some(256.0));
}
