
struct Identifier(String);

enum Expression {

}

enum Statement {
    Closue(Vec<Identifier>, Vec<Expression>)
    Call(Vec<Expression>)
    Block(Vec<Statement>)
}
