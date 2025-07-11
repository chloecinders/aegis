use crate::{
    lexer::{Operator, Token, TokenStream},
    parser::Expr,
};

pub fn parse_math_expr(leading: Token, stream: &mut TokenStream) -> Expr {
    stream.push_front(leading);

    let mut chunk: Vec<Token> = Vec::new();

    while let Some(token) = stream.next() {
        if let Token::Semicolon = token {
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

    fn precedence(op: &Token) -> u8 {
        match op {
            Token::Operator(o) => match o {
                Operator::Plus | Operator::Minus => 1,
                Operator::Multiply | Operator::Divide => 2,
                _ => 0,
            },
            _ => 0,
        }
    }

    for token in stream {
        match token {
            Token::Word(_)
            | Token::String(_)
            | Token::Int(_)
            | Token::Float(_)
            | Token::Bool(_) => {
                output.push(token);
            }
            Token::Operator(_) => {
                while let Some(top) = stack.last() {
                    if let Token::Operator(_) = top {
                        if precedence(&token) <= precedence(top) {
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
            Token::OpenBracket => {
                stack.push(token);
            }
            Token::ClosedBracket => {
                while let Some(top) = stack.pop() {
                    if let Token::OpenBracket = top {
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
