#![warn(clippy::all)]

use std::path::PathBuf;

#[macro_use]
extern crate serde_derive;

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

    let output = buck::buck_command(file_path.parent().unwrap(), rule).output()?;
    let rules: buck::Rules = serde_json::from_slice(&output.stdout)?;

    println!("{:#?}", rules);

    Ok(())
}
