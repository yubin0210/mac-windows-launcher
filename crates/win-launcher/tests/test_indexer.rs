use mac_physics_core::spotlight::SpotlightCategory;
use win_launcher::indexer::AppIndexer;

#[test]
fn test_indexer_search_and_ranking() {
    let mut indexer = AppIndexer::new();
    indexer.register_mock_item("Terminal", "Command Line", SpotlightCategory::Application, "wt.exe");
    indexer.register_mock_item("Google Chrome", "Web Browser", SpotlightCategory::Application, "chrome.exe");

    let results = indexer.search("gc", 5);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Google Chrome");

    let results_term = indexer.search("term", 5);
    assert_eq!(results_term[0].title, "Terminal");
}

#[test]
fn test_indexer_math_expression() {
    let indexer = AppIndexer::new();
    let results = indexer.search("100 * 5 + 25", 5);
    assert!(!results.is_empty());
    assert_eq!(results[0].category, SpotlightCategory::Calculation);
    assert_eq!(results[0].title, "525");
}

#[test]
fn test_indexer_empty_query() {
    let mut indexer = AppIndexer::new();
    indexer.register_mock_item("Terminal", "Command Line", SpotlightCategory::Application, "wt.exe");
    let results = indexer.search("", 5);
    assert!(results.is_empty());
    let results_whitespace = indexer.search("   ", 5);
    assert!(results_whitespace.is_empty());
}

#[test]
fn test_indexer_limit_truncation() {
    let mut indexer = AppIndexer::new();
    for i in 0..10 {
        indexer.register_mock_item(
            format!("App {i}"),
            "Application",
            SpotlightCategory::Application,
            format!("app{i}.exe"),
        );
    }
    let results = indexer.search("App", 3);
    assert_eq!(results.len(), 3);
}

#[test]
fn test_indexer_build_index() {
    let mut indexer = AppIndexer::new();
    indexer.build_index();
    assert!(indexer.item_count() > 0);

    let display_results = indexer.search("display", 5);
    assert!(!display_results.is_empty());
    assert_eq!(display_results[0].category, SpotlightCategory::SystemSetting);
}
