#![warn(clippy::all)]

#[macro_use]
extern crate serde_derive;

use std::path::PathBuf;

use getopts::Options;

mod buck;

fn main() -> Result<(), failure::Error> {
    let args = std::env::args().collect::<Vec<_>>();

    let mut opts = Options::new();
    opts.reqopt("d", "dir", "Directory to run in", "DIR");
    opts.reqopt("r", "rule", "Buck rule to translate", "RULE");
    let matches = opts.parse(&args[1..])?;
    let dir = PathBuf::from(matches.opt_str("d").unwrap());
    let rule = matches.opt_str("r").unwrap();

    let root = buck::buck_root(dir)?;
    let rules = buck::query_rules(&root, rule)?;

    println!("{:#?}", rules);
    println!("root: {:#?}", root);

    Ok(())
}
