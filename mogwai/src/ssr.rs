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
                        format!("<{} />", name)
                    } else {
                        format!("<{} {} />", name, atts)
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
