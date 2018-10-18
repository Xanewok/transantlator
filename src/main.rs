#![warn(clippy::all)]

#[macro_use]
extern crate serde_derive;
extern crate petgraph;

use std::path::{Path, PathBuf};

use getopts::Options;

mod buck;
mod graph;
mod translate;

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

    if let Some(gv_filename) = matches.opt_str("gv") {
        let dep_graph = graph::dep_graph(&rules);
        graph::output_graphviz(Path::new(&gv_filename), &dep_graph)?;
    }

    if let Some((target, rule)) = rules.iter().find(|(_, r)| !r.typ.is_supported()) {
        return Err(failure::format_err!(
            "Build target {} (of type {}) not supported",
            target,
            rule.typ.name()
        ));
    }

    translate::translate_rules(&root, rules.iter())?;

    Ok(())
}
