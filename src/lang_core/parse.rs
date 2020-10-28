extern crate nom;
use nom::{
    IResult,
    multi::{many0, fold_many0},
    bytes::complete::{tag, take_until, take_till1},
    combinator::{not, map},
    branch::alt,
    character::complete::char,
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

fn add_block_arg(mut vec: Vec<AST>, r: Option<AST>) -> Vec<AST> {
    if let Some(ast) = r {
        match (&ast, vec.last_mut()) {
            (AST::String(s), Some(AST::String(ast_str))) => {
                ast_str.push_str(s);
            }
            _ => {
                vec.push(ast);
            }
        }
    }
    vec
}

fn parse_block_arg(chars: &[char]) -> impl Fn(&str) -> IResult<&str, Vec<AST>> + '_ {
    move |i: &str| {
        fold_many0(
            alt((
                map(take_till1(|c| chars.contains(&c)), |s: &str| Some(AST::String(s.to_owned()))),
                map(parse_comment, |_| None),
                map(parse_block, Some)
            )),
            Vec::new(),
            add_block_arg
        )(i)
    }
}

fn parse_block_args(mut input: &str) -> IResult<&str, (Vec<Vec<AST>>, &str)> {
    let mut args = Vec::new();
    loop {
        let (i, arg) = parse_block_arg(&['{',':',';','}'])(input)?;
        let (i, sep) = alt((
            tag(";}"),
            tag(";"),
            tag("}")
        ))(i)?;
        args.push(arg);
        match sep {
            ";" => {
                input = i;
                continue;
            }
            ";}" => {
                return Ok((i, (args, ";}")));
            }
            "}" => {
                return Ok((i, (args, "}")));
            }
            _ => unreachable!()
        }
    }
}

fn parse_block(input: &str) -> IResult<&str, AST> {
    let (input, _) = char('{')(input)?;
    not(char('!'))(input)?;
    not(char('>'))(input)?;

    let (input, arg1) = parse_block_arg(&['{',':',';','}'])(input)?;

    let (input, sep) = alt((
        tag(":"),
        tag(";}"),
        tag("}"),
        tag(";")
    ))(input)?;

    match sep {
        ":" => {
            let (input, (mut args, end)) = parse_block_args(input)?;
            args.insert(0, arg1);
            match end {
                ";}" => Ok((input, AST::Function(args))),
                "}"  => Ok((input, AST::Variable(args))),
                _    => unreachable!()
            }
        },
        ";}" => Ok((input, AST::Function(vec![arg1]))),
        "}"  => Ok((input, AST::Variable(vec![arg1]))),
        ";" => {
            let (input, _) = parse_block_args(input)?;
            Ok((input, AST::String(String::from("<error:missing semicolon>"))))
        },
        _ => unreachable!()
    }
}

pub fn parse_base(input: &str) -> IResult<&str, Vec<AST>> {
    fold_many0(
        alt((
            map(take_till1(|c| c == '{'), |s: &str| Some(AST::String(s.to_owned()))),
            map(parse_comment, |_| None),
            map(parse_block, Some)
        )),
        Vec::new(),
        add_block_arg
    )(input)
}