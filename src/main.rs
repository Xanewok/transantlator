#![warn(clippy::all)]

#[macro_use]
extern crate serde_derive;

use std::path::PathBuf;

use getopts::Options;

mod buck;

fn main() -> Result<(), failure::Error> {
    let args = std::env::args().collect::<Vec<_>>();

    let mut opts = Options::new();
    opts.reqopt("f", "buck-file", "Buck manifest to translate", "BUCK-FILE");
    opts.reqopt("r", "rule", "Buck rule to translate", "RULE");
    let matches = opts.parse(&args[1..])?;
    let file_path = PathBuf::from(matches.opt_str("f").unwrap());
    let rule = matches.opt_str("r").unwrap();

    let root = buck::buck_root(file_path.parent().unwrap())?;
    let rules = buck::query_rules(&root, rule)?;

    println!("{:#?}", rules);
    println!("root: {:#?}", root);

    Ok(())
}
