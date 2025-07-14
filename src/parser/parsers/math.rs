use crate::{
    lexer::{Operator, Token, TokenKind, TokenStream},
    parser::Expr,
};

pub fn parse_math(leading: Token, stream: &mut TokenStream) -> Expr {
    stream.push_front(leading);

    let mut chunk: Vec<Token> = Vec::new();

    while let Some(token) = stream.next() {
        if let Token {
            kind: TokenKind::Semicolon,
            ..
        } = token
        {
            break;
        }

        chunk.push(token);
    }

    let postfix_expr: Vec<Box<Token>> = infix_to_postfix(chunk)
        .into_iter()
        .map(|e| Box::new(e))
        .collect();

    Expr::Operation {
        stack: postfix_expr,
    }
}

fn infix_to_postfix(stream: Vec<Token>) -> Vec<Token> {
    let mut output: Vec<Token> = Vec::new();
    let mut stack: Vec<Token> = Vec::new();

    fn precedence(op: &TokenKind) -> u8 {
        match op {
            TokenKind::Operator(o) => match o {
                Operator::Plus | Operator::Minus => 1,
                Operator::Multiply | Operator::Divide => 2,
                _ => 0,
            },
            _ => 0,
        }
    }

    for token in stream {
        match token.kind {
            TokenKind::Word(_)
            | TokenKind::String(_)
            | TokenKind::Int(_)
            | TokenKind::Float(_)
            | TokenKind::Bool(_) => {
                output.push(token);
            }
            TokenKind::Operator(_) => {
                while let Some(top) = stack.last() {
                    if let Token {
                        kind: TokenKind::Operator(_),
                        ..
                    } = top
                    {
                        if precedence(&token.kind) <= precedence(&top.kind) {
                            output.push(stack.pop().unwrap());
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                stack.push(token);
            }
            TokenKind::OpenBracket => {
                stack.push(token);
            }
            TokenKind::ClosedBracket => {
                while let Some(top) = stack.pop() {
                    if let Token {
                        kind: TokenKind::OpenBracket,
                        ..
                    } = top
                    {
                        break;
                    } else {
                        output.push(top);
                    }
                }
            }
            _ => {}
        }
    }

    while let Some(op) = stack.pop() {
        output.push(op);
    }

    output
}
