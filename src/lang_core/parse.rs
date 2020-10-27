extern crate nom;
use nom::{
    IResult,
    multi::many0,
    bytes::complete::{tag, take_until},
    combinator::not,
    branch::alt
};

pub fn parse_comment(input: &str) -> IResult<&str, ()> {
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