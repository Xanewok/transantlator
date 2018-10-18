use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use petgraph::graphmap::DiGraphMap;
use petgraph::dot::{Config, Dot};

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

pub fn output_graphviz(filename: &Path, graph: &DepGraph<'_>) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(filename)?;
    let output = format!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));

    file.write_all(output.as_bytes())
}
