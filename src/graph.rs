use petgraph::graphmap::DiGraphMap;

use crate::buck::{BuildTarget, Rules};

pub type DepGraph<'a> = DiGraphMap<&'a BuildTarget, ()>;

pub fn dep_graph(rules: &Rules) -> DepGraph {
    let mut graph = DepGraph::new();

    for (target, rule) in rules {
        graph.add_node(target);
        for dep in &rule.common.deps {
            graph.add_edge(target, &dep, ());
        }
    }

    graph
}
