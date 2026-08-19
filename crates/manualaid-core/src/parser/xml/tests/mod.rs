use super::*;

fn parse(input: &str) -> ParseOutcome {
    XmlParser.try_parse(input, &EnabledToolSet::all()).unwrap()
}

mod basic;
mod cdata;
mod entities;
mod robustness;
mod templates;
