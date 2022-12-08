//! # Cookbook snippets and pages

doc_comment::doctest!("intro.md", intro);
doc_comment::doctest!("new_project.md", new_project);
doc_comment::doctest!("component.md", component);
doc_comment::doctest!("nest_component.md", nest_component);
doc_comment::doctest!("rsx.md", rsx);
//doc_comment::doctest!("logic_view_comms.md", logic_view_comms);
//doc_comment::doctest!("view_capture.md", view_capture);

#[test]
fn can_test() {
    assert_eq!(1, 1);
}
