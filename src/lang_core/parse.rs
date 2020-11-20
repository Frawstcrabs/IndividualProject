extern crate nom;
use nom::{
    IResult,
    multi::{many0, fold_many0, separated_list},
    bytes::complete::{tag, take_until, take_till1},
    combinator::{not, map},
    character::complete::{char, anychar},
    branch::alt
};
use std::borrow::Cow;
use nom::Err;
use nom::{InputTake, FindSubstring, InputLength};
use nom::error::{ParseError, ErrorKind};

#[derive(Clone, Debug)]
pub enum AST {
    String(String),
    Function(Vec<Vec<AST>>),
    Variable(Vec<Vec<AST>>)
}

enum ASTVariants {
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

fn take_until_or_eof<T, Input, Error: ParseError<Input>>(
    tag: T,
) -> impl Fn(Input) -> IResult<Input, Input, Error>
    where
        Input: InputTake + InputLength + FindSubstring<T>,
        T: InputLength + Clone,
{
    move |i: Input| {
        let t = tag.clone();
        let res: IResult<_, _, Error> = match i.find_substring(t) {
            None => Ok(i.take_split(i.input_len())),
            Some(index) => Ok(i.take_split(index)),
        };
        res
    }
}

fn take_until1_or_eof<T, Input, Error: ParseError<Input>>(
    tag: T,
) -> impl Fn(Input) -> IResult<Input, Input, Error>
    where
        Input: InputTake + InputLength + FindSubstring<T>,
        T: InputLength + Clone,
{
    move |i: Input| {
        let t = tag.clone();
        let res: IResult<_, _, Error> = match i.find_substring(t) {
            None => if i.input_len() > 0 {
                Ok(i.take_split(i.input_len()))
            } else {
                Err(Err::Error(Error::from_error_kind(i, ErrorKind::TakeUntil)))
            },
            Some(index) => if index > 0 {
                Ok(i.take_split(index))
            } else {
                Err(Err::Error(Error::from_error_kind(i, ErrorKind::TakeUntil)))
            },
        };
        res
    }
}

fn remove_comments(input: &str) -> Result<String, ()> {
    let (rem, strings) = separated_list(
        parse_comment,
        take_until_or_eof("{!")
    )(input).map_err(|_| ())?;
    if rem.len() > 0 {
        return Err(());
    }
    let size = strings.iter().map(|s| s.len()).sum();
    let mut ret = String::with_capacity(size);
    for s in strings {
        ret.push_str(s);
    }
    Ok(ret)
}

fn handle_escapes(input: &str) -> Result<String, ()> {
    let (rem, strings) = many0(alt((
        map(parse_escaped_char, Cow::Owned),
        map(take_until1_or_eof("\\"), Cow::Borrowed)
    )))(input).map_err(|_| ())?;
    if rem.len() > 0 {
        return Err(());
    }
    let size = strings.iter().map(|v| v.len()).sum();
    let mut ret = String::with_capacity(size);
    for s in strings {
        ret.push_str(&*s);
    }
    Ok(ret)
}

fn parse_escaped_char(input: &str) -> IResult<&str, String> {
    let (input, _) = char('\\')(input)?;
    let (input, c) = anychar(input)?;

    let escaped_c = match c {
        '{' | '}' | ':' | ';' | '\\' | '>' => c.to_string(),
        'n' => String::from("\n"),
        _ => {
            let mut ret = String::from("\\");
            ret.push(c);
            ret
        }
    };

    Ok((input, escaped_c))
}

fn parse_string(check: impl Fn(char) -> bool) -> impl Fn(&str) -> IResult<&str, String> {
    move |i| {
        map(take_till1(|c| check(c)), String::from)(i)
    }
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
    }
    vec
}

fn parse_block_arg(chars: &[char]) -> impl Fn(&str) -> IResult<&str, Vec<AST>> + '_ {
    move |i: &str| {
        fold_many0(
            alt((
                map(parse_string(|c| chars.contains(&c)), |s| ASTVariants::ASTValue(AST::String(s))),
                map(parse_escaped_block, ASTVariants::ASTVec),
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
            map(parse_string(|c| c == '{' || c == '}'), |s| ASTVariants::ASTValue(AST::String(s))),
            map(parse_escaped_block, ASTVariants::ASTVec),
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
            map(parse_string(|c| c == '{'), |s| ASTVariants::ASTValue(AST::String(s))),
            map(parse_escaped_block, ASTVariants::ASTVec),
            map(parse_block, ASTVariants::ASTValue)
        )),
        Vec::new(),
        add_block_arg
    )(input)
}

pub fn run_parser(input: &str) -> Result<Vec<AST>, ()> {
    let input = remove_comments(input)?;
    let input = handle_escapes(&input)?;
    match parse_base(&input) {
        Ok((rem, ast)) => {
            if rem.len() == 0 {
                Ok(ast)
            } else {
                Err(())
            }
        },
        Err(_) => Err(()),
    }
}