//! Provides string rendering for server-side mogwai nodes.

/// Only certain nodes can be "void" - which means written as <tag /> when
/// the node contains no children. Writing non-void nodes in void notation
/// does some spooky things to the DOM at parse-time.
///
/// From https://riptutorial.com/html/example/4736/void-elements
/// HTML 4.01/XHTML 1.0 Strict includes the following void elements:
///
///     area - clickable, defined area in an image
///     base - specifies a base URL from which all links base
///     br - line break
///     col - column in a table [deprecated]
///     hr - horizontal rule (line)
///     img - image
///     input - field where users enter data
///     link - links an external resource to the document
///     meta - provides information about the document
///     param - defines parameters for plugins
///
///     HTML 5 standards include all non-deprecated tags from the previous list and
///
///     command - represents a command users can invoke [obsolete]
///     keygen - facilitates public key generation for web certificates [deprecated]
///     source - specifies media sources for picture, audio, and video elements
fn tag_is_voidable(tag: &str) -> bool {
    tag == "area"
        || tag == "base"
        || tag == "br"
        || tag == "col"
        || tag == "hr"
        || tag == "img"
        || tag == "input"
        || tag == "link"
        || tag == "meta"
        || tag == "param"
        || tag == "command"
        || tag == "keygen"
        || tag == "source"
}

#[derive(Debug)]
pub enum Node {
    Text(String),
    Container {
        name: String,
        attributes: Vec<(String, Option<String>)>,
        children: Vec<Node>,
    },
}

impl From<Node> for String {
    fn from(node: Node) -> String {
        match node {
            Node::Text(s) => s,
            Node::Container {
                name,
                attributes,
                children,
            } => {
                let atts = attributes
                    .iter()
                    .map(|(key, may_val)| {
                        if let Some(val) = may_val {
                            format!(r#"{}="{}""#, key, val)
                        } else {
                            format!("{}", key)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                if children.is_empty() {
                    if attributes.is_empty() {
                        if tag_is_voidable(&name) {
                            format!("<{} />", name)
                        } else {
                            format!("<{}></{}>", name, name)
                        }
                    } else {
                        if tag_is_voidable(&name) {
                            format!("<{} {} />", name, atts)
                        } else {
                            format!("<{} {}></{}>", name, atts, name)
                        }
                    }
                } else {
                    let kids = children
                        .into_iter()
                        .map(|k| String::from(k).trim().to_string())
                        .collect::<Vec<String>>()
                        .join(" ");
                    if attributes.is_empty() {
                        format!("<{}>{}</{}>", name, kids, name)
                    } else {
                        format!("<{} {}>{}</{}>", name, atts, kids, name)
                    }
                }
            }
        }
    }
}
