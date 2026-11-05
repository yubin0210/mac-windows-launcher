use mac_physics_core::spotlight::SpotlightCategory;
use win_launcher::indexer::AppIndexer;
use win_launcher::spotlight_view::SpotlightView;

#[test]
fn test_spotlight_view_typing_and_navigation() {
    let mut indexer = AppIndexer::new();
    indexer.register_mock_item("Terminal", "CLI", SpotlightCategory::Application, "wt.exe");
    indexer.register_mock_item("Task Manager", "System", SpotlightCategory::Application, "taskmgr.exe");

    let mut view = SpotlightView::new(indexer);
    assert_eq!(view.get_query(), "");
    assert!(view.get_results().is_empty());

    view.input_text("t");
    assert_eq!(view.get_results().len(), 2);
    assert_eq!(view.selected_index(), 0);

    view.navigate(1); // Down arrow
    assert_eq!(view.selected_index(), 1);

    view.navigate(-1); // Up arrow wraps
    assert_eq!(view.selected_index(), 0);

    // Navigating down then typing new text resets selection to top hero item
    view.navigate(1);
    assert_eq!(view.selected_index(), 1);
    view.input_text("e");
    assert_eq!(view.selected_index(), 0);

    // Selected item lookup
    let item = view.selected_item().unwrap();
    assert_eq!(item.title, "Terminal");
}

#[test]
fn test_spotlight_view_delete_and_clear() {
    let mut indexer = AppIndexer::new();
    indexer.register_mock_item("Terminal", "CLI", SpotlightCategory::Application, "wt.exe");

    let mut view = SpotlightView::new(indexer);
    view.input_text("term");
    assert_eq!(view.get_query(), "term");
    assert_eq!(view.get_results().len(), 1);

    view.delete_char();
    assert_eq!(view.get_query(), "ter");
    assert_eq!(view.get_results().len(), 1);

    view.clear();
    assert_eq!(view.get_query(), "");
    assert!(view.get_results().is_empty());
    assert_eq!(view.selected_index(), 0);
}

#[test]
fn test_spotlight_view_calculation() {
    let indexer = AppIndexer::new();
    let mut view = SpotlightView::new(indexer);

    view.input_text("24 * 7");
    let results = view.get_results();
    assert!(!results.is_empty());
    assert_eq!(results[0].category, SpotlightCategory::Calculation);
    assert_eq!(results[0].title, "168");
    assert!(view.has_calculation());
}

#[test]
fn test_spotlight_view_navigation_wrapping() {
    let mut indexer = AppIndexer::new();
    indexer.register_mock_item("Alpha", "App", SpotlightCategory::Application, "a.exe");
    indexer.register_mock_item("Beta", "App", SpotlightCategory::Application, "b.exe");
    indexer.register_mock_item("Gamma", "App", SpotlightCategory::Application, "g.exe");

    let mut view = SpotlightView::new(indexer);
    view.input_text("a"); // matches Alpha, Beta, Gamma
    let count = view.results_count();
    assert!(count >= 2);

    // Up arrow at 0 wraps to last item
    view.navigate(-1);
    assert_eq!(view.selected_index(), count - 1);

    // Down arrow at last item wraps back to 0
    view.navigate(1);
    assert_eq!(view.selected_index(), 0);
}
