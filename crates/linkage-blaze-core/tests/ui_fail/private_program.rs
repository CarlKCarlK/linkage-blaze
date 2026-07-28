use linkage_blaze_core::{LinkageFixed, linkage_program};

mod declarations {
    use super::*;

    linkage_program! {
        Private {
            program: LinkageFixed::<0, 0, 1>::start(),
            dof: 0,
            marks: 0,
        }
    }
}

const _: LinkageFixed<0, 0, 1> = declarations::Private::fixed();

fn main() {}
