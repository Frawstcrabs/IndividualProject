mod lang_core;

use lang_core::parse;

fn main() {
    println!("{:?}", parse::parse_base("{set:a;{>{a}};}{a}"));
}