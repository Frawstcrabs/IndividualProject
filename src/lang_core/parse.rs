extern crate nom;
use nom::{
    IResult,
    multi::{many0, fold_many0},
    bytes::complete::{tag, take_until, take_till1},
    combinator::{not, map},
    branch::alt
};

#[derive(Clone, Debug)]
pub enum AST {
    String(String),
    Function(Vec<Vec<AST>>),
    Variable(Vec<Vec<AST>>)
}

fn parse_comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = tag("{!")(input)?;

    pub fn parse_comment_str(input: &str) -> IResult<&str, ()> {
        let (input, _) = not(
            alt((tag("{!"), tag("!}")))
        )(input)?;

        let (input, _) = alt((
            take_until("{!"),
            take_until("!}")
        ))(input)?;

        Ok((input, ()))
    }

    let (input, _) = many0(
        alt((parse_comment, parse_comment_str))
    )(input)?;

    let (input, _) = tag("!}")(input)?;
    Ok((input, ()))
}

pub fn parse_base(input: &str) -> IResult<&str, Vec<AST>> {
    fold_many0(
        alt((
            map(take_till1(|c| c == '{'), Some),
            map(parse_comment, |_| None),
        )),
        Vec::new(),
        |mut vec, r: Option<&str>| {
            if let Some(s) = r {
                match vec.last_mut() {
                    Some(AST::String(ast_str)) => {
                        ast_str.push_str(s);
                    }
                    _ => {
                        vec.push(AST::String(s.to_owned()));
                    }
                }
            }
            vec
        }
    )(input)
}