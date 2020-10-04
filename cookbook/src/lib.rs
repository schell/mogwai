//! # Cookbook

#[cfg(doctest)]
doc_comment::doctest!("component.md", component_md);

#[cfg(doctest)]
doc_comment::doctest!("nest_component.md", component_md);

/// A cookbook
pub fn cookbook() {
    println!("Hello, world!");
}

#[cfg(test)]
mod my_tests {
    #[test]
    fn can_test() {
    }
}
