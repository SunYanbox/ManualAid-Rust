use super::*;

fn parse(input: &str) -> ParseOutcome {
    InvokeParser
        .try_parse(input, &EnabledToolSet::all())
        .unwrap()
}

mod attrs;
mod basic;
mod cdata;
mod entities;
mod robustness;
