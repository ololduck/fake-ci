#[cfg(test)]
mod tests {
    #[test]
    fn test_get_nexts() {}
}

struct DAGNode {
    name: String,
    depends_on: Vec<Self>,
    complete: bool,
}

struct DAG {
    nodes: Vec<DAGNode>,
}

impl DAG {
    pub(crate) fn nexts(self) -> Vec<DAGNode> {
        unimplemented!();
    }
}
