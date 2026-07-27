use linkage_blaze_core::{LinkageFixed, LinkageView, linkage_program};

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

const _: LinkageView<'static, 0, 0> = declarations::Private::VIEW;

fn main() {}
