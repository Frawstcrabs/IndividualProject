extern crate nom;
use nom::{
    IResult,
    multi::{many0, fold_many0},
    bytes::complete::{tag, take_until, take_till1},
    combinator::{not, map},
    character::complete::char,
    branch::alt
};

#[derive(Clone, Debug)]
pub enum AST {
    String(String),
    Function(Vec<Vec<AST>>),
    Variable(Vec<Vec<AST>>)
}

enum ASTVariants {
    Comment,
    ASTValue(AST),
    ASTVec(Vec<AST>)
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

fn add_block_arg(mut vec: Vec<AST>, r: ASTVariants) -> Vec<AST> {
    match r {
        ASTVariants::ASTValue(ast) => {
            match (&ast, vec.last_mut()) {
                (AST::String(new_str), Some(AST::String(str))) => {
                    str.push_str(new_str);
                }
                _ => vec.push(ast)
            }
        }
        ASTVariants::ASTVec(mut ast) => {
            match (ast.first(), vec.last_mut()) {
                (Some(AST::String(v)), Some(AST::String(s))) => {
                    s.push_str(v);
                    ast.remove(0);
                }
                _ => {}
            }
            vec.append(&mut ast)
        }
        ASTVariants::Comment => {}
    }
    vec
}

fn parse_block_arg(chars: &[char]) -> impl Fn(&str) -> IResult<&str, Vec<AST>> + '_ {
    move |i: &str| {
        fold_many0(
            alt((
                map(take_till1(|c| chars.contains(&c)), |s: &str| ASTVariants::ASTValue(AST::String(s.to_owned()))),
                map(parse_escaped_block, ASTVariants::ASTVec),
                map(parse_comment, |_| ASTVariants::Comment),
                map(parse_block, ASTVariants::ASTValue)
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

fn parse_escaped_block(input: &str) -> IResult<&str, Vec<AST>> {
    let (input, _) = tag("{>")(input)?;
    let (input, mut body) = fold_many0(
        alt((
            map(take_till1(|c| c == '{' || c == '}'), |s: &str| ASTVariants::ASTValue(AST::String(s.to_owned()))),
            map(parse_escaped_block, ASTVariants::ASTVec),
            map(parse_comment, |_| ASTVariants::Comment),
            map(parse_block, ASTVariants::ASTValue)
        )),
        vec![AST::String(String::from("{"))],
        add_block_arg
    )(input)?;
    let (input, _) = tag("}")(input)?;
    match body.last_mut() {
        Some(AST::String(ref mut s)) => {
            s.push_str("}");
        }
        Some(_) | None => {
            body.push(AST::String(String::from("}")))
        }
    }
    Ok((input, body))
}

pub fn parse_base(input: &str) -> IResult<&str, Vec<AST>> {
    fold_many0(
        alt((
            map(take_till1(|c| c == '{'), |s: &str| ASTVariants::ASTValue(AST::String(s.to_owned()))),
            map(parse_escaped_block, ASTVariants::ASTVec),
            map(parse_comment, |_| ASTVariants::Comment),
            map(parse_block, ASTVariants::ASTValue)
        )),
        Vec::new(),
        add_block_arg
    )(input)
}