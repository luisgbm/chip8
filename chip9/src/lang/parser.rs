//! Recursive descent over the tokens, producing the tree in [`super::ast`].

use super::ast::{
    AssignOp, BinOp, Const, Data, Expr, ExprKind, Function, Item, LogicalOp, Place, Program, Stmt,
    StmtKind, VarDecl,
};
use super::lexer::{Token, TokenKind};
use super::CompileError;

/// The names that cannot be used for anything else.
const KEYWORDS: &[&str] = &[
    "const", "var", "byte", "sprite", "fn", "if", "else", "while", "do", "loop", "goto", "return",
    "break", "continue", "delay", "sound",
];

pub struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.position].kind
    }

    fn peek_at(&self, offset: usize) -> &TokenKind {
        let index = (self.position + offset).min(self.tokens.len() - 1);
        &self.tokens[index].kind
    }

    fn line(&self) -> usize {
        self.tokens[self.position].line
    }

    fn advance(&mut self) -> TokenKind {
        let kind = self.tokens[self.position].kind.clone();
        if self.position + 1 < self.tokens.len() {
            self.position += 1;
        }
        kind
    }

    fn error<T>(&self, message: impl Into<String>) -> Result<T, CompileError> {
        Err(CompileError {
            line: self.line(),
            message: message.into(),
        })
    }

    fn at_symbol(&self, symbol: &str) -> bool {
        matches!(self.peek(), TokenKind::Symbol(found) if *found == symbol)
    }

    fn at_word(&self, word: &str) -> bool {
        matches!(self.peek(), TokenKind::Name(found) if found == word)
    }

    fn eat_symbol(&mut self, symbol: &str) -> bool {
        let found = self.at_symbol(symbol);
        if found {
            self.advance();
        }
        found
    }

    fn eat_word(&mut self, word: &str) -> bool {
        let found = self.at_word(word);
        if found {
            self.advance();
        }
        found
    }

    fn expect_symbol(&mut self, symbol: &str) -> Result<(), CompileError> {
        if self.eat_symbol(symbol) {
            return Ok(());
        }

        self.error(format!("expected `{symbol}`, found {}", self.peek()))
    }

    fn expect_name(&mut self, what: &str) -> Result<String, CompileError> {
        let TokenKind::Name(name) = self.peek().clone() else {
            return self.error(format!("expected {what}, found {}", self.peek()));
        };

        if KEYWORDS.contains(&name.as_str()) {
            return self.error(format!("`{name}` is a keyword, so it cannot name {what}"));
        }

        self.advance();
        Ok(name)
    }

    // -- items ---------------------------------------------------------------

    pub fn program(&mut self) -> Result<Program, CompileError> {
        let mut items = Vec::new();

        while !matches!(self.peek(), TokenKind::End) {
            items.push(self.item()?);
        }

        Ok(Program { items })
    }

    fn item(&mut self) -> Result<Item, CompileError> {
        let line = self.line();

        if self.eat_word("const") {
            let name = self.expect_name("a constant")?;
            self.expect_symbol("=")?;
            let value = self.expression()?;
            self.expect_symbol(";")?;
            return Ok(Item::Const(Const { name, value, line }));
        }

        if self.eat_word("var") {
            let declarations = self.var_declarations()?;
            self.expect_symbol(";")?;
            return Ok(Item::Var(declarations));
        }

        if self.eat_word("byte") {
            let name = self.expect_name("an array")?;
            self.expect_symbol("[")?;
            self.expect_symbol("]")?;
            self.expect_symbol("=")?;
            let bytes = self.brace_list()?;
            self.expect_symbol(";")?;
            return Ok(Item::Data(Data { name, bytes, line }));
        }

        if self.eat_word("sprite") {
            let name = self.expect_name("a sprite")?;
            self.expect_symbol("=")?;
            let bytes = self.brace_list()?;
            self.expect_symbol(";")?;
            return Ok(Item::Data(Data { name, bytes, line }));
        }

        if self.eat_word("fn") {
            let name = self.expect_name("a function")?;
            self.expect_symbol("(")?;
            let params = if self.at_symbol(")") {
                Vec::new()
            } else {
                self.var_declarations()?
            };
            self.expect_symbol(")")?;
            let body = self.block()?;
            return Ok(Item::Function(Function {
                name,
                params,
                body,
                line,
            }));
        }

        self.error(format!(
            "expected `const`, `var`, `byte`, `sprite` or `fn`, found {}",
            self.peek()
        ))
    }

    fn brace_list(&mut self) -> Result<Vec<Expr>, CompileError> {
        self.expect_symbol("{")?;
        let mut values = Vec::new();

        while !self.at_symbol("}") {
            values.push(self.expression()?);
            if !self.eat_symbol(",") {
                break;
            }
        }

        self.expect_symbol("}")?;
        Ok(values)
    }

    fn var_declarations(&mut self) -> Result<Vec<VarDecl>, CompileError> {
        let mut declarations = Vec::new();

        loop {
            let line = self.line();
            let name = self.expect_name("a variable")?;

            let register = if self.eat_symbol("@") {
                match self.advance() {
                    TokenKind::Register(index) => Some(index),
                    other => {
                        return Err(CompileError {
                            line,
                            message: format!("expected a register after `@`, found {other}"),
                        })
                    }
                }
            } else {
                None
            };

            declarations.push(VarDecl {
                name,
                register,
                line,
            });

            if !self.eat_symbol(",") {
                break;
            }
        }

        Ok(declarations)
    }

    // -- statements ----------------------------------------------------------

    fn block(&mut self) -> Result<Vec<Stmt>, CompileError> {
        self.expect_symbol("{")?;
        let mut body = Vec::new();

        while !self.at_symbol("}") {
            if matches!(self.peek(), TokenKind::End) {
                return self.error("expected `}` before the end of the file");
            }
            body.push(self.statement()?);
        }

        self.expect_symbol("}")?;
        Ok(body)
    }

    fn statement(&mut self) -> Result<Stmt, CompileError> {
        let line = self.line();
        let kind = self.statement_kind()?;

        Ok(Stmt { kind, line })
    }

    fn statement_kind(&mut self) -> Result<StmtKind, CompileError> {
        if self.at_symbol("{") {
            return Ok(StmtKind::Block(self.block()?));
        }

        if self.eat_symbol(";") {
            return Ok(StmtKind::Block(Vec::new()));
        }

        if self.eat_word("var") {
            let declarations = self.var_declarations()?;
            self.expect_symbol(";")?;
            return Ok(StmtKind::Var(declarations));
        }

        if self.eat_word("if") {
            self.expect_symbol("(")?;
            let condition = self.expression()?;
            self.expect_symbol(")")?;
            let then = Box::new(self.statement()?);
            let otherwise = if self.eat_word("else") {
                Some(Box::new(self.statement()?))
            } else {
                None
            };
            return Ok(StmtKind::If {
                condition,
                then,
                otherwise,
            });
        }

        if self.eat_word("while") {
            self.expect_symbol("(")?;
            let condition = self.expression()?;
            self.expect_symbol(")")?;
            let body = Box::new(self.statement()?);
            return Ok(StmtKind::While { condition, body });
        }

        if self.eat_word("do") {
            let body = Box::new(self.statement()?);
            if !self.eat_word("while") {
                return self.error(format!(
                    "expected `while` after `do`, found {}",
                    self.peek()
                ));
            }
            self.expect_symbol("(")?;
            let condition = self.expression()?;
            self.expect_symbol(")")?;
            self.expect_symbol(";")?;
            return Ok(StmtKind::DoWhile { body, condition });
        }

        if self.eat_word("loop") {
            return Ok(StmtKind::Loop(Box::new(self.statement()?)));
        }

        if self.eat_word("goto") {
            let name = self.expect_name("a label")?;
            self.expect_symbol(";")?;
            return Ok(StmtKind::Goto(name));
        }

        if self.eat_word("return") {
            let value = if self.at_symbol(";") {
                None
            } else {
                Some(self.expression()?)
            };
            self.expect_symbol(";")?;
            return Ok(StmtKind::Return(value));
        }

        if self.eat_word("break") {
            self.expect_symbol(";")?;
            return Ok(StmtKind::Break);
        }

        if self.eat_word("continue") {
            self.expect_symbol(";")?;
            return Ok(StmtKind::Continue);
        }

        // `name:` is a label, and nothing else starts that way.
        if let TokenKind::Name(name) = self.peek().clone() {
            if matches!(self.peek_at(1), TokenKind::Symbol(":")) {
                self.advance();
                self.advance();
                return Ok(StmtKind::Label(name));
            }

            // `name(...)` on its own is a call.
            if matches!(self.peek_at(1), TokenKind::Symbol("(")) {
                self.advance();
                let arguments = self.arguments()?;
                self.expect_symbol(";")?;
                return Ok(StmtKind::Call { name, arguments });
            }
        }

        let target = self.place()?;
        let operator = self.assign_operator()?;
        let value = self.expression()?;
        self.expect_symbol(";")?;

        Ok(StmtKind::Assign {
            target,
            operator,
            value,
        })
    }

    fn place(&mut self) -> Result<Place, CompileError> {
        if self.eat_word("delay") {
            return Ok(Place::Delay);
        }

        if self.eat_word("sound") {
            return Ok(Place::Sound);
        }

        if matches!(self.peek(), TokenKind::Name(name) if name == "I") {
            self.advance();
            return Ok(Place::Index);
        }

        Ok(Place::Var(self.expect_name("a variable")?))
    }

    fn assign_operator(&mut self) -> Result<AssignOp, CompileError> {
        let operator = match self.peek() {
            TokenKind::Symbol("=") => AssignOp::Set,
            TokenKind::Symbol("+=") => AssignOp::Add,
            TokenKind::Symbol("-=") => AssignOp::Sub,
            TokenKind::Symbol("*=") => AssignOp::Mul,
            TokenKind::Symbol("/=") => AssignOp::Div,
            TokenKind::Symbol("%=") => AssignOp::Mod,
            TokenKind::Symbol("&=") => AssignOp::And,
            TokenKind::Symbol("|=") => AssignOp::Or,
            TokenKind::Symbol("^=") => AssignOp::Xor,
            TokenKind::Symbol(">>=") => AssignOp::Shr,
            TokenKind::Symbol("<<=") => AssignOp::Shl,
            other => {
                return Err(CompileError {
                    line: self.line(),
                    message: format!("expected an assignment, found {other}"),
                })
            }
        };

        self.advance();
        Ok(operator)
    }

    fn arguments(&mut self) -> Result<Vec<Expr>, CompileError> {
        self.expect_symbol("(")?;
        let mut arguments = Vec::new();

        while !self.at_symbol(")") {
            arguments.push(self.expression()?);
            if !self.eat_symbol(",") {
                break;
            }
        }

        self.expect_symbol(")")?;
        Ok(arguments)
    }

    // -- expressions ---------------------------------------------------------
    //
    // The levels are C's, so `a + b < c` groups the way it looks.

    pub fn expression(&mut self) -> Result<Expr, CompileError> {
        self.logical(0)
    }

    /// `||` binds loosest, then `&&`, then everything the machine can work out
    /// in a register.
    fn logical(&mut self, level: usize) -> Result<Expr, CompileError> {
        const LEVELS: &[(&str, LogicalOp)] = &[("||", LogicalOp::Or), ("&&", LogicalOp::And)];

        let Some(&(symbol, operator)) = LEVELS.get(level) else {
            return self.binary(0);
        };

        let mut left = self.logical(level + 1)?;

        while self.at_symbol(symbol) {
            let line = self.line();
            self.advance();
            let right = self.logical(level + 1)?;

            left = Expr {
                kind: ExprKind::Logical {
                    operator,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                line,
            };
        }

        Ok(left)
    }

    fn binary(&mut self, level: usize) -> Result<Expr, CompileError> {
        const LEVELS: &[&[(&str, BinOp)]] = &[
            &[("|", BinOp::Or)],
            &[("^", BinOp::Xor)],
            &[("&", BinOp::And)],
            &[("==", BinOp::Equal), ("!=", BinOp::NotEqual)],
            &[
                ("<", BinOp::Less),
                ("<=", BinOp::LessOrEqual),
                (">", BinOp::Greater),
                (">=", BinOp::GreaterOrEqual),
            ],
            &[(">>", BinOp::Shr), ("<<", BinOp::Shl)],
            &[("+", BinOp::Add), ("-", BinOp::Sub)],
            &[("*", BinOp::Mul), ("/", BinOp::Div), ("%", BinOp::Mod)],
        ];

        let Some(operators) = LEVELS.get(level) else {
            return self.unary();
        };

        let mut left = self.binary(level + 1)?;

        loop {
            let Some(&(_, operator)) = operators.iter().find(|(symbol, _)| self.at_symbol(symbol))
            else {
                return Ok(left);
            };

            let line = self.line();
            self.advance();
            let right = self.binary(level + 1)?;

            left = Expr {
                kind: ExprKind::Binary {
                    operator,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                line,
            };
        }
    }

    fn unary(&mut self) -> Result<Expr, CompileError> {
        let line = self.line();

        if self.eat_symbol("!") {
            return Ok(Expr {
                kind: ExprKind::Not(Box::new(self.unary()?)),
                line,
            });
        }

        // `-x` only makes sense for constants, and `0 - x` covers the rest.
        if self.eat_symbol("-") {
            let inner = self.unary()?;
            return Ok(Expr {
                kind: ExprKind::Binary {
                    operator: BinOp::Sub,
                    left: Box::new(Expr {
                        kind: ExprKind::Number(0),
                        line,
                    }),
                    right: Box::new(inner),
                },
                line,
            });
        }

        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, CompileError> {
        let line = self.line();

        if self.eat_symbol("(") {
            let inner = self.expression()?;
            self.expect_symbol(")")?;
            return Ok(inner);
        }

        match self.peek().clone() {
            TokenKind::Number(value) => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Number(value),
                    line,
                })
            }
            TokenKind::Register(index) => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Register(index),
                    line,
                })
            }
            TokenKind::Name(name) if name == "delay" => {
                self.advance();
                Ok(Expr {
                    kind: ExprKind::Delay,
                    line,
                })
            }
            TokenKind::Name(name) => {
                if KEYWORDS.contains(&name.as_str()) {
                    return self.error(format!("`{name}` cannot be used in an expression"));
                }

                self.advance();

                if self.at_symbol("(") {
                    let arguments = self.arguments()?;
                    return Ok(Expr {
                        kind: ExprKind::Call { name, arguments },
                        line,
                    });
                }

                if self.eat_symbol("[") {
                    let index = Box::new(self.expression()?);
                    self.expect_symbol("]")?;
                    return Ok(Expr {
                        kind: ExprKind::Index { name, index },
                        line,
                    });
                }

                Ok(Expr {
                    kind: ExprKind::Name(name),
                    line,
                })
            }
            other => self.error(format!("expected a value, found {other}")),
        }
    }
}
