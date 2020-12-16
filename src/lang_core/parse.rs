extern crate nom;
use nom::{
    IResult, Err, InputTake, FindSubstring, InputLength,
    error::ParseError,
    multi::{many0, many1, fold_many0, separated_list},
    bytes::complete::{tag, take_until, take_till1},
    combinator::{not, map, opt, peek},
    character::complete::{char, anychar, multispace0, line_ending},
    branch::alt,
    sequence::{pair, delimited, preceded},
};
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub enum AST {
    String(String),
    Variable(VarAccess),
    SetVar(VarAccess, Vec<AST>),
    DelVar(VarAccess),
}

#[derive(Clone, Debug)]
pub enum Accessor {
    Index(Vec<AST>),
    Attr(Vec<AST>),
    Call(Vec<Vec<AST>>)
}

#[derive(Clone, Debug)]
pub struct VarAccess {
    pub(crate) value: Vec<AST>,
    pub(crate) accessors: Vec<Accessor>,
}

enum ASTVariants {
    ASTValue(AST),
    ASTVec(Vec<AST>)
}

fn parse_comment(input: &str) -> IResult<&str, ()> {
    let (input, _) = tag("{!")(input)?;

    fn parse_comment_str(input: &str) -> IResult<&str, ()> {
        let (input, _) = not(
            alt((tag("{!"), tag("!}")))
        )(input)?;

        let (input1, test1) = opt(take_until("{!"))(input)?;
        let (input2, test2) = opt(take_until("!}"))(input)?;
        match (test1, test2) {
            (None, None) => {
                panic!("unclosed comment")
            },
            (Some(_), None) => {
                Ok((input1, ()))
            },
            (None, Some(_)) => {
                Ok((input2, ()))
            },
            (Some(_), Some(_)) => {
                if input1.len() > input2.len() {
                    Ok((input1, ()))
                } else {
                    Ok((input2, ()))
                }
            }
        }
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

fn remove_comments(input: &str) -> Result<String, ()> {
    let (rem, strings) = delimited(
        opt(parse_comment),
        separated_list(
            parse_comment,
            take_until_or_eof("{!")
        ),
        opt(parse_comment)
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

fn parse_escaped_char(chars: &[char]) -> impl Fn(&str) -> IResult<&str, Cow<str>> + '_ {
    move |init_input: &str| {
        let (input, _) = char('\\')(init_input)?;
        let (input, c) = anychar(input)?;

        let escaped_c;
        if chars.contains(&c) {
            escaped_c = Cow::Borrowed(&init_input[1 .. c.len_utf8()+1]);
        } else if c == 'n' {
            escaped_c = Cow::Owned(String::from("\n"));
        } else {
            escaped_c = Cow::Borrowed(&init_input[0 .. c.len_utf8()+1]);
        }

        Ok((input, escaped_c))
    }
}

fn parse_string(chars: &[char]) -> impl Fn(&str) -> IResult<&str, String> + '_ {
    move |input| {
        let (input, strings) = many1(alt((
            parse_escaped_char(chars),
            map(
                take_till1(|c: char| c == '\\' || chars.contains(&c)),
                Cow::Borrowed
            )
        )))(input)?;

        let size = strings.iter().map(|s| s.len()).sum();
        let mut ret = String::with_capacity(size);
        for s in strings {
            ret.push_str(&*s);
        }
        Ok((input, ret))
    }
}

fn add_block_arg(mut vec: Vec<AST>, r: ASTVariants) -> Vec<AST> {
    fn try_join_strings(ast: AST, vec: &mut Vec<AST>) {
        match (&ast, vec.last_mut()) {
            (AST::String(new_str), Some(AST::String(str))) => {
                str.push_str(new_str);
            }
            _ => vec.push(ast)
        }
    }
    match r {
        ASTVariants::ASTValue(ast) => {
            try_join_strings(ast, &mut vec);
        }
        ASTVariants::ASTVec(ast) => {
            let mut iter = ast.into_iter();
            if let Some(ast) = iter.next() {
                try_join_strings(ast, &mut vec);
            }
            vec.extend(iter);
        }
    }
    vec
}

macro_rules! match_strings {
    ($($str:expr),+) => {
        alt((
            $(tag($str)),+
        ))
    };
}

fn parse_var_access(input: &str) -> IResult<&str, VarAccess> {
    let (input, value) = parse_block_arg(&['.', '[', ':', ';', '{', '}'])(input)?;

    fn parse_index(input: &str) -> IResult<&str, Accessor> {
        map(
            delimited(tag("["), parse_block_arg(&['{', ']']), tag("]")),
            |v| Accessor::Index(v)
        )(input)
    }
    fn parse_attr(input: &str) -> IResult<&str, Accessor> {
        map(
            preceded(tag("."), parse_block_arg(&['{', '.', '[', ':', ';'])),
            |v| Accessor::Attr(v)
        )(input)
    }
    fn parse_call(mut input: &str) -> IResult<&str, Accessor> {
        let mut args = Vec::new();
        loop {
            let (i, sep) = match_strings!(":", ";")(input)?;
            match sep {
                ":" => {
                    let (i, arg) = parse_block_arg(&['{', ':', ';'])(i)?;
                    args.push(arg);
                    input = i;
                    continue;
                },
                ";" => {
                    return Ok((i, Accessor::Call(args)));
                },
                _ => unreachable!()
            }
        }
    }

    let (input, accessors) = many0(alt((parse_index, parse_attr, parse_call)))(input)?;

    Ok((input, VarAccess {value, accessors}))
}

fn parse_block_arg(chars: &[char]) -> impl Fn(&str) -> IResult<&str, Vec<AST>> + '_ {
    move |i: &str| {
        fold_many0(
            alt((
                map(parse_string(chars), |s| ASTVariants::ASTValue(AST::String(s))),
                map(parse_escaped_block, ASTVariants::ASTVec),
                map(parse_set_block, ASTVariants::ASTValue),
                map(parse_func_block, ASTVariants::ASTValue),
                map(parse_del_block, ASTVariants::ASTValue),
                map(parse_block, ASTVariants::ASTValue)
            )),
            Vec::new(),
            add_block_arg
        )(i)
    }
}

fn parse_set_block(input: &str) -> IResult<&str, AST> {
    let (input, _) = tag("{set:")(input)?;
    let (input, mut access) = parse_var_access(input)?;
    let val;
    match access.accessors.pop() {
        Some(Accessor::Call(mut args)) => {
            assert!(args.len() == 1);
            val = args.pop().unwrap();
        },
        _ => {
            panic!("invalid call to set");
        },
    }
    let (input, _) = tag("}")(input)?;
    Ok((input, AST::SetVar(access, val)))
}

fn parse_del_block(input: &str) -> IResult<&str, AST> {
    let (input, _) = tag("{del:")(input)?;
    let (input, mut access) = parse_var_access(input)?;
    match access.accessors.pop() {
        Some(Accessor::Call(args)) => {
            assert!(args.is_empty());
        },
        _ => {
            panic!("invalid call to set");
        },
    }
    let (input, _) = tag("}")(input)?;
    Ok((input, AST::DelVar(access)))
}

fn parse_func_block(input: &str) -> IResult<&str, AST> {
    let (input, _) = tag("{func:{")(input)?;
    not(tag(">"))(input)?;
    let (input, name) = parse_string(&[':', ';', '{', '}', '[', ']', '.'])(input)?;
    let (mut input, sep) = match_strings!(":", ";", "{", "}", "[", "]", ".")(input)?;
    let mut args = Vec::new();
    match sep {
        ";" => {
            // no args in function
        },
        ":" => loop {
            let (i, name) = parse_string(&[':', ';', '{', '}', '[', ']', '.'])(input)?;
            let (i, sep) = match_strings!(":", ";", "{", "}", "[", "]", ".")(i)?;
            args.push(vec![AST::String(name)]);
            input = i;
            match sep {
                ":" => {
                    continue;
                },
                ";" => {
                    break;
                },
                "{" | "}" | "[" | "]" | "." => {
                    panic!("invalid character in func arg name");
                },
                _ => unreachable!()
            }
        },
        "{" | "}" | "[" | "]" | "." | ">" => {
            panic!("invalid character in func arg name");
        },
        _ => unreachable!()
    }
    let (input, _) = tag("}:")(input)?;
    let (input, body) = parse_block_arg(&['{', ';'])(input)?;
    let (input, _) = tag(";}")(input)?;
    args.push(body);
    Ok((input, AST::SetVar(
        VarAccess {
            value: vec![AST::String(name)],
            accessors: Vec::new()
        },
        vec![AST::Variable(
            VarAccess {
                value: vec![AST::String(String::from("lambda"))],
                accessors: vec![Accessor::Call(args)]
            }
        )]
    )))
}

fn parse_block(input: &str) -> IResult<&str, AST> {
    let (input, _) = char('{')(input)?;
    not(char('!'))(input)?;
    not(char('>'))(input)?;

    let (input, var) = parse_var_access(input)?;

    let (input, _) = char('}')(input)?;

    Ok((input, AST::Variable(var)))
}

fn parse_escaped_block(input: &str) -> IResult<&str, Vec<AST>> {
    let (input, _) = tag("{>")(input)?;
    let (input, mut body) = fold_many0(
        alt((
            map(parse_string(&['{', '}']), |s| ASTVariants::ASTValue(AST::String(s))),
            map(parse_escaped_block, ASTVariants::ASTVec),
            map(parse_set_block, ASTVariants::ASTValue),
            map(parse_func_block, ASTVariants::ASTValue),
            map(parse_del_block, ASTVariants::ASTValue),
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

fn parse_base(input: &str) -> IResult<&str, Vec<AST>> {
    fold_many0(
        alt((
            map(parse_string(&['{']), |s| ASTVariants::ASTValue(AST::String(s))),
            map(parse_escaped_block, ASTVariants::ASTVec),
            map(parse_set_block, ASTVariants::ASTValue),
            map(parse_func_block, ASTVariants::ASTValue),
            map(parse_del_block, ASTVariants::ASTValue),
            map(parse_block, ASTVariants::ASTValue)
        )),
        Vec::new(),
        add_block_arg
    )(input)
}

fn parse_oneline(input: String) -> Result<String, ()> {
    fn check_oneline(inp: &str) -> IResult<&str, &str> {
        let (input, _) = multispace0(inp)?;
        tag("{!>oneline}")(input)
    }

    if let Ok((input, _)) = check_oneline(&input) {
        let (rem, strings) = delimited(
            multispace0,
            separated_list(
                pair(line_ending, multispace0),
                take_till1(|c| c == '\r' || c == '\n')
            ),
            multispace0
        )(input).map_err(|_: Err<()>| ())?;
        if rem.len() > 0 {
            return Err(());
        }
        let size = strings.iter().map(|s| s.len()).sum();
        let mut ret = String::with_capacity(size);
        for s in strings {
            ret.push_str(s);
        }
        Ok(ret)
    } else {
        Ok(input)
    }
}

pub fn run_parser(input: &str) -> Result<Vec<AST>, ()> {
    let input = parse_oneline(input.to_owned())?;
    let input = remove_comments(&input)?;
    match parse_base(&input) {
        Ok((rem, ast)) => {
            if rem.len() == 0 {
                Ok(ast)
            } else {
                Err(())
            }
        },
        Err(v) => {
            println!("parse error: {:?}", v);
            Err(())
        },
    }
}