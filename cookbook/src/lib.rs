//! # Cookbook

#[cfg(doctest)]
doc_comment::doctest!("intro.md", intro);
#[cfg(doctest)]
doc_comment::doctest!("new_project.md", new_project);
#[cfg(doctest)]
doc_comment::doctest!("component.md", component);
#[cfg(doctest)]
doc_comment::doctest!("nest_component.md", nest_component);
#[cfg(doctest)]
doc_comment::doctest!("rsx.md", rsx);
#[cfg(doctest)]
doc_comment::doctest!("logic_view_comms.md", logic_view_comms);
#[cfg(doctest)]
doc_comment::doctest!("view_capture.md", view_capture);

/// A cookbook
pub fn cookbook() {
    println!("Hello, world!");
}

#[cfg(test)]
mod my_tests {
    #[test]
    fn can_test() {}
}
