#![warn(clippy::all)]

#[macro_use]
extern crate serde_derive;
extern crate petgraph;

use std::io::Write;
use std::path::PathBuf;

use getopts::Options;

mod buck;
mod graph;

fn main() -> Result<(), failure::Error> {
    let args = std::env::args().collect::<Vec<_>>();

    let mut opts = Options::new();
    opts.reqopt("d", "dir", "Directory to run inside", "DIR");
    opts.reqopt("r", "rule", "Buck rule to translate", "RULE");
    opts.optopt("", "gv", "Graphviz file to output Buck rule graph", "DOT");
    let matches = opts.parse(&args[1..])?;
    let dir = PathBuf::from(matches.opt_str("d").unwrap());
    let rule = matches.opt_str("r").unwrap();

    let root = buck::buck_root(dir)?;
    let rules = buck::query_rules(&root, rule)?;

    println!("{:#?}", rules);
    println!("root: {:#?}", root);

    let dep_graph = graph::dep_graph(&rules);

    if let Some(gv_filename) = matches.opt_str("gv") {
        use petgraph::dot::{Config, Dot};
        use std::fs::OpenOptions;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(gv_filename)?;
        let output = format!("{:?}", Dot::with_config(&dep_graph, &[Config::EdgeNoLabel]));
        let _ = file.write(output.as_bytes());
    }

    use petgraph::visit::Walker;
    let topo = petgraph::visit::Topo::new(&dep_graph);
    for ident in topo.iter(&dep_graph) {
        println!("> {}", ident);
    }

    Ok(())
}
