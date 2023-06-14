use std::sync::Arc;

pub mod fork;

struct A {}

struct S<'a> {
    a: &'a A,
}

struct C<'a> {
    a: &'a A,
}

struct B<'a> {
    c: C<'a>,
}

fn create<'a>(a: &'a A) -> B<'a> {
    let c = C { a: a };
    B { c: c }
}
